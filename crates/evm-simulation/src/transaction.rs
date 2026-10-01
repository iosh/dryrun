use alloy::{
    consensus::TxType,
    primitives::{Address, TxKind, U256},
    providers::{DynProvider, Provider, utils::EIP1559_BASE_FEE_MULTIPLIER},
    rpc::types::{TransactionInput, TransactionRequest},
};
use alloy_evm::EvmEnv;
use revm::{
    context::{Block, TxEnv},
    context_interface::either::Either,
    primitives::hardfork::SpecId,
};

use crate::{Error, db::ForkDatabase};

/// Owns a compatible RPC request until its missing parameters are filled.
pub(crate) struct Preparation {
    request: TransactionRequest,
    sender: Address,
    explicit_type: Option<TxType>,
}

impl Preparation {
    /// Checks the request's representation before any RPC is issued.
    pub(crate) fn new(request: TransactionRequest) -> Result<Self, Error> {
        let sender = request
            .from
            .ok_or_else(|| Error::InvalidInput("from is required".into()))?;
        let explicit_type = request
            .transaction_type
            .map(|value| {
                TxType::try_from(value).map_err(|_| {
                    Error::InvalidInput(format!("unsupported transaction type {value:#x}"))
                })
            })
            .transpose()?;
        let tx_type = explicit_type.unwrap_or_else(|| request.minimal_tx_type());
        check_type_compatibility(&request, tx_type)?;
        request
            .input
            .unique_input()
            .map_err(|error| Error::InvalidInput(error.to_string()))?;
        Ok(Self {
            request,
            sender,
            explicit_type,
        })
    }

    /// Returns the completed request and its VM transaction on the fixed database.
    pub(crate) async fn complete(
        self,
        env: &EvmEnv,
        db: &mut ForkDatabase,
    ) -> Result<(TransactionRequest, TxEnv), Error> {
        let Self {
            mut request,
            sender,
            explicit_type,
        } = self;
        // For compatible input, London only upgrades legacy/access-list requests
        // without gasPrice to EIP-1559, preserving their existing fields.
        let tx_type = explicit_type.unwrap_or_else(|| {
            if env.cfg_env.spec.is_enabled_in(SpecId::LONDON) {
                request.preferred_type()
            } else {
                request.minimal_tx_type()
            }
        });
        request.transaction_type = Some(tx_type as u8);
        request.chain_id.get_or_insert(env.cfg_env.chain_id);
        request.value.get_or_insert(U256::ZERO);
        if request.input.input().is_none() {
            request.input = TransactionInput::new(Default::default());
        }
        if tx_type != TxType::Legacy {
            request.access_list.get_or_insert_default();
        }
        if request.blob_versioned_hashes.is_none() {
            request.populate_blob_hashes();
        }
        if request.nonce.is_none() {
            request.nonce = Some(db.account(sender).await?.unwrap_or_default().nonce);
        }

        let provider = db.provider();
        fill_fees(&mut request, tx_type, provider, env).await?;
        if request.gas.is_none() {
            let gas = provider
                .estimate_gas(request.clone())
                .block(db.block())
                .await
                .map_err(|source| Error::Provider {
                    operation: "eth_estimateGas",
                    source,
                })?;
            request.gas = Some(gas);
        }
        let tx = to_tx_env(&request, sender, tx_type)?;
        Ok((request, tx))
    }
}

/// Fills missing prices without changing explicit prices or checking VM rules.
async fn fill_fees(
    request: &mut TransactionRequest,
    tx_type: TxType,
    provider: &DynProvider,
    env: &EvmEnv,
) -> Result<(), Error> {
    if matches!(tx_type, TxType::Legacy | TxType::Eip2930) {
        if request.gas_price.is_none() {
            let price = provider
                .get_gas_price()
                .await
                .map_err(|source| Error::Provider {
                    operation: "eth_gasPrice",
                    source,
                })?;
            request.gas_price = Some(price);
        }
        return Ok(());
    }

    let priority_fee = match request.max_priority_fee_per_gas {
        Some(price) => price,
        None => {
            let price = provider
                .get_max_priority_fee_per_gas()
                .await
                .map_err(|source| Error::Provider {
                    operation: "eth_maxPriorityFeePerGas",
                    source,
                })?;
            request.max_priority_fee_per_gas = Some(price);
            price
        }
    };
    if request.max_fee_per_gas.is_none() {
        let cap = (u128::from(env.block_env.basefee) * EIP1559_BASE_FEE_MULTIPLIER)
            .checked_add(priority_fee)
            .ok_or_else(|| {
                Error::Unsupported(
                    "automatic fee cap exceeds the supported range; specify maxFeePerGas".into(),
                )
            })?;
        request.max_fee_per_gas = Some(cap);
    }
    if tx_type == TxType::Eip4844 && request.max_fee_per_blob_gas.is_none() {
        request.max_fee_per_blob_gas = Some(env.block_env.blob_gasprice().ok_or_else(|| {
            Error::Unsupported("the selected block has no blob gas price".into())
        })?);
    }
    Ok(())
}

