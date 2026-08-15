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
use cfx_vm_types as vm;
use contract_standards::MetadataCall;
use primitives::transaction::{Action, Eip155Transaction, EthereumTransaction};

use crate::{
    espace::EspaceChangesError, execution::PreparedTransactionExecution, primitive::address_to_cfx,
};

const METADATA_CALL_GAS_LIMIT: u64 = 100_000;

pub(crate) enum ReadCallOutcome {
    Success(Bytes),
    Reverted,
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

impl From<MetadataReadError> for EspaceChangesError {
    fn from(error: MetadataReadError) -> Self {
        match error {
            MetadataReadError::StateAccess { call, source } => {
                Self::MetadataStateAccess { call, source }
            }
            MetadataReadError::ProbeExecution { call, details } => {
                Self::MetadataProbeExecution { call, details }
            }
        }
    }
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
    let sender = address_to_cfx(sender).with_evm_space();
    let nonce = state
        .nonce(&sender)
        .map_err(|error| MetadataReadError::StateAccess {
            call: metadata_call.clone(),
            source: error,
        })?;
    let chain_id = prepared_execution
        .env
        .chain_id
        .get(&cfx_types::Space::Ethereum)
        .copied()
        .ok_or_else(|| MetadataReadError::ProbeExecution {
            call: metadata_call.clone(),
            details: "execution environment is missing the eSpace chain id".to_owned(),
        })?;
    let read_transaction = EthereumTransaction::Eip155(Eip155Transaction {
        nonce,
        gas_price: U256::zero(),
        gas: U256::from(METADATA_CALL_GAS_LIMIT),
        action: Action::Call(address_to_cfx(target)),
        value: U256::zero(),
        chain_id: Some(chain_id),
        data: data.to_vec(),
    })
    .fake_sign_rpc(sender);
    let mut probe_env = prepared_execution.env.clone();
    probe_env.gas_limit = U256::from(METADATA_CALL_GAS_LIMIT);
    probe_env.transaction_hash = read_transaction.hash();

    // Reading the nonce may populate the cache. Save that cache state so each
    // probe starts from the same committed S1 and leaves no transition behind.
    let snapshot = state.save();
    let outcome = ExecutiveContext::new(state, &probe_env, machine, &prepared_execution.spec)
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
        .map_err(|error| MetadataReadError::StateAccess {
            call: metadata_call.clone(),
            source: error,
        })?;

    let result = match outcome {
        ExecutionOutcome::Finished(executed) => ReadCallOutcome::Success(executed.output.into()),
        ExecutionOutcome::ExecutionErrorBumpNonce(
            ExecutionError::VmError(vm::Error::StateDbError(error)),
            _,
        ) => {
            state.restore(snapshot);
            return Err(MetadataReadError::StateAccess {
                call: metadata_call.clone(),
                source: error.0,
            });
        }
        ExecutionOutcome::ExecutionErrorBumpNonce(
            ExecutionError::VmError(vm::Error::Reverted),
            _,
        ) => ReadCallOutcome::Reverted,
        ExecutionOutcome::ExecutionErrorBumpNonce(_, _)
        | ExecutionOutcome::NotExecutedDrop(_)
        | ExecutionOutcome::NotExecutedToReconsiderPacking(_) => ReadCallOutcome::Failed,
    };

    state.update_state_post_tx_execution(!prepared_execution.spec.cip645.fix_eip1153);
    state.restore(snapshot);
    Ok(result)
}
