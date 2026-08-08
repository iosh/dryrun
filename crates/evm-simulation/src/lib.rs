use alloy::{
    consensus::{BlockHeader, Header, Sealed},
    eips::BlockId,
    network::Ethereum,
    primitives::{Address, B256, Bytes, TxKind, U256},
    providers::{DynProvider, Provider, layers::BlockIdProvider},
    rpc::types::{
        AccessList as RpcAccessList, TransactionInput, TransactionRequest as RpcTransactionRequest,
    },
};
pub use simulation_transaction::{AccessListItem, TransactionRequest as EvmTransactionRequest};
use simulation_transaction::{
    Transaction, TransactionRequest, TransactionVariant, TransactionVariantRequest,
};

mod chain_spec;
mod changes;
mod context;
mod error;
mod execution;
mod outcome;
mod simulation;
mod simulator;

pub(crate) use chain_spec::EthereumChainSpec;
pub(crate) use changes::EvmNativeChangeError;
pub(crate) use context::resolve_block;
pub(crate) use error::{EvmContextError, EvmTransactionCompletionError};
pub use error::{EvmInitializationError, EvmSimulationError, EvmSimulationErrorKind};
pub(crate) use execution::{
    EvmExecutionError, EvmExecutionObservation, EvmExecutionObserver, EvmExecutionOutput,
    EvmFeeSettlement, EvmTransactionExecutor, create_database,
};
pub use simulation::{
    EvmBlockContext, EvmExecution, EvmExecutionDetails, EvmExecutionFailure,
    EvmExecutionFailureCode, EvmOutcome, EvmSimulation,
};
pub use simulation_changes::{Change, Erc20Metadata, Erc721CollectionMetadata, NativeMetadata};
pub use simulator::EvmTransactionSimulator;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvmBlockSelector {
    Latest,
    Safe,
    Finalized,
    Number(u64),
    Hash(B256),
}

impl std::fmt::Display for EvmBlockSelector {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Latest => formatter.write_str("latest"),
            Self::Safe => formatter.write_str("safe"),
            Self::Finalized => formatter.write_str("finalized"),
            Self::Number(number) => write!(formatter, "number {number}"),
            Self::Hash(hash) => write!(formatter, "hash {hash}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmSimulationRequest {
    pub block: EvmBlockSelector,
    pub transaction: TransactionRequest,
}

async fn complete_transaction(
    request: TransactionRequest,
    provider: &DynProvider<Ethereum>,
    block: &Sealed<Header>,
) -> Result<Transaction, EvmTransactionCompletionError> {
    let TransactionRequest {
        from,
        to,
        nonce,
        gas_limit,
        value,
        data,
        chain_id,
        variant,
    } = request;
    let block_id = BlockId::Hash(block.hash().into());
    let anchored_provider = BlockIdProvider::new(provider.clone(), block_id);
    let nonce = match nonce {
        Some(nonce) => nonce,
        None => anchored_provider
            .get_transaction_count(from)
            .await
            .map_err(|error| {
                EvmTransactionCompletionError::new(format!(
                    "failed to fetch nonce at block {}: {error}",
                    block.number()
                ))
            })?,
    };
    let variant = complete_transaction_variant(provider, block, variant).await?;
    let value = value.unwrap_or(U256::ZERO);
    let data = data.unwrap_or_default();
    let gas_limit = match gas_limit {
        Some(gas_limit) => gas_limit,
        None => anchored_provider
            .estimate_gas(estimation_request(
                from,
                to,
                nonce,
                value,
                data.clone(),
                chain_id,
                &variant,
            ))
            .await
            .map_err(|error| {
                EvmTransactionCompletionError::new(format!(
                    "failed to estimate gas at block {}: {error}",
                    block.number()
                ))
            })?,
    };

    Ok(Transaction {
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

async fn complete_transaction_variant(
    provider: &DynProvider<Ethereum>,
    block: &Sealed<Header>,
    variant: TransactionVariantRequest,
) -> Result<TransactionVariant, EvmTransactionCompletionError> {
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
                        EvmTransactionCompletionError::new(format!(
                            "failed to fetch max priority fee per gas: {error}"
                        ))
                    })?,
            };
            let max_fee_per_gas = match max_fee_per_gas {
                Some(value) => value,
                None => suggested_dynamic_fee_cap(block.inner(), max_priority_fee_per_gas)?,
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
    provider: &DynProvider<Ethereum>,
    gas_price: Option<u128>,
) -> Result<u128, EvmTransactionCompletionError> {
    match gas_price {
        Some(value) => Ok(value),
        None => provider.get_gas_price().await.map_err(|error| {
            EvmTransactionCompletionError::new(format!("failed to fetch gas price: {error}"))
        }),
    }
}

fn suggested_dynamic_fee_cap(
    block: &Header,
    max_priority_fee_per_gas: u128,
) -> Result<u128, EvmTransactionCompletionError> {
    let base_fee = block.base_fee_per_gas().ok_or_else(|| {
        EvmTransactionCompletionError::new(format!(
            "block {} does not provide a base fee for dynamic fee completion",
            block.number()
        ))
    })?;

    u128::from(base_fee)
        .checked_mul(2)
        .and_then(|value| value.checked_add(max_priority_fee_per_gas))
        .ok_or_else(|| {
            EvmTransactionCompletionError::new(
                "calculated dynamic fee exceeds the simulator maximum \
                 340282366920938463463374607431768211455",
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
    variant: &TransactionVariant,
) -> RpcTransactionRequest {
    let mut request = RpcTransactionRequest {
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
    RpcAccessList(items.to_vec())
}