/// Converts the completed request. Missing required parameters here indicate a
/// completion bug; optional fields retain their transaction-defined defaults.
fn to_tx_env(
    request: &TransactionRequest,
    caller: Address,
    tx_type: TxType,
) -> Result<TxEnv, Error> {
    let (Some(nonce), Some(chain_id), Some(gas_limit)) =
        (request.nonce, request.chain_id, request.gas)
    else {
        return Err(Error::Internal(
            "transaction completion omitted nonce, chainId or gas",
        ));
    };
    let (gas_price, gas_priority_fee) = match tx_type {
        TxType::Legacy | TxType::Eip2930 => (
            request
                .gas_price
                .ok_or(Error::Internal("transaction completion omitted gasPrice"))?,
            None,
        ),
        _ => {
            let (Some(cap), Some(tip)) =
                (request.max_fee_per_gas, request.max_priority_fee_per_gas)
            else {
                return Err(Error::Internal(
                    "transaction completion omitted dynamic fees",
                ));
            };
            (cap, Some(tip))
        }
    };
    let max_fee_per_blob_gas = if tx_type == TxType::Eip4844 {
        request.max_fee_per_blob_gas.ok_or(Error::Internal(
            "transaction completion omitted maxFeePerBlobGas",
        ))?
    } else {
        0
    };

    Ok(TxEnv {
        tx_type: tx_type as u8,
        caller,
        gas_limit,
        gas_price,
        gas_priority_fee,
        kind: request.to.unwrap_or(TxKind::Create),
        value: request.value.unwrap_or_default(),
        data: request.input.input().cloned().unwrap_or_default(),
        nonce,
        chain_id: Some(chain_id),
        access_list: request.access_list.clone().unwrap_or_default(),
        blob_hashes: request.blob_versioned_hashes.clone().unwrap_or_default(),
        max_fee_per_blob_gas,
        authorization_list: request
            .authorization_list
            .iter()
            .flatten()
            .cloned()
            .map(Either::Left)
            .collect(),
    })
}

/// Rejects fields that cannot be represented by the chosen transaction type.
fn check_type_compatibility(request: &TransactionRequest, tx_type: TxType) -> Result<(), Error> {
    if matches!(tx_type, TxType::Legacy | TxType::Eip2930) {
        if request.max_fee_per_gas.is_some() || request.max_priority_fee_per_gas.is_some() {
            return Err(Error::InvalidInput(
                "dynamic fee fields require a dynamic fee transaction type".into(),
            ));
        }
    } else if request.gas_price.is_some() {
        return Err(Error::InvalidInput(
            "gasPrice requires a legacy or EIP-2930 transaction".into(),
        ));
    }
    if tx_type == TxType::Legacy && request.access_list.is_some() {
        return Err(Error::InvalidInput(
            "accessList is not a field of a legacy transaction".into(),
        ));
    }
    if tx_type != TxType::Eip4844
        && (request.has_eip4844_blob_data() || request.max_fee_per_blob_gas.is_some())
    {
        return Err(Error::InvalidInput(
            "blob fields require an EIP-4844 transaction".into(),
        ));
    }
    if tx_type != TxType::Eip7702 && request.authorization_list.is_some() {
        return Err(Error::InvalidInput(
            "authorizationList requires an EIP-7702 transaction".into(),
        ));
    }
    if matches!(tx_type, TxType::Eip4844 | TxType::Eip7702)
        && !matches!(request.to, Some(TxKind::Call(_)))
    {
        return Err(Error::InvalidInput(format!(
            "transaction type {:#x} cannot create a contract",
            tx_type as u8
        )));
    }
    Ok(())
}
