use crate::{EvmEngineError, EvmExecutionInput, EvmExecutionOutput};

pub(crate) async fn simulate_latest_dynamic_fee(
    _rpc_url: &str,
    _input: EvmExecutionInput,
) -> Result<EvmExecutionOutput, EvmEngineError> {
    Err(EvmEngineError::not_ready(
        "execution for block.tag=latest and transaction.type=0x2 is not implemented",
    ))
}
