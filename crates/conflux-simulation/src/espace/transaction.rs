use crate::Error;
use crate::{context::BlockContext, primitive::*, state::StateSource};
use alloy::{
    consensus::TxType,
    primitives::{Address, TxKind, U256},
    providers::{DynProvider, Provider, utils::EIP1559_BASE_FEE_MULTIPLIER},
    rpc::types::{TransactionInput, TransactionRequest},
};
use cfx_rpc_eth_types::DEFAULT_ETH_GAS_CALL_REQUEST;
use cfx_types::{AddressSpaceUtil, Space};
use primitives::{
    SignedTransaction,
    transaction::{
        Action, AuthorizationListItem, Eip155Transaction, Eip1559Transaction, Eip2930Transaction,
        Eip7702Transaction, EthereumTransaction,
    },
};

/// Owns a compatible RPC request until its missing parameters are filled.
pub(crate) struct Preparation {
    request: TransactionRequest,
    sender: Address,
    explicit_type: Option<TxType>,
}

impl Preparation {
    /// Checks the request's representation before any RPC is issued.
    pub(crate) fn new(request: TransactionRequest) -> Result<Self, Error> {
        let (sender, explicit_type) =
            simulation_core::transaction::parse_transaction_input(&request)
                .map_err(Error::InvalidInput)?;
        if explicit_type == Some(TxType::Eip4844)
            || request.blob_versioned_hashes.is_some()
            || request.sidecar.is_some()
            || request.max_fee_per_blob_gas.is_some()
        {
            return Err(Error::InvalidInput(
                "eSpace does not support blob transactions".into(),
            ));
        }
        if request.chain_id.is_some_and(|id| id > u64::from(u32::MAX)) {
            return Err(Error::InvalidInput("chainId exceeds u32".into()));
        }
        if let Some(gas) = request.gas {
            check_simulation_gas(gas)?;
        }
        Ok(Self {
            request,
            sender,
            explicit_type,
        })
    }

    /// Returns the completed request and its VM transaction on the fixed database.
    pub(crate) async fn complete(
        self,
        context: &BlockContext,
        source: &StateSource,
        chain_id: u32,
        dynamic_fees: bool,
    ) -> Result<(TransactionRequest, SignedTransaction), Error> {
        let Self {
            mut request,
            sender,
            explicit_type,
        } = self;
        // For compatible input, London only upgrades legacy/access-list requests
        // without gasPrice to EIP-1559, preserving their existing fields.
        let tx_type = explicit_type.unwrap_or_else(|| {
            if dynamic_fees {
                request.preferred_type()
            } else {
                request.minimal_tx_type()
            }
        });
        request.transaction_type = Some(tx_type as u8);
        request.chain_id.get_or_insert(u64::from(chain_id));
        request.value.get_or_insert(U256::ZERO);
        if request.input.input().is_none() {
            request.input = TransactionInput::new(Default::default());
        }
        if tx_type != TxType::Legacy {
            request.access_list.get_or_insert_default();
        }
        if request.nonce.is_none() {
            request.nonce = Some(source.nonce(sender).await?);
        }

        let provider = &source.espace;
        fill_fees(&mut request, tx_type, provider, context).await?;
        if request.gas.is_none() {
            let gas = provider
                .estimate_gas(request.clone())
                .block(source.anchor.block_id())
                .await
                .map_err(|source| Error::EspaceProvider {
                    operation: "eth_estimateGas",
                    source,
                })?;
            check_simulation_gas(gas)?;
            request.gas = Some(gas);
        }
        let tx = to_signed(&request, sender, tx_type)?;
        Ok((request, tx))
    }
}

// Keep the same resource bound as the node's call interface without using
// sign_call, which also fills defaults and replaces the requested chain ID.
fn check_simulation_gas(gas: u64) -> Result<(), Error> {
    if gas > DEFAULT_ETH_GAS_CALL_REQUEST {
        return Err(Error::Unsupported(format!(
            "gas exceeds the simulation limit of {DEFAULT_ETH_GAS_CALL_REQUEST}"
        )));
    }
    Ok(())
}

