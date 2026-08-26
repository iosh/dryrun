use alloy::primitives::{Address, Bytes, U256};
use revm::{
    ExecuteEvm,
    context::TxEnv,
    context_interface::{
        result::{EVMError, ExecutionResult},
        transaction::{AccessList as RevmAccessList, TransactionType},
    },
    primitives::TxKind,
};

use crate::EvmStateAccessError;

use super::{EvmReadCallContext, EvmReadCallResult, EvmStateReadError, MainnetEvm};

const READ_CALL_GAS_LIMIT: u64 = 100_000;

pub(super) fn execute_read_call(
    evm: &mut MainnetEvm,
    context: EvmReadCallContext,
    target: Address,
    data: Bytes,
) -> Result<EvmReadCallResult, EvmStateReadError> {
    let result = evm
        .transact(build_read_call_tx(context, target, data))
        .map_err(|error| match error {
            EVMError::Database(source) => {
                EvmStateReadError::from(EvmStateAccessError::from(source))
            }
            error => EvmStateReadError::ReadCallExecution {
                details: error.to_string(),
            },
        })?;

    Ok(match result.result {
        ExecutionResult::Success { output, .. } => EvmReadCallResult::Success(output.into_data()),
        ExecutionResult::Revert { output, .. } => EvmReadCallResult::Reverted(output),
        ExecutionResult::Halt { reason, .. } => EvmReadCallResult::Halted(reason),
    })
}

fn build_read_call_tx(context: EvmReadCallContext, target: Address, data: Bytes) -> TxEnv {
    TxEnv {
        tx_type: TransactionType::Legacy as u8,
        caller: context.caller,
        gas_limit: READ_CALL_GAS_LIMIT,
        gas_price: 0,
        kind: TxKind::Call(target),
        value: U256::ZERO,
        data,
        nonce: context.nonce,
        chain_id: Some(context.chain_id),
        access_list: RevmAccessList::default(),
        gas_priority_fee: None,
        blob_hashes: Vec::new(),
        max_fee_per_blob_gas: 0,
        authorization_list: Vec::new(),
    }
}
