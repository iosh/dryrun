use alloy::{
    eips::{BlockId, BlockNumberOrTag},
    primitives::{Address, Bytes, TxKind, U256},
    providers::{Provider, RootProvider},
    rpc::types::{
        AccessList as RpcAccessList, AccessListItem as RpcAccessListItem, TransactionInput,
        TransactionRequest,
    },
};
use evm_engine::{AccessListItem, EvmTransaction, EvmTransactionVariant, ResolvedBlock};
use simulation_transaction::{TransactionVariant, TransactionVariantRequest};

use crate::SimulationServiceError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmTransactionRequest {
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: Option<u64>,
    pub gas_limit: Option<u64>,
    pub value: Option<U256>,
    pub input: Option<Bytes>,
    pub chain_id: u64,
    pub variant: TransactionVariantRequest<u128, AccessListItem>,
}

impl EvmTransactionRequest {
    pub(crate) async fn resolve(
        self,
        provider: &RootProvider,
        block: &ResolvedBlock,
    ) -> Result<EvmTransaction, SimulationServiceError> {
        let Self {
            from,
            to,
            nonce,
            gas_limit,
            value,
            input,
            chain_id,
            variant,
        } = self;
        let block_id = BlockId::Number(BlockNumberOrTag::Number(block.number()));
        let nonce = match nonce {
            Some(nonce) => nonce,
            None => provider
                .get_transaction_count(from)
                .block_id(block_id)
                .await
                .map_err(|error| {
                    SimulationServiceError::transaction_resolution(format!(
                        "failed to resolve nonce at block {}: {error}",
                        block.number()
                    ))
                })?,
        };
        let variant = resolve_fees(provider, block, variant).await?;
        let value = value.unwrap_or(U256::ZERO);
        let data = input.unwrap_or_default();
        let gas_limit = match gas_limit {
            Some(gas_limit) => gas_limit,
            None => provider
                .estimate_gas(estimation_request(
                    from,
                    to,
                    nonce,
                    value,
                    data.clone(),
                    chain_id,
                    &variant,
                ))
                .block(block_id)
                .await
                .map_err(|error| {
                    SimulationServiceError::transaction_resolution(format!(
                        "failed to estimate gas at block {}: {error}",
                        block.number()
                    ))
                })?,
        };

        Ok(EvmTransaction {
            chain_id,
            from,
            to,
            nonce,
            gas_limit,
            value,
            data,
            variant,
        })
    }
}

async fn resolve_fees(
    provider: &RootProvider,
    block: &ResolvedBlock,
    variant: TransactionVariantRequest<u128, AccessListItem>,
) -> Result<EvmTransactionVariant, SimulationServiceError> {
    match variant {
        TransactionVariantRequest::Legacy { gas_price } => Ok(TransactionVariant::Legacy {
            gas_price: suggested_gas_price(provider, gas_price).await?,
        }),
        TransactionVariantRequest::AccessList {
            gas_price,
            access_list,
        } => Ok(TransactionVariant::AccessList {
            gas_price: suggested_gas_price(provider, gas_price).await?,
            access_list,
        }),
        TransactionVariantRequest::DynamicFee {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        } => {
            let max_priority_fee_per_gas = match max_priority_fee_per_gas {
                Some(value) => value,
                None => provider
                    .get_max_priority_fee_per_gas()
                    .await
                    .map_err(|error| {
                        SimulationServiceError::transaction_resolution(format!(
                            "failed to resolve max priority fee per gas: {error}"
                        ))
                    })?,
            };
            let max_fee_per_gas = match max_fee_per_gas {
                Some(value) => value,
                None => suggested_dynamic_fee_cap(block, max_priority_fee_per_gas)?,
            };

            Ok(TransactionVariant::DynamicFee {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list,
            })
        }
    }
}

async fn suggested_gas_price(
    provider: &RootProvider,
    gas_price: Option<u128>,
) -> Result<u128, SimulationServiceError> {
    match gas_price {
        Some(value) => Ok(value),
        None => provider.get_gas_price().await.map_err(|error| {
            SimulationServiceError::transaction_resolution(format!(
                "failed to resolve gas price: {error}"
            ))
        }),
    }
}

fn suggested_dynamic_fee_cap(
    block: &ResolvedBlock,
    max_priority_fee_per_gas: u128,
) -> Result<u128, SimulationServiceError> {
    let base_fee = block.base_fee_per_gas().ok_or_else(|| {
        SimulationServiceError::transaction_resolution(format!(
            "block {} does not provide a base fee for dynamic fee resolution",
            block.number()
        ))
    })?;

    u128::from(base_fee)
        .checked_mul(2)
        .and_then(|value| value.checked_add(max_priority_fee_per_gas))
        .ok_or_else(|| {
            SimulationServiceError::transaction_resolution(
                "resolved dynamic fee exceeds an unsigned 128-bit integer",
            )
        })
}

fn estimation_request(
    from: Address,
    to: Option<Address>,
    nonce: u64,
    value: U256,
    input: Bytes,
    chain_id: u64,
    variant: &EvmTransactionVariant,
) -> TransactionRequest {
    let mut request = TransactionRequest {
        from: Some(from),
        to: Some(to.map_or(TxKind::Create, TxKind::Call)),
        value: Some(value),
        input: TransactionInput::new(input),
        nonce: Some(nonce),
        chain_id: Some(chain_id),
        ..Default::default()
    };

    match variant {
        TransactionVariant::Legacy { gas_price } => {
            request.transaction_type = Some(0);
            request.gas_price = Some(*gas_price);
        }
        TransactionVariant::AccessList {
            gas_price,
            access_list,
        } => {
            request.transaction_type = Some(1);
            request.gas_price = Some(*gas_price);
            request.access_list = Some(rpc_access_list(access_list));
        }
        TransactionVariant::DynamicFee {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        } => {
            request.transaction_type = Some(2);
            request.max_fee_per_gas = Some(*max_fee_per_gas);
            request.max_priority_fee_per_gas = Some(*max_priority_fee_per_gas);
            request.access_list = Some(rpc_access_list(access_list));
        }
    }

    request
}

fn rpc_access_list(items: &[AccessListItem]) -> RpcAccessList {
    RpcAccessList(
        items
            .iter()
            .map(|item| RpcAccessListItem {
                address: item.address,
                storage_keys: item.storage_keys.clone(),
            })
            .collect(),
    )
}
