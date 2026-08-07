use alloy::{
    consensus::{BlockHeader, Header, Sealed},
    eips::{BlockId, BlockNumberOrTag},
    primitives::{Address, B256, Bytes, TxKind, U256},
    providers::{Provider, RootProvider, layers::BlockIdProvider},
    rpc::types::{
        AccessList as RpcAccessList, TransactionInput, TransactionRequest as RpcTransactionRequest,
    },
};
pub use simulation_transaction::{AccessListItem, TransactionRequest as EvmTransactionRequest};
use simulation_transaction::{
    Transaction, TransactionRequest, TransactionVariant, TransactionVariantRequest,
};
use thiserror::Error;

mod changes;
mod error;
mod execution;
mod outcome;
mod simulation;
mod simulator;

pub(crate) use changes::EvmNativeChangeError;
pub use error::{EvmSimulationError, EvmSimulationErrorKind};
pub(crate) use execution::{
    EvmBlockAnchor, EvmExecutionError, EvmExecutionObservation, EvmExecutionObserver,
    EvmExecutionOutput, EvmFeeSettlement, EvmStateSource, EvmTransactionExecutor,
};
pub use simulation::{
    EvmBlockContext, EvmExecution, EvmExecutionDetails, EvmExecutionFailure,
    EvmExecutionFailureCode, EvmOutcome, EvmSimulation,
};
pub use simulation_changes::{Change, Erc20Metadata, Erc721CollectionMetadata, NativeMetadata};
pub use simulator::EvmSimulator;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvmBlockSelector {
    Latest,
    Safe,
    Finalized,
    Number(u64),
}

impl EvmBlockSelector {
    fn block_number_or_tag(self) -> BlockNumberOrTag {
        match self {
            Self::Latest => BlockNumberOrTag::Latest,
            Self::Safe => BlockNumberOrTag::Safe,
            Self::Finalized => BlockNumberOrTag::Finalized,
            Self::Number(number) => BlockNumberOrTag::Number(number),
        }
    }
}

#[derive(Debug, Error)]
pub enum EvmPreparationError {
    #[error("block resolution failed: {details}")]
    BlockResolution { details: String },

    #[error("transaction completion failed: {details}")]
    TransactionCompletion { details: String },
}

impl EvmPreparationError {
    fn block_resolution(details: impl Into<String>) -> Self {
        Self::BlockResolution {
            details: details.into(),
        }
    }

    fn transaction_completion(details: impl Into<String>) -> Self {
        Self::TransactionCompletion {
            details: details.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EvmSimulationPreparer {
    provider: RootProvider,
}

impl EvmSimulationPreparer {
    pub fn new(provider: RootProvider) -> Self {
        Self { provider }
    }

    pub async fn prepare_transaction(
        &self,
        block: EvmBlockSelector,
        transaction: TransactionRequest,
    ) -> Result<PreparedEvmSimulation, EvmPreparationError> {
        let block = resolve_block(&self.provider, block).await?;
        let transaction = complete_transaction(transaction, &self.provider, &block).await?;

        Ok(PreparedEvmSimulation { block, transaction })
    }
}

#[derive(Debug, Clone)]
pub struct PreparedEvmSimulation {
    block: Sealed<Header>,
    transaction: Transaction,
}

impl PreparedEvmSimulation {
    pub fn into_parts(self) -> (Sealed<Header>, Transaction) {
        (self.block, self.transaction)
    }
}

async fn resolve_block(
    provider: &RootProvider,
    selector: EvmBlockSelector,
) -> Result<Sealed<Header>, EvmPreparationError> {
    let block = provider
        .get_block_by_number(selector.block_number_or_tag())
        .await
        .map_err(|_| {
            EvmPreparationError::block_resolution("provider request failed while resolving block")
        })?
        .ok_or_else(|| {
            EvmPreparationError::block_resolution("provider did not return the requested block")
        })?;

    let provider_hash = block.hash();
    let header = block.into_consensus_header();
    seal_and_validate_block(header, provider_hash)
}

fn seal_and_validate_block(
    header: Header,
    provider_hash: B256,
) -> Result<Sealed<Header>, EvmPreparationError> {
    let sealed_header = Sealed::new(header);

    if sealed_header.hash() != provider_hash {
        return Err(EvmPreparationError::block_resolution(
            "provider block hash did not match the recomputed header hash",
        ));
    }

    Ok(sealed_header)
}

async fn complete_transaction(
    request: TransactionRequest,
    provider: &RootProvider,
    block: &Sealed<Header>,
) -> Result<Transaction, EvmPreparationError> {
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
                EvmPreparationError::transaction_completion(format!(
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
                EvmPreparationError::transaction_completion(format!(
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
    provider: &RootProvider,
    block: &Sealed<Header>,
    variant: TransactionVariantRequest,
) -> Result<TransactionVariant, EvmPreparationError> {
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
                        EvmPreparationError::transaction_completion(format!(
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
    provider: &RootProvider,
    gas_price: Option<u128>,
) -> Result<u128, EvmPreparationError> {
    match gas_price {
        Some(value) => Ok(value),
        None => provider.get_gas_price().await.map_err(|error| {
            EvmPreparationError::transaction_completion(format!(
                "failed to fetch gas price: {error}"
            ))
        }),
    }
}

fn suggested_dynamic_fee_cap(
    block: &Header,
    max_priority_fee_per_gas: u128,
) -> Result<u128, EvmPreparationError> {
    let base_fee = block.base_fee_per_gas().ok_or_else(|| {
        EvmPreparationError::transaction_completion(format!(
            "block {} does not provide a base fee for dynamic fee completion",
            block.number()
        ))
    })?;

    u128::from(base_fee)
        .checked_mul(2)
        .and_then(|value| value.checked_add(max_priority_fee_per_gas))
        .ok_or_else(|| {
            EvmPreparationError::transaction_completion(
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
