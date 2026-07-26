use cfx_executor::{
    executive::{ChargeCollateral, ExecutiveContext, TransactOptions, TransactSettings},
    machine::Machine,
    state::State,
};
use cfx_types::Space;

mod context;
mod env;
mod observer;
mod outcome;
mod params;
mod transaction;

pub use context::{
    CoreSpacePivotBlockContext, EspaceBlockContext, ExecutionBaseFees, ExecutionBlockContext,
    ExecutionBlockContextError, ExecutionConsensusContext, build_core_space_pivot_block_context,
    build_espace_block_context, build_execution_block_context,
};
pub(crate) use env::build_rpc_backed_state;
pub use env::{build_execution_spec, build_mainnet_machine, build_transaction_env};
pub(crate) use outcome::{TransactionExecutionError, TransactionExecutionOutcome};
pub use params::mainnet_common_params;
pub use transaction::{
    CoreSpaceTransactionInput, DryRunTransactionInput, EspaceTransactionInput,
    signed_transaction_for_dryrun,
};

pub(crate) struct TransactionExecutionInput {
    pub(crate) block_context: ExecutionBlockContext,
    pub(crate) transaction: DryRunTransactionInput,
}

pub(crate) fn execute_transaction(
    state: &mut State,
    machine: &Machine,
    input: TransactionExecutionInput,
) -> Result<TransactionExecutionOutcome, TransactionExecutionError> {
    let options = transact_options_for(&input.transaction);
    let tx = signed_transaction_for_dryrun(input.transaction);
    let env = build_transaction_env(machine, state, &tx, &input.block_context);
    let spec = build_execution_spec(machine, &env);

    let outcome = ExecutiveContext::new(state, &env, machine, &spec).transact(&tx, options)?;

    state.update_state_post_tx_execution(!spec.cip645.fix_eip1153);

    if let Some(burnt_fee) = outcome.try_as_executed().and_then(|e| e.burnt_fee) {
        state.burn_by_cip1559(burnt_fee);
    }

    TransactionExecutionOutcome::from_upstream(outcome)
}

fn transact_options_for(
    transaction: &DryRunTransactionInput,
) -> TransactOptions<observer::ObservationObserver> {
    let mut options = TransactOptions {
        observer: observer::ObservationObserver::new(match transaction {
            DryRunTransactionInput::Espace(_) => Space::Ethereum,
            DryRunTransactionInput::CoreSpace(_) => Space::Native,
        }),
        settings: TransactSettings::all_checks(),
    };

    if matches!(transaction, DryRunTransactionInput::CoreSpace(_)) {
        // Public Core Space RPC returns storage values without storage owners.
        // Use estimate mode so collateral checks do not fail incorrectly.
        options.settings.charge_collateral = ChargeCollateral::EstimateSender;
    }

    options
}
