use cfx_executor::{
    executive::{ChargeCollateral, ExecutiveContext, TransactOptions, TransactSettings},
    machine::Machine,
    state::State,
};
use cfx_types::Space;
use cfx_vm_types::{Env, Spec};
use primitives::SignedTransaction;

mod context;
mod env;
mod observer;
mod outcome;
mod params;
mod transaction;

pub use context::{
    CoreSpacePivotBlockContext, EspaceBlockContext, ExecutionBaseFees, ExecutionBlockContext,
    ExecutionBlockContextError, ExecutionConsensusContext,
};
pub(crate) use context::{
    build_core_space_pivot_block_context, build_espace_block_context, build_execution_block_context,
};
pub(crate) use env::build_rpc_backed_state;
pub use env::{build_execution_spec, build_mainnet_machine, build_transaction_env};
pub(crate) use observer::Observation;
pub(crate) use outcome::{
    ExecutedTransactionDetails, TransactionExecutionError, TransactionExecutionOutcome,
};
pub use params::mainnet_common_params;
pub use transaction::{
    CoreSpaceTransactionInput, DryRunTransactionInput, EspaceTransactionInput,
    signed_transaction_for_dryrun,
};

pub(crate) struct TransactionExecutionInput {
    pub(crate) block_context: ExecutionBlockContext,
    pub(crate) transaction: DryRunTransactionInput,
}

pub(crate) struct PreparedTransactionExecution {
    pub(crate) transaction: SignedTransaction,
    pub(crate) env: Env,
    pub(crate) spec: Spec,
}

pub(crate) fn prepare_transaction_execution(
    state: &State,
    machine: &Machine,
    input: TransactionExecutionInput,
) -> Result<PreparedTransactionExecution, ExecutionBlockContextError> {
    let transaction = signed_transaction_for_dryrun(input.transaction);
    let env = build_transaction_env(machine, state, &transaction, &input.block_context)?;
    let spec = build_execution_spec(machine, &env);

    Ok(PreparedTransactionExecution {
        transaction,
        env,
        spec,
    })
}

pub(crate) fn execute_transaction(
    state: &mut State,
    machine: &Machine,
    prepared: &PreparedTransactionExecution,
) -> Result<TransactionExecutionOutcome, TransactionExecutionError> {
    let options = transact_options_for(prepared.transaction.space());

    let outcome = ExecutiveContext::new(state, &prepared.env, machine, &prepared.spec)
        .transact(&prepared.transaction, options)?;

    state.update_state_post_tx_execution(!prepared.spec.cip645.fix_eip1153);

    if let Some(burnt_fee) = outcome.try_as_executed().and_then(|e| e.burnt_fee) {
        state.burn_by_cip1559(burnt_fee);
    }

    TransactionExecutionOutcome::from_upstream(outcome)
}

fn transact_options_for(space: Space) -> TransactOptions<observer::ObservationObserver> {
    let mut options = TransactOptions {
        observer: observer::ObservationObserver::new(space),
        settings: TransactSettings::all_checks(),
    };

    if space == Space::Native {
        // Public Core Space RPC returns storage values without storage owners.
        // Use estimate mode so collateral checks do not fail incorrectly.
        options.settings.charge_collateral = ChargeCollateral::EstimateSender;
    }

    options
}
