use crate::{Error, tracer::Calls};
use cfx_executor::{
    executive::{ExecutionOutcome, ExecutiveContext, TransactOptions, TransactSettings},
    machine::Machine,
    state::State,
};
use cfx_vm_types::{Env, Spec};
use primitives::SignedTransaction;

/// The only formal local execution of the user's transaction.
pub(crate) fn transact(
    state: &mut State,
    machine: &Machine,
    env: &Env,
    spec: &Spec,
    tx: &SignedTransaction,
    calls: &mut Calls<'_>,
) -> Result<ExecutionOutcome, Error> {
    state.update_state_post_tx_execution(!spec.cip645.fix_eip1153);
    let outcome = ExecutiveContext::new(state, env, machine, spec).transact(
        tx,
        TransactOptions {
            observer: calls,
            settings: TransactSettings::all_checks(),
        },
    )?;
    // A DB failure can leave executor checkpoints active. Only commit a normal
    // outcome; callers discard the State on error.
    state.update_state_post_tx_execution(!spec.cip645.fix_eip1153);
    Ok(outcome)
}
