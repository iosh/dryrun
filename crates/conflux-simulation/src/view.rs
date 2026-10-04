use crate::{Error, address::VmAddress};
use alloy::primitives::Bytes;
use cfx_executor::{
    executive::{
        ChargeCollateral, ExecutionError, ExecutionOutcome, ExecutiveContext, TransactOptions,
        TransactSettings,
    },
    machine::Machine,
    state::State,
};
use cfx_types::{AddressWithSpace, Space, U256};
use cfx_vm_types::{Env, Spec};
use primitives::transaction::{
    Action, Eip155Transaction, EthereumTransaction, NativeTransaction, TypedNativeTransaction,
};
use simulation_core::{CallResult, ReadBudget, StateView};
use std::cell::RefCell;

pub(crate) struct View<'a> {
    state: RefCell<Option<State>>,
    machine: &'a Machine,
    env: &'a Env,
    spec: &'a Spec,
    budget: &'a ReadBudget,
}

impl<'a> View<'a> {
    pub fn new(
        state: State,
        machine: &'a Machine,
        env: &'a Env,
        spec: &'a Spec,
        budget: &'a ReadBudget,
    ) -> Self {
        Self {
            state: RefCell::new(Some(state)),
            machine,
            env,
            spec,
            budget,
        }
    }

    fn probe<A: VmAddress>(
        &self,
        state: &mut State,
        contract: A,
        input: Bytes,
    ) -> Result<CallResult, Error> {
        let contract = contract.to_vm();
        let sender = AddressWithSpace {
            address: cfx_types::Address::zero(),
            space: contract.space,
        };
        let nonce = state.nonce(&sender)?;
        let gas = self.budget.limits().read_call_gas.into();
        let action = Action::Call(contract.address);
        let data = input.to_vec();
        let chain_id = self
            .env
            .chain_id
            .get(&sender.space)
            .copied()
            .ok_or(Error::Internal("execution context lacks chain ID"))?;
        let tx = match contract.space {
            Space::Ethereum => EthereumTransaction::Eip155(Eip155Transaction {
                nonce,
                gas_price: U256::zero(),
                gas,
                action,
                value: U256::zero(),
                chain_id: Some(chain_id),
                data,
            })
            .fake_sign_rpc(sender),
            Space::Native => TypedNativeTransaction::Cip155(NativeTransaction {
                nonce,
                gas_price: U256::zero(),
                gas,
                action,
                value: U256::zero(),
                storage_limit: 0,
                epoch_height: self.env.epoch_height,
                chain_id,
                data,
            })
            .fake_sign_rpc(sender),
        };
        let mut env = self.env.clone();
        env.gas_limit = *tx.gas();
        env.transaction_hash = tx.hash();
        // This copies only this view's request-local cache. Each successful
        // executor outcome, including revert/halt, is followed by restoration.
        let saved = state.save();
        let outcome = ExecutiveContext::new(state, &env, self.machine, self.spec).transact(
            &tx,
            TransactOptions {
                observer: (),
                settings: TransactSettings {
                    charge_collateral: if contract.space == Space::Native {
                        ChargeCollateral::Skip
                    } else {
                        ChargeCollateral::EstimateSender
                    },
                    charge_gas: false,
                    check_base_price: false,
                    check_epoch_bound: false,
                    forbid_eoa_with_code: false,
                },
            },
        )?;
        state.update_state_post_tx_execution(!self.spec.cip645.fix_eip1153);
        state.restore(saved);
        match outcome {
            ExecutionOutcome::Finished(executed) => {
                self.budget.check_read_call_output(executed.output.len())?;
                Ok(CallResult::Success(executed.output.into()))
            }
            ExecutionOutcome::ExecutionErrorBumpNonce(
                ExecutionError::VmError(cfx_vm_types::Error::Reverted),
                executed,
            ) => {
                self.budget.check_read_call_output(executed.output.len())?;
                Ok(CallResult::Revert)
            }
            _ => Ok(CallResult::Halt),
        }
    }
}

impl<A: VmAddress> StateView<A> for View<'_> {
    type Error = Error;
    fn call(&self, contract: A, input: Bytes) -> Result<CallResult, Error> {
        self.budget.record_read_call()?;
        let mut slot = self.state.borrow_mut();
        let mut state = slot.take().ok_or(Error::Internal(
            "state view was discarded after a failed read",
        ))?;
        let result = self.probe(&mut state, contract, input);
        if result.is_ok() {
            *slot = Some(state);
        }
        // On DB error discard the state, without touching unfinished checkpoints.
        result
    }
}
