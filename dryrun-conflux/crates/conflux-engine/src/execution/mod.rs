use cfx_executor::{
    executive::{ExecutionOutcome, ExecutiveContext, TransactOptions},
    machine::Machine,
    state::State,
};
use cfx_statedb::Result as StateDbResult;

mod context;
mod env;
mod params;
mod transaction;

pub use context::{
    EspaceBlockContext, ExecutionBaseFees, ExecutionBlockContext, ExecutionBlockContextError,
    ExecutionConsensusContext, NativePivotBlockContext, build_espace_block_context,
    build_execution_block_context, build_native_pivot_block_context,
};
pub use env::{
    build_execution_spec, build_mainnet_machine, build_rpc_backed_state, build_transaction_env,
};
pub use params::mainnet_common_params;
pub use transaction::{EspaceTransactionInput, signed_transaction_for_dryrun};

pub struct TransactionExecutionInput {
    pub block_context: ExecutionBlockContext,
    pub transaction: EspaceTransactionInput,
}

pub fn execute_transaction(
    state: &mut State,
    machine: &Machine,
    input: TransactionExecutionInput,
) -> StateDbResult<ExecutionOutcome> {
    let tx = signed_transaction_for_dryrun(input.transaction);
    let env = build_transaction_env(machine, state, &tx, input.block_context);
    let spec = build_execution_spec(machine, &env);

    let outcome = ExecutiveContext::new(state, &env, machine, &spec)
        .transact(&tx, TransactOptions::default())?;

    state.update_state_post_tx_execution(!spec.cip645.fix_eip1153);

    if let Some(burnt_fee) = outcome.try_as_executed().and_then(|e| e.burnt_fee) {
        state.burn_by_cip1559(burnt_fee);
    }

    Ok(outcome)
}
