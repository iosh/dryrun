use alloy_primitives::Bytes;
use cfx_executor::{
    executive::{
        ChargeCollateral, ExecutionError, ExecutionOutcome, ExecutiveContext, TransactOptions,
        TransactSettings,
    },
    machine::Machine,
    state::State,
};
use cfx_statedb::Error as StateDbError;
use cfx_types::{Address, AddressWithSpace, Space, U256};
use cfx_vm_types::{self as vm, Env, Spec};
use primitives::transaction::{
    Action, Eip155Transaction, EthereumTransaction, NativeTransaction, TypedNativeTransaction,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadCallOutcome {
    Success(Bytes),
    Reverted(Bytes),
    Failed,
}

impl ReadCallOutcome {
    pub fn output(&self) -> Option<&Bytes> {
        match self {
            Self::Success(output) | Self::Reverted(output) => Some(output),
            Self::Failed => None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum IsolatedReadCallError {
    #[error(transparent)]
    StateAccess(#[from] StateDbError),
    #[error("isolated call integration failed: {0}")]
    Execution(String),
}

pub(crate) struct ReadCallInput {
    pub(crate) sender: AddressWithSpace,
    pub(crate) target: Address,
    pub(crate) data: Bytes,
    pub(crate) gas_limit: u64,
}

pub(crate) fn execute_isolated_read_call(
    state: &mut State,
    machine: &Machine,
    env: &Env,
    spec: &Spec,
    input: ReadCallInput,
) -> Result<ReadCallOutcome, IsolatedReadCallError> {
    // `State::save` commits the cache and asserts that no executor checkpoint
    // is active. A read call requires an independent state point; fail
    // explicitly instead of allowing an upstream assertion to panic.
    if !state.no_checkpoint() {
        return Err(IsolatedReadCallError::Execution(
            "isolated Conflux read call cannot run with an active state checkpoint".to_owned(),
        ));
    }

    let ReadCallInput {
        sender,
        target,
        data,
        gas_limit,
    } = input;
    if gas_limit == 0 {
        return Err(IsolatedReadCallError::Execution(
            "isolated Conflux read call has a zero gas limit".to_owned(),
        ));
    }
    let nonce = state.nonce(&sender)?;
    let chain_id = env.chain_id.get(&sender.space).copied().ok_or_else(|| {
        IsolatedReadCallError::Execution(
            "execution environment is missing the requested Space chain id".to_owned(),
        )
    })?;
    let read_transaction = match sender.space {
        Space::Ethereum => EthereumTransaction::Eip155(Eip155Transaction {
            nonce,
            gas_price: U256::zero(),
            gas: U256::from(gas_limit),
            action: Action::Call(target),
            value: U256::zero(),
            chain_id: Some(chain_id),
            data: data.to_vec(),
        })
        .fake_sign_rpc(sender),
        Space::Native => TypedNativeTransaction::Cip155(NativeTransaction {
            nonce,
            gas_price: U256::zero(),
            gas: U256::from(gas_limit),
            action: Action::Call(target),
            value: U256::zero(),
            chain_id,
            storage_limit: u64::MAX,
            epoch_height: env.epoch_height,
            data: data.to_vec(),
        })
        .fake_sign_rpc(sender),
    };
    let mut probe_env = env.clone();
    probe_env.gas_limit = U256::from(gas_limit);
    probe_env.transaction_hash = read_transaction.hash();

    // Reading the nonce may populate the cache. Save that cache state so each
    // probe starts from the same retained state point and leaves no transition behind.
    let snapshot = state.save();
    // State errors propagate directly. The caller invalidates this reader on
    // failure; restoring it here could hit an unfinished executor checkpoint.
    let outcome = ExecutiveContext::new(state, &probe_env, machine, spec).transact(
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
    )?;

    let result = match outcome {
        ExecutionOutcome::Finished(executed) => ReadCallOutcome::Success(executed.output.into()),
        ExecutionOutcome::ExecutionErrorBumpNonce(
            ExecutionError::VmError(vm::Error::Reverted),
            details,
        ) => ReadCallOutcome::Reverted(details.output.into()),
        ExecutionOutcome::ExecutionErrorBumpNonce(_, _)
        | ExecutionOutcome::NotExecutedDrop(_)
        | ExecutionOutcome::NotExecutedToReconsiderPacking(_) => ReadCallOutcome::Failed,
    };

    if !state.no_checkpoint() {
        return Err(IsolatedReadCallError::Execution(
            "isolated Conflux read call left an active state checkpoint".to_owned(),
        ));
    }

    state.update_state_post_tx_execution(!spec.cip645.fix_eip1153);
    state.restore(snapshot);
    Ok(result)
}
