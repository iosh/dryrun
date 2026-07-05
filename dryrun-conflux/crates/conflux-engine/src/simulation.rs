use cfx_bytes::Bytes;
use cfx_executor::executive::{ExecutionError, ExecutionOutcome};
use cfx_types::{H256, U256};
use cfx_vm_types as vm;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EspaceExecutionStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedBlock {
    pub number: u64,
    pub hash: H256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceExecutionFailure {
    pub code: String,
    pub message: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceExecution {
    pub chain_id: u64,
    pub block: SimulatedBlock,
    pub status: EspaceExecutionStatus,
    pub gas_used: U256,
    pub gas_limit: U256,
    pub gas_charged: U256,
    pub fee: U256,
    pub burnt_fee: Option<U256>,
    pub output: Bytes,
    pub failure: Option<EspaceExecutionFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceSimulation {
    execution: EspaceExecution,
}

impl EspaceSimulation {
    pub fn new(execution: EspaceExecution) -> Self {
        Self { execution }
    }

    pub fn execution(&self) -> &EspaceExecution {
        &self.execution
    }

    pub fn into_execution(self) -> EspaceExecution {
        self.execution
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeExecutionStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeExecutionFailure {
    pub code: String,
    pub message: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeExecution {
    pub chain_id: u64,
    pub status: NativeExecutionStatus,
    pub gas_used: U256,
    pub gas_limit: U256,
    pub gas_charged: U256,
    pub fee: U256,
    pub burnt_fee: Option<U256>,
    pub output: Bytes,
    pub failure: Option<NativeExecutionFailure>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSimulation {
    execution: NativeExecution,
}
impl NativeSimulation {
    pub fn new(execution: NativeExecution) -> Self {
        Self { execution }
    }

    pub fn execution(&self) -> &NativeExecution {
        &self.execution
    }

    pub fn into_execution(self) -> NativeExecution {
        self.execution
    }
}
pub(crate) fn build_espace_execution(
    chain_id: u32,
    block: SimulatedBlock,
    gas_limit: U256,
    outcome: ExecutionOutcome,
) -> EspaceExecution {
    let failure = build_failure(&outcome);
    let status = if failure.is_some() {
        EspaceExecutionStatus::Failed
    } else {
        EspaceExecutionStatus::Success
    };

    let executed = outcome.try_into_executed();

    EspaceExecution {
        chain_id: u64::from(chain_id),
        block,
        status,
        gas_used: executed
            .as_ref()
            .map(|executed| executed.gas_used)
            .unwrap_or_else(U256::zero),
        gas_limit,
        gas_charged: executed
            .as_ref()
            .map(|executed| executed.gas_charged)
            .unwrap_or_else(U256::zero),
        fee: executed
            .as_ref()
            .map(|executed| executed.fee)
            .unwrap_or_else(U256::zero),
        burnt_fee: executed.as_ref().and_then(|executed| executed.burnt_fee),
        output: executed.map(|executed| executed.output).unwrap_or_default(),
        failure,
    }
}
pub(crate) fn build_native_execution(
    chain_id: u32,
    gas_limit: U256,
    outcome: ExecutionOutcome,
) -> NativeExecution {
    let failure = build_native_failure(&outcome);
    let status = if failure.is_some() {
        NativeExecutionStatus::Failed
    } else {
        NativeExecutionStatus::Success
    };

    let executed = outcome.try_into_executed();

    NativeExecution {
        chain_id: u64::from(chain_id),
        status,
        gas_used: executed
            .as_ref()
            .map(|executed| executed.gas_used)
            .unwrap_or_else(U256::zero),
        gas_limit,
        gas_charged: executed
            .as_ref()
            .map(|executed| executed.gas_charged)
            .unwrap_or_else(U256::zero),
        fee: executed
            .as_ref()
            .map(|executed| executed.fee)
            .unwrap_or_else(U256::zero),
        burnt_fee: executed.as_ref().and_then(|executed| executed.burnt_fee),
        output: executed.map(|executed| executed.output).unwrap_or_default(),
        failure,
    }
}

fn build_failure(outcome: &ExecutionOutcome) -> Option<EspaceExecutionFailure> {
    match outcome {
        ExecutionOutcome::Finished(_) => None,
        ExecutionOutcome::ExecutionErrorBumpNonce(error, executed) => Some(
            build_execution_error_failure(error, executed.output.as_ref()),
        ),
        ExecutionOutcome::NotExecutedDrop(_)
        | ExecutionOutcome::NotExecutedToReconsiderPacking(_) => Some(EspaceExecutionFailure {
            code: "TRANSACTION_NOT_EXECUTED".to_string(),
            message: outcome.error_message(),
            reason: None,
        }),
    }
}

fn build_native_failure(outcome: &ExecutionOutcome) -> Option<NativeExecutionFailure> {
    build_failure(outcome).map(|failure| NativeExecutionFailure {
        code: failure.code,
        message: failure.message,
        reason: failure.reason,
    })
}
fn build_execution_error_failure(error: &ExecutionError, output: &[u8]) -> EspaceExecutionFailure {
    if error == &ExecutionError::VmError(vm::Error::Reverted) {
        return EspaceExecutionFailure {
            code: "REVERT".to_string(),
            message: "execution reverted".to_string(),
            reason: revert_reason(output),
        };
    }

    EspaceExecutionFailure {
        code: "EXECUTION_FAILED".to_string(),
        message: format!("{error:?}"),
        reason: None,
    }
}

// TODO
fn revert_reason(_output: &[u8]) -> Option<String> {
    None
}
