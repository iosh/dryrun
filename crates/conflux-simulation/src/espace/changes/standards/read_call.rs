use alloy_primitives::{Address, Bytes};
use cfx_executor::{
    executive::{
        ChargeCollateral, ExecutionError, ExecutionOutcome, ExecutiveContext, TransactOptions,
        TransactSettings,
    },
    machine::Machine,
    state::State,
};
use cfx_statedb::Error as StateDbError;
use cfx_types::{AddressSpaceUtil, U256};
use cfx_vm_types::{self as vm, Env, Spec};
use contract_standards::MetadataCall;
use primitives::transaction::{Action, Eip155Transaction, EthereumTransaction};

use crate::{execution::PreparedTransactionExecution, primitive::address_to_cfx};

const METADATA_CALL_GAS_LIMIT: u64 = 100_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReadCallOutcome {
    Success(Bytes),
    Reverted(Bytes),
    Failed,
}

pub(crate) enum MetadataReadError {
    StateAccess {
        call: MetadataCall<Address>,
        source: StateDbError,
    },
    ProbeExecution {
        call: MetadataCall<Address>,
        details: String,
    },
}

#[derive(Debug)]
pub(crate) enum IsolatedReadCallError {
    StateAccess(StateDbError),
    Execution(String),
}

pub(crate) fn execute_read_call(
    state: &mut State,
    machine: &Machine,
    prepared_execution: &PreparedTransactionExecution,
    sender: Address,
    target: Address,
    data: Bytes,
    metadata_call: &MetadataCall<Address>,
) -> Result<ReadCallOutcome, MetadataReadError> {
    execute_isolated_read_call(
        state,
        machine,
        &prepared_execution.env,
        &prepared_execution.spec,
        sender,
        target,
        data,
        Some(METADATA_CALL_GAS_LIMIT),
    )
    .map_err(|error| match error {
        IsolatedReadCallError::StateAccess(source) => MetadataReadError::StateAccess {
            call: metadata_call.clone(),
            source,
        },
        IsolatedReadCallError::Execution(details) => MetadataReadError::ProbeExecution {
            call: metadata_call.clone(),
            details,
        },
    })
}

pub(crate) fn execute_isolated_read_call(
    state: &mut State,
    machine: &Machine,
    env: &Env,
    spec: &Spec,
    sender: Address,
    target: Address,
    data: Bytes,
    gas_limit: Option<u64>,
) -> Result<ReadCallOutcome, IsolatedReadCallError> {
    // `State::save` commits the cache and asserts that no executor checkpoint
    // is active.  A read call is only valid on a finalized state point; fail
    // explicitly instead of allowing an upstream assertion to panic.
    if !state.no_checkpoint() {
        return Err(IsolatedReadCallError::Execution(
            "isolated eSpace read call cannot run with an active state checkpoint".to_owned(),
        ));
    }

    let gas_limit = gas_limit.unwrap_or(METADATA_CALL_GAS_LIMIT);
    if gas_limit == 0 {
        return Err(IsolatedReadCallError::Execution(
            "isolated eSpace read call has a zero gas limit".to_owned(),
        ));
    }
    let sender = address_to_cfx(sender).with_evm_space();
    let nonce = state
        .nonce(&sender)
        .map_err(IsolatedReadCallError::StateAccess)?;
    let chain_id = env
        .chain_id
        .get(&cfx_types::Space::Ethereum)
        .copied()
        .ok_or_else(|| {
            IsolatedReadCallError::Execution(
                "execution environment is missing the eSpace chain id".to_owned(),
            )
        })?;
    let read_transaction = EthereumTransaction::Eip155(Eip155Transaction {
        nonce,
        gas_price: U256::zero(),
        gas: U256::from(gas_limit),
        action: Action::Call(address_to_cfx(target)),
        value: U256::zero(),
        chain_id: Some(chain_id),
        data: data.to_vec(),
    })
    .fake_sign_rpc(sender);
    let mut probe_env = env.clone();
    probe_env.gas_limit = U256::from(gas_limit);
    probe_env.transaction_hash = read_transaction.hash();

    // Reading the nonce may populate the cache. Save that cache state so each
    // probe starts from the same committed S1 and leaves no transition behind.
    let snapshot = state.save();
    let outcome = ExecutiveContext::new(state, &probe_env, machine, spec)
        .transact(
            &read_transaction,
            TransactOptions {
                observer: (),
                settings: TransactSettings {
                    charge_collateral: ChargeCollateral::EstimateSender,
                    charge_gas: false,
                    check_base_price: false,
                    check_epoch_bound: false,
                    forbid_eoa_with_code: false,
                },
            },
        )
        .map_err(IsolatedReadCallError::StateAccess)?;

    if !state.no_checkpoint() {
        return Err(IsolatedReadCallError::Execution(
            "isolated eSpace read call left an active state checkpoint".to_owned(),
        ));
    }

    let result = match outcome {
        ExecutionOutcome::Finished(executed) => ReadCallOutcome::Success(executed.output.into()),
        ExecutionOutcome::ExecutionErrorBumpNonce(
            ExecutionError::VmError(vm::Error::StateDbError(error)),
            _,
        ) => {
            // A state-db failure can escape while an upstream checkpoint is
            // still open. Abort the simulation and drop State instead of
            // restoring through State::restore's no-checkpoint assertion.
            return Err(IsolatedReadCallError::StateAccess(error.0));
        }
        ExecutionOutcome::ExecutionErrorBumpNonce(
            ExecutionError::VmError(vm::Error::Reverted),
            details,
        ) => ReadCallOutcome::Reverted(details.output.into()),
        ExecutionOutcome::ExecutionErrorBumpNonce(_, _)
        | ExecutionOutcome::NotExecutedDrop(_)
        | ExecutionOutcome::NotExecutedToReconsiderPacking(_) => ReadCallOutcome::Failed,
    };

    state.update_state_post_tx_execution(!spec.cip645.fix_eip1153);
    state.restore(snapshot);
    Ok(result)
}
