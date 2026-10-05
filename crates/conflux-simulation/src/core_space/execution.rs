use super::{Address, Execution, Fee, FeePayer, changes};
use crate::address::VmAddress;
use crate::{
    Error,
    context::BlockContext,
    execution,
    primitive::*,
    state::{StateSource, new_state},
    tracer::Tracer,
};
use alloy::primitives::U256;
use cfx_executor::{
    executive::{ExecutionError, ExecutionOutcome},
    machine::Machine,
};
use cfx_types::Space;
use primitives::SignedTransaction;
use simulation_core::Outcome;
use simulation_core::{ExecutionStatus, FeePayment};
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
    let base_fee = context.base_gas_price[Space::Native];
    let price = if spec.cip1559 {
        tx.effective_gas_price(&base_fee.min(*tx.gas_price()))
    } else {
        *tx.gas_price()
    };
    let fee = Fee {
        gas_price: u256_from_cfx(price),
        base_fee: u256_from_cfx(base_fee),
        amount: u256_from_cfx(executed.fee),
        payer: if executed.gas_sponsor_paid {
            let primitives::transaction::Action::Call(contract) = tx.action() else {
                return Err(Error::Internal("contract creation charged a gas sponsor"));
            };
            FeePayer::Sponsor {
                contract: Address::from_vm(
                    cfx_types::AddressWithSpace {
                        address: contract,
                        space: Space::Native,
                    },
                    source.network,
                ),
            }
        } else {
            FeePayer::Sender {
                address: Address::from_vm(tx.sender(), source.network),
            }
        },
    };
    let payment = FeePayment {
        payer: match fee.payer {
            FeePayer::Sender { address } => Some(address),
            FeePayer::Sponsor { .. } => None,
        },
        amount: fee.amount,
        beneficiary: Address::from_vm(
            cfx_types::AddressWithSpace {
                address: context.author,
                space: Space::Native,
            },
            source.network,
        ),
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
            changes: changes::derive(
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
        gas_used: u256_from_cfx(executed.gas_used),
        gas_charged: u256_from_cfx(executed.gas_charged),
        fee,
        status,
    })))
}
