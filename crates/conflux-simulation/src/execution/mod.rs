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
pub(crate) use env::build_conflux_state;
pub use env::{build_execution_spec, build_mainnet_machine, build_transaction_env};
pub(crate) use observer::{Observation, ObservationObserver};
pub(crate) use outcome::{
    ConfluxExecutionOutput, TransactionExecutionError, TransactionExecutionOutcome,
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

pub(crate) struct ConfluxTransactionExecution {
    pub(crate) prepared: PreparedTransactionExecution,
    pub(crate) outcome: TransactionExecutionOutcome,
}

pub(crate) struct ConfluxTransactionExecutor<'a> {
    state: &'a mut State,
    machine: &'a Machine,
}

impl<'a> ConfluxTransactionExecutor<'a> {
    pub(crate) fn new(state: &'a mut State, machine: &'a Machine) -> Self {
        Self { state, machine }
    }

    pub(crate) fn execute(
        self,
        input: TransactionExecutionInput,
        observer: ObservationObserver,
    ) -> Result<ConfluxTransactionExecution, TransactionExecutionError> {
        let prepared = self.prepare(input)?;
        let options = transact_options_for(prepared.transaction.space(), observer);

        let outcome =
            ExecutiveContext::new(self.state, &prepared.env, self.machine, &prepared.spec)
                .transact(&prepared.transaction, options)?;

        self.state
            .update_state_post_tx_execution(!prepared.spec.cip645.fix_eip1153);

        if let Some(burnt_fee) = outcome.try_as_executed().and_then(|e| e.burnt_fee) {
            self.state.burn_by_cip1559(burnt_fee);
        }

        Ok(ConfluxTransactionExecution {
            prepared,
            outcome: TransactionExecutionOutcome::from_upstream(outcome)?,
        })
    }

    fn prepare(
        &self,
        input: TransactionExecutionInput,
    ) -> Result<PreparedTransactionExecution, ExecutionBlockContextError> {
        let transaction = signed_transaction_for_dryrun(input.transaction);
        let env =
            build_transaction_env(self.machine, self.state, &transaction, &input.block_context)?;
        let spec = build_execution_spec(self.machine, &env);

        Ok(PreparedTransactionExecution {
            transaction,
            env,
            spec,
        })
    }
}

fn transact_options_for(
    space: Space,
    observer: ObservationObserver,
) -> TransactOptions<ObservationObserver> {
    let mut options = TransactOptions {
        observer,
        settings: TransactSettings::all_checks(),
    };

    if space == Space::Native {
        // Public Core Space RPC returns storage values without storage owners.
        // Use estimate mode so collateral checks do not fail incorrectly.
        options.settings.charge_collateral = ChargeCollateral::EstimateSender;
    }

    options
}
