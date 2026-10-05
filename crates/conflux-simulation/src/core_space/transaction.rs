use cfx_rpc_cfx_types::{DEFAULT_CFX_GAS_CALL_REQUEST, TransactionRequest};
use cfx_types::{Address, AddressSpaceUtil, Space, U256};
use conflux_provider::Network;
use primitives::{
    SignedTransaction,
    transaction::{
        Action, Cip1559Transaction, Cip2930Transaction, NativeTransaction, TypedNativeTransaction,
    },
};

use crate::{Error, context::BlockContext, primitive::u256_to_cfx, state::StateSource};

pub(crate) struct Preparation {
    request: TransactionRequest,
    sender: Address,
}

impl Preparation {
    pub fn new(request: TransactionRequest, network: Network) -> Result<Self, Error> {
        let sender = request
            .from
            .as_ref()
            .ok_or_else(|| Error::InvalidInput("from is required".into()))?
            .hex_address;
        for address in request.from.iter().chain(request.to.iter()).chain(
            request
                .access_list
                .iter()
                .flatten()
                .map(|item| &item.address),
        ) {
            if address.network != network.into() {
                return Err(Error::InvalidInput(
                    "transaction address belongs to another network".into(),
                ));
            }
        }
        if request.chain_id.is_some_and(|id| id > u32::MAX.into()) {
            return Err(Error::InvalidInput("chainId exceeds u32".into()));
        }
        if request
            .epoch_height
            .is_some_and(|epoch| epoch > u64::MAX.into())
        {
            return Err(Error::InvalidInput("epochHeight exceeds u64".into()));
        }
        if let Some(gas) = request.gas {
            check_gas(gas)?;
        }
        let dynamic =
            request.max_fee_per_gas.is_some() || request.max_priority_fee_per_gas.is_some();
        if request.gas_price.is_some() && dynamic {
            return Err(Error::InvalidInput(
                "gasPrice cannot be combined with dynamic fees".into(),
            ));
        }
        if let Some(tx_type) = request.transaction_type {
            match tx_type.as_u64() {
                0 if request.access_list.is_none() && !dynamic => (),
                1 if !dynamic => (),
                2 if request.gas_price.is_none() => (),
                0..=2 => {
                    return Err(Error::InvalidInput(
                        "transaction fields are incompatible with type".into(),
                    ));
                }
                _ => {
                    return Err(Error::InvalidInput(
                        "Core transaction type must be 0, 1 or 2".into(),
                    ));
                }
            }
        }
        Ok(Self { request, sender })
    }

    pub async fn complete(
        self,
        context: &BlockContext,
        source: &StateSource,
        chain_id: u32,
        dynamic_fees: bool,
    ) -> Result<(TransactionRequest, SignedTransaction), Error> {
        let Self {
            mut request,
            sender,
        } = self;
        let tx_type = request
            .transaction_type
            .map(|t| t.as_u64())
            .unwrap_or_else(|| {
                if request.max_fee_per_gas.is_some()
                    || request.max_priority_fee_per_gas.is_some()
                    || (dynamic_fees && request.gas_price.is_none())
                {
                    2
                } else if request.access_list.is_some() {
                    1
                } else {
                    0
                }
            });
        request.transaction_type = Some(tx_type.into());
        request.chain_id.get_or_insert(chain_id.into());
        request
            .epoch_height
            .get_or_insert(source.anchor.epoch.into());
        request.value.get_or_insert_default();
        request.data.get_or_insert_with(|| Vec::new().into());
        if tx_type != 0 {
            request.access_list.get_or_insert_default();
        }
        if request.nonce.is_none() {
            request.nonce = Some(source.core_nonce(sender).await?);
        }
        if tx_type < 2 {
            if request.gas_price.is_none() {
                request.gas_price = Some(u256_to_cfx(source.core.cfx_gas_price().await?));
            }
        } else {
            let tip = match request.max_priority_fee_per_gas {
                Some(tip) => tip,
                None => {
                    let tip = u256_to_cfx(source.core.cfx_max_priority_fee_per_gas().await?);
                    request.max_priority_fee_per_gas = Some(tip);
                    tip
                }
            };
            if request.max_fee_per_gas.is_none() {
                request.max_fee_per_gas = Some(
                    context.base_gas_price[Space::Native]
                        .checked_add(tip)
                        .ok_or_else(|| {
                            Error::Unsupported(
                                "automatic fee cap exceeds U256; specify maxFeePerGas".into(),
                            )
                        })?,
                );
            }
        }
        if request.gas.is_none() || request.storage_limit.is_none() {
            let estimate = source
                .core
                .cfx_estimate_gas_and_collateral(request.clone(), source.anchor.epoch_number())
                .await?;
            if request.gas.is_none() {
                let gas = u256_to_cfx(estimate.gas_limit);
                check_gas(gas)?;
                request.gas = Some(gas);
            }
            if request.storage_limit.is_none() {
                let limit: u64 = estimate
                    .storage_collateralized
                    .try_into()
                    .map_err(|_| Error::Unsupported("estimated storageLimit exceeds u64".into()))?;
                request.storage_limit = Some(limit.into());
            }
        }
        let tx = to_signed(&request, sender, tx_type)?;
        Ok((request, tx))
    }
}

