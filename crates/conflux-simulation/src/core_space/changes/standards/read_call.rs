use alloy_primitives::{Address, Bytes};
use cfx_executor::{
    executive::{
        ChargeCollateral, ExecutionError, ExecutionOutcome, ExecutiveContext, TransactOptions,
        TransactSettings,
    },
    machine::Machine,
    state::State,
};
use cfx_types::{Space, U256};
use cfx_vm_types as vm;
use primitives::transaction::{Action, NativeTransaction, TypedNativeTransaction};

use crate::{
    core_space::CoreSpaceChangesError, execution::PreparedTransactionExecution,
    primitive::address_to_cfx,
};

const STANDARD_READ_CALL_GAS_LIMIT: u64 = 100_000;

#[derive(Debug)]
pub(super) enum StandardReadCallOutcome {
    Success(Bytes),
    Revert,
    Halt,
}

pub(super) fn execute_standard_read_call(
    state: &mut State,
    machine: &Machine,
    prepared_execution: &PreparedTransactionExecution,
    target_contract: Address,
    call_data: Bytes,
) -> Result<StandardReadCallOutcome, CoreSpaceChangesError> {
    let getter_sender = prepared_execution.transaction.sender();
    let getter_chain_id = prepared_execution
        .env
        .chain_id
        .get(&Space::Native)
        .copied()
        .ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(
                "Core Space standard metadata probe environment is missing its chain id",
            )
        })?;
    let sender_nonce = state.nonce(&getter_sender).map_err(|error| {
        CoreSpaceChangesError::state_read("read Core Space metadata probe nonce", error)
    })?;
    let getter_transaction = TypedNativeTransaction::Cip155(NativeTransaction {
        nonce: sender_nonce,
        gas_price: U256::zero(),
        gas: U256::from(STANDARD_READ_CALL_GAS_LIMIT),
        action: Action::Call(address_to_cfx(target_contract)),
        value: U256::zero(),
        storage_limit: u64::MAX,
        epoch_height: prepared_execution.env.epoch_height,
        chain_id: getter_chain_id,
        data: call_data.to_vec(),
    })
    .fake_sign_rpc(getter_sender);

    // Reading the nonce may populate State's cache. Saving commits that cache
    // and leaves the executive's required empty-cache entry condition intact.
    let read_call_snapshot = state.save();
    let getter_options = TransactOptions {
        observer: (),
        settings: TransactSettings {
            charge_collateral: ChargeCollateral::EstimateSender,
            charge_gas: false,
            check_base_price: false,
            check_epoch_bound: false,
            forbid_eoa_with_code: false,
        },
    };
    let getter_execution_outcome = ExecutiveContext::new(
        state,
        &prepared_execution.env,
        machine,
        &prepared_execution.spec,
    )
    .transact(&getter_transaction, getter_options)
    .map_err(|error| {
        CoreSpaceChangesError::state_read("execute Core Space metadata probe", error)
    })?;

    let read_call_outcome = match getter_execution_outcome {
        ExecutionOutcome::Finished(executed_read_call) => {
            StandardReadCallOutcome::Success(Bytes::from(executed_read_call.output))
        }
        ExecutionOutcome::ExecutionErrorBumpNonce(
            ExecutionError::VmError(vm::Error::StateDbError(error)),
            _,
        ) => {
            // A state-db failure can escape while an upstream checkpoint is still
            // open. Abort the whole simulation and drop State instead of restoring
            // through State::restore's no-checkpoint assertion.
            return Err(CoreSpaceChangesError::state_read(
                "execute Core Space metadata probe",
                error.0,
            ));
        }
        ExecutionOutcome::ExecutionErrorBumpNonce(
            ExecutionError::VmError(vm::Error::Reverted),
            _,
        ) => StandardReadCallOutcome::Revert,
        ExecutionOutcome::ExecutionErrorBumpNonce(_, _) => StandardReadCallOutcome::Halt,
        ExecutionOutcome::NotExecutedDrop(_)
        | ExecutionOutcome::NotExecutedToReconsiderPacking(_) => StandardReadCallOutcome::Halt,
    };

    state.update_state_post_tx_execution(!prepared_execution.spec.cip645.fix_eip1153);
    state.restore(read_call_snapshot);

    Ok(read_call_outcome)
}