/// Fills missing prices without changing explicit prices or checking VM rules.
async fn fill_fees(
    request: &mut TransactionRequest,
    tx_type: TxType,
    provider: &DynProvider,
    context: &BlockContext,
) -> Result<(), Error> {
    if matches!(tx_type, TxType::Legacy | TxType::Eip2930) {
        if request.gas_price.is_none() {
            let price = provider
                .get_gas_price()
                .await
                .map_err(|source| Error::EspaceProvider {
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
                .map_err(|source| Error::EspaceProvider {
                    operation: "eth_maxPriorityFeePerGas",
                    source,
                })?;
            request.max_priority_fee_per_gas = Some(price);
            price
        }
    };
    if request.max_fee_per_gas.is_none() {
        let cap = simulation_core::transaction::fee_cap(
            u128::try_from(context.base_gas_price[Space::Ethereum])
                .map_err(|_| Error::Unsupported("base fee exceeds u128".into()))?,
            priority_fee,
            EIP1559_BASE_FEE_MULTIPLIER,
        )
        .ok_or_else(|| {
            Error::Unsupported(
                "automatic fee cap exceeds the supported range; specify maxFeePerGas".into(),
            )
        })?;
        request.max_fee_per_gas = Some(cap);
    }
    Ok(())
}

fn to_signed(
    request: &TransactionRequest,
    sender: Address,
    tx_type: TxType,
) -> Result<SignedTransaction, Error> {
    let (Some(nonce), Some(chain_id), Some(gas)) = (request.nonce, request.chain_id, request.gas)
    else {
        return Err(Error::Internal(
            "transaction completion omitted nonce, chainId or gas",
        ));
    };
    let chain_id =
        u32::try_from(chain_id).map_err(|_| Error::InvalidInput("chainId exceeds u32".into()))?;
    let nonce = nonce.into();
    let gas = gas.into();
    let action = match request.to.unwrap_or(TxKind::Create) {
        TxKind::Create => Action::Create,
        TxKind::Call(to) => Action::Call(address_to_cfx(to)),
    };
    let value = u256_to_cfx(request.value.unwrap_or_default());
    let data = request.input.input().cloned().unwrap_or_default().to_vec();
    let access_list = access_list_to_cfx(
        request
            .access_list
            .as_ref()
            .map(|list| list.0.as_slice())
            .unwrap_or_default(),
    );
    let tx = match tx_type {
        TxType::Legacy | TxType::Eip2930 => {
            let gas_price = request
                .gas_price
                .ok_or(Error::Internal("transaction completion omitted gasPrice"))?
                .into();
            if tx_type == TxType::Legacy {
                EthereumTransaction::Eip155(Eip155Transaction {
                    nonce,
                    gas_price,
                    gas,
                    action,
                    value,
                    chain_id: Some(chain_id),
                    data,
                })
            } else {
                EthereumTransaction::Eip2930(Eip2930Transaction {
                    chain_id,
                    nonce,
                    gas_price,
                    gas,
                    action,
                    value,
                    data,
                    access_list,
                })
            }
        }
        TxType::Eip1559 | TxType::Eip7702 => {
            let (Some(cap), Some(tip)) =
                (request.max_fee_per_gas, request.max_priority_fee_per_gas)
            else {
                return Err(Error::Internal(
                    "transaction completion omitted dynamic fees",
                ));
            };
            let max_fee_per_gas = cap.into();
            let max_priority_fee_per_gas = tip.into();
            if tx_type == TxType::Eip1559 {
                EthereumTransaction::Eip1559(Eip1559Transaction {
                    chain_id,
                    nonce,
                    max_priority_fee_per_gas,
                    max_fee_per_gas,
                    gas,
                    action,
                    value,
                    data,
                    access_list,
                })
            } else {
                let Action::Call(destination) = action else {
                    return Err(Error::InvalidInput("EIP-7702 requires to".into()));
                };
                let authorization_list = request
                    .authorization_list
                    .iter()
                    .flatten()
                    .map(|auth| AuthorizationListItem {
                        chain_id: u256_to_cfx(*auth.chain_id()),
                        address: address_to_cfx(*auth.address()),
                        nonce: auth.nonce(),
                        y_parity: auth.y_parity(),
                        r: u256_to_cfx(auth.r()),
                        s: u256_to_cfx(auth.s()),
                    })
                    .collect();
                EthereumTransaction::Eip7702(Eip7702Transaction {
                    chain_id,
                    nonce,
                    max_priority_fee_per_gas,
                    max_fee_per_gas,
                    gas,
                    destination,
                    value,
                    data,
                    access_list,
                    authorization_list,
                })
            }
        }
        TxType::Eip4844 => {
            return Err(Error::InvalidInput(
                "eSpace does not support blob transactions".into(),
            ));
        }
    };
    Ok(tx.fake_sign_rpc(address_to_cfx(sender).with_evm_space()))
}
