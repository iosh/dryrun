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
use primitives::transaction::{Action, Eip155Transaction, EthereumTransaction};

use crate::{
    ConfluxEngineError, execution::PreparedTransactionExecution, primitive::address_to_cfx,
};

const READ_CALL_GAS_LIMIT: u64 = 100_000;

#[derive(Debug)]
pub(super) enum ReadCallOutcome {
    Success(Bytes),
    Revert,
    Halt(String),
}

pub(super) fn execute_read_call(
    state: &mut State,
    machine: &Machine,
    prepared: &PreparedTransactionExecution,
    target: Address,
    data: Bytes,
) -> Result<ReadCallOutcome, ConfluxEngineError> {
    let sender = prepared.transaction.sender();
    if sender.space != Space::Ethereum {
        return Err(ConfluxEngineError::analysis_failed(
            "eSpace state getter received a non-eSpace transaction sender",
        ));
    }

    let chain_id = prepared
        .env
        .chain_id
        .get(&Space::Ethereum)
        .copied()
        .ok_or_else(|| {
            ConfluxEngineError::analysis_failed(
                "eSpace execution environment is missing its chain id",
            )
        })?;
    let nonce = state
        .nonce(&sender)
        .map_err(|error| ConfluxEngineError::StateAccess {
            message: format!("failed to read eSpace state getter nonce: {error}"),
        })?;
    let transaction = EthereumTransaction::Eip155(Eip155Transaction {
        nonce,
        gas_price: U256::zero(),
        gas: U256::from(READ_CALL_GAS_LIMIT),
        action: Action::Call(address_to_cfx(target)),
        value: U256::zero(),
        chain_id: Some(chain_id),
        data: data.to_vec(),
    })
    .fake_sign_rpc(sender);

    // Reading the nonce may populate State's cache. Saving commits that cache
    // and leaves the executive's required empty-cache entry condition intact.
    let phase = state.save();
    let options = TransactOptions {
        observer: (),
        settings: TransactSettings {
            charge_collateral: ChargeCollateral::EstimateSender,
            charge_gas: false,
            check_base_price: false,
            check_epoch_bound: false,
            forbid_eoa_with_code: false,
        },
    };
    let outcome = ExecutiveContext::new(state, &prepared.env, machine, &prepared.spec)
        .transact(&transaction, options)
        .map_err(|error| ConfluxEngineError::StateAccess {
            message: format!("state access failed during eSpace getter execution: {error}"),
        })?;

    if let ExecutionOutcome::ExecutionErrorBumpNonce(
        ExecutionError::VmError(vm::Error::StateDbError(error)),
        _,
    ) = &outcome
    {
        // A state-db failure can escape while an upstream checkpoint is still
        // open. Abort the whole simulation and drop State instead of restoring
        // through State::restore's no-checkpoint assertion.
        return Err(ConfluxEngineError::StateAccess {
            message: format!("state access failed during eSpace getter execution: {error:?}"),
        });
    }

    let read_outcome = match outcome {
        ExecutionOutcome::Finished(executed) => {
            ReadCallOutcome::Success(Bytes::from(executed.output))
        }
        ExecutionOutcome::ExecutionErrorBumpNonce(
            ExecutionError::VmError(vm::Error::Reverted),
            _,
        ) => ReadCallOutcome::Revert,
        ExecutionOutcome::ExecutionErrorBumpNonce(error, _) => {
            ReadCallOutcome::Halt(format!("{error:?}"))
        }
        ExecutionOutcome::NotExecutedDrop(error) => {
            ReadCallOutcome::Halt(format!("getter transaction was dropped: {error:?}"))
        }
        ExecutionOutcome::NotExecutedToReconsiderPacking(error) => {
            ReadCallOutcome::Halt(format!("getter transaction was not executable: {error:?}"))
        }
    };

    state.update_state_post_tx_execution(!prepared.spec.cip645.fix_eip1153);
    state.restore(phase);

    Ok(read_outcome)
}
