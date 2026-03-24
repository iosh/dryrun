mod error;
mod types;

use std::sync::Arc;

use evm_engine::{EvmEngine, EvmExecutionInput, EvmExecutionOutput};

pub use error::SimulationServiceError;
pub use types::{
    AccessListItem, BlockRef, EvmTransaction, EvmTransactionType, RawLog,
    SimulateEvmTransactionInput, SimulateEvmTransactionOutput, SimulatedBlock, SimulationFailure,
    SimulationOptions, SimulationStatus,
};

#[derive(Debug, Clone)]
pub struct SimulationService {
    evm_engine: Arc<EvmEngine>,
}

impl SimulationService {
    pub fn new(evm_engine: Arc<EvmEngine>) -> Self {
        Self { evm_engine }
    }

    pub async fn simulate_evm_transaction(
        &self,
        input: SimulateEvmTransactionInput,
    ) -> Result<SimulateEvmTransactionOutput, SimulationServiceError> {
        let SimulateEvmTransactionInput {
            block,
            transaction,
            options,
        } = input;
        let include_logs = options.include_logs;
        let output = self
            .evm_engine
            .simulate(EvmExecutionInput {
                block: block.into(),
                transaction: transaction.into(),
            })
            .await?;

        Ok(SimulateEvmTransactionOutput::from_engine_output(
            output,
            include_logs,
        ))
    }
}

impl From<BlockRef> for evm_engine::BlockRef {
    fn from(block: BlockRef) -> Self {
        match block {
            BlockRef::Latest => Self::Latest,
            BlockRef::Number(number) => Self::Number(number),
            BlockRef::Hash(hash) => Self::Hash(hash),
        }
    }
}

impl From<EvmTransactionType> for evm_engine::EvmTransactionType {
    fn from(tx_type: EvmTransactionType) -> Self {
        match tx_type {
            EvmTransactionType::Legacy => Self::Legacy,
            EvmTransactionType::AccessList => Self::AccessList,
            EvmTransactionType::DynamicFee => Self::DynamicFee,
        }
    }
}

impl From<AccessListItem> for evm_engine::AccessListItem {
    fn from(item: AccessListItem) -> Self {
        Self {
            address: item.address,
            storage_keys: item.storage_keys,
        }
    }
}

impl From<EvmTransaction> for evm_engine::EvmTransaction {
    fn from(transaction: EvmTransaction) -> Self {
        Self {
            tx_type: transaction.tx_type.into(),
            chain_id: transaction.chain_id,
            from: transaction.from,
            to: transaction.to,
            nonce: transaction.nonce,
            gas_limit: transaction.gas_limit,
            value: transaction.value,
            data: transaction.data,
            access_list: transaction
                .access_list
                .into_iter()
                .map(Into::into)
                .collect(),
            gas_price: transaction.gas_price,
            max_fee_per_gas: transaction.max_fee_per_gas,
            max_priority_fee_per_gas: transaction.max_priority_fee_per_gas,
        }
    }
}

impl From<evm_engine::EvmExecutionStatus> for SimulationStatus {
    fn from(status: evm_engine::EvmExecutionStatus) -> Self {
        match status {
            evm_engine::EvmExecutionStatus::Success => Self::Success,
            evm_engine::EvmExecutionStatus::Failed => Self::Failed,
        }
    }
}

impl From<evm_engine::SimulatedBlock> for SimulatedBlock {
    fn from(block: evm_engine::SimulatedBlock) -> Self {
        Self {
            number: block.number,
            hash: block.hash,
        }
    }
}

impl From<evm_engine::EvmExecutionFailure> for SimulationFailure {
    fn from(failure: evm_engine::EvmExecutionFailure) -> Self {
        Self {
            code: failure.code,
            message: failure.message,
            reason: failure.reason,
        }
    }
}

impl From<evm_engine::EvmExecutionLog> for RawLog {
    fn from(log: evm_engine::EvmExecutionLog) -> Self {
        Self {
            log_index: log.log_index,
            address: log.address,
            topics: log.topics,
            data: log.data,
        }
    }
}

impl SimulateEvmTransactionOutput {
    fn from_engine_output(output: EvmExecutionOutput, include_logs: bool) -> Self {
        let EvmExecutionOutput {
            chain_id,
            block,
            status,
            gas_used,
            gas_limit,
            output,
            failure,
            logs,
        } = output;

        Self {
            chain_id,
            block: block.into(),
            status: status.into(),
            gas_used,
            gas_limit,
            output,
            failure: failure.map(Into::into),
            logs: if include_logs {
                logs.into_iter().map(Into::into).collect()
            } else {
                Vec::new()
            },
        }
    }
}
