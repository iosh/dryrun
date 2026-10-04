use super::{Execution, Fee, Outcome};
use crate::{
    Error,
    env::BlockContext,
    execution,
    primitive::*,
    state::{StateSource, new_state},
    tracer::Tracer,
    view::View,
};
use alloy::primitives::{Address, U256};
use cfx_executor::{
    executive::{ExecutionError, ExecutionOutcome},
    machine::Machine,
    state::State,
};
use cfx_types::Space;
use cfx_vm_types::{Env, Spec};
use primitives::SignedTransaction;
use simulation_core::{ChangeSet, ExecutionStatus, FeePayment, derive_changes};
use std::sync::Arc;
use tokio::runtime::Handle;

pub(crate) fn execute(
    source: Arc<StateSource>,
    runtime: Handle,
    machine: &Machine,
    context: BlockContext,
    tx: SignedTransaction,
) -> Result<Outcome<Execution>, Error> {
    let spec = machine.spec(context.number, context.epoch_height);
    if let Err(rejection) =
        execution::verify_transaction_static(&tx, machine, &spec, context.epoch_height)
    {
        return Ok(Outcome::Rejected(rejection));
    }
    let mut state = new_state(Arc::clone(&source), runtime.clone())?;
    let env = context.env(machine, &state, &tx);
    let mut tracer = Tracer::new(machine, &env, &spec, source.network);
    let outcome = execution::transact(&mut state, machine, &env, &spec, &tx, &mut tracer)?;
    let (failure, executed) = match outcome {
        ExecutionOutcome::NotExecutedDrop(error) => {
            return Ok(Outcome::Rejected(execution::drop_rejection(error)));
        }
        ExecutionOutcome::NotExecutedToReconsiderPacking(error) => {
            return Ok(Outcome::Rejected(execution::repack_rejection(error)));
        }
        ExecutionOutcome::ExecutionErrorBumpNonce(error, executed) => (Some(error), executed),
        ExecutionOutcome::Finished(executed) => (None, executed),
    };
    let base_fee = context.base_gas_price[Space::Ethereum];
    let price = if spec.cip1559 {
        tx.effective_gas_price(&base_fee.min(*tx.gas_price()))
    } else {
        *tx.gas_price()
    };
    let fee = Fee {
        gas_price: price
            .try_into()
            .map_err(|_| Error::Execution("gas price exceeds u128".into()))?,
        base_fee: base_fee
            .try_into()
            .map_err(|_| Error::Execution("base fee exceeds u128".into()))?,
        amount: u256_from_cfx(executed.fee),
    };
    let payment = FeePayment {
        payer: Some(address_from_cfx(tx.sender().address)),
        amount: fee.amount,
        beneficiary: address_from_cfx(context.author),
        reward: U256::ZERO,
    };
    let status = match failure {
        Some(ExecutionError::VmError(cfx_vm_types::Error::Reverted)) => {
            ExecutionStatus::reverted(executed.output.into())
        }
        Some(error) => ExecutionStatus::Halted {
            reason: format!("{error:?}"),
        },
        None => ExecutionStatus::Success {
            output: executed.output.into(),
            changes: derive(
                source,
                runtime,
                machine,
                &env,
                &spec,
                state,
                tracer,
                executed.logs,
                &payment,
            ),
        },
    };
    Ok(Outcome::Executed(Box::new(Execution {
        gas_used: executed
            .gas_used
            .try_into()
            .map_err(|_| Error::Execution("gas used exceeds u64".into()))?,
        gas_charged: executed
            .gas_charged
            .try_into()
            .map_err(|_| Error::Execution("gas charged exceeds u64".into()))?,
        fee,
        status,
    })))
}

#[allow(clippy::too_many_arguments)]
fn derive(
    source: Arc<StateSource>,
    runtime: Handle,
    machine: &Machine,
    env: &Env,
    spec: &Spec,
    after: State,
    tracer: Tracer<'_, Address>,
    logs: Vec<primitives::LogEntry>,
    payment: &FeePayment<Address>,
) -> Result<ChangeSet<Address>, Error> {
    let before = new_state(Arc::clone(&source), runtime)?;
    let trace = tracer.into_trace(&before, &after, logs)?;
    derive_changes(
        &trace,
        payment,
        &View::new(before, machine, env, spec, &source.budget),
        &View::new(after, machine, env, spec, &source.budget),
    )
}