fn check_gas(gas: U256) -> Result<(), Error> {
    if gas > DEFAULT_CFX_GAS_CALL_REQUEST.into() {
        return Err(Error::Unsupported(format!(
            "gas exceeds the simulation limit of {DEFAULT_CFX_GAS_CALL_REQUEST}"
        )));
    }
    Ok(())
}

fn to_signed(
    request: &TransactionRequest,
    sender: Address,
    tx_type: u64,
) -> Result<SignedTransaction, Error> {
    let (Some(nonce), Some(gas), Some(storage_limit), Some(epoch_height), Some(chain_id)) = (
        request.nonce,
        request.gas,
        request.storage_limit,
        request.epoch_height,
        request.chain_id,
    ) else {
        return Err(Error::Internal(
            "transaction completion omitted required fields",
        ));
    };
    // Representation ranges were checked before completion; defaults have the same widths.
    let chain_id = chain_id.as_u32();
    let epoch_height = epoch_height.as_u64();
    let storage_limit = storage_limit.as_u64();
    let action = request
        .to
        .as_ref()
        .map_or(Action::Create, |to| Action::Call(to.hex_address));
    let value = request.value.unwrap_or_default();
    let data = request
        .data
        .as_ref()
        .map(|bytes| bytes.0.clone())
        .unwrap_or_default();
    let access_list = request
        .access_list
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(Into::into)
        .collect();
    let transaction = match tx_type {
        0 | 1 => {
            let gas_price = request
                .gas_price
                .ok_or(Error::Internal("transaction completion omitted gasPrice"))?;
            if tx_type == 0 {
                TypedNativeTransaction::Cip155(NativeTransaction {
                    nonce,
                    gas_price,
                    gas,
                    action,
                    value,
                    storage_limit,
                    epoch_height,
                    chain_id,
                    data,
                })
            } else {
                TypedNativeTransaction::Cip2930(Cip2930Transaction {
                    nonce,
                    gas_price,
                    gas,
                    action,
                    value,
                    storage_limit,
                    epoch_height,
                    chain_id,
                    data,
                    access_list,
                })
            }
        }
        2 => {
            let (Some(max_fee_per_gas), Some(max_priority_fee_per_gas)) =
                (request.max_fee_per_gas, request.max_priority_fee_per_gas)
            else {
                return Err(Error::Internal(
                    "transaction completion omitted dynamic fees",
                ));
            };
            TypedNativeTransaction::Cip1559(Cip1559Transaction {
                nonce,
                max_priority_fee_per_gas,
                max_fee_per_gas,
                gas,
                action,
                value,
                storage_limit,
                epoch_height,
                chain_id,
                data,
                access_list,
            })
        }
        _ => return Err(Error::Internal("unvalidated Core transaction type")),
    };
    Ok(transaction.fake_sign_rpc(sender.with_native_space()))
}
