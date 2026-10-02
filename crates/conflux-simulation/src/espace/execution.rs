use super::{Execution, Fee, Outcome};
use crate::{
    Error,
    env::BlockContext,
    execution,
    primitive::*,
    state::{StateSource, new_state},
    tracer::Calls,
    view::View,
};
use alloy::primitives::{Address, B256, LogData, U256};
use cfx_executor::{
    executive::{ExecutionError, ExecutionOutcome, ToRepackError, TxDropError},
    machine::Machine,
    state::State,
    verification::{TransactionVerifier, VerifyTxLocalMode, VerifyTxMode},
};
use cfx_parameters::{
    consensus::TRANSACTION_DEFAULT_EPOCH_BOUND, tx_pool::TXPOOL_DEFAULT_NONCE_BITS,
};
use cfx_types::Space;
use cfx_vm_types::{Env, Spec};
use primitives::{SignedTransaction, transaction::TransactionError};
use simulation_core::{
    AccountDiff, CallFrame, ChangeSet, Diff, ExecutionStatus, ExecutionTrace, FeePayment, Log,
    Rejection, RejectionReason, derive_changes,
};
use std::{collections::BTreeMap, sync::Arc};
use tokio::runtime::Handle;

pub(crate) fn execute(
    source: Arc<StateSource>,
    runtime: Handle,
    machine: &Machine,
    context: BlockContext,
    tx: SignedTransaction,
) -> Result<Outcome, Error> {
    let spec = machine.spec(context.number, context.epoch_height);
    let verifier =
        TransactionVerifier::new(TRANSACTION_DEFAULT_EPOCH_BOUND, TXPOOL_DEFAULT_NONCE_BITS);
    if let Err(error) = verifier.verify_transaction_common(
        &tx,
        cfx_types::AllChainID::new(
            machine
                .params()
                .chain_id(context.epoch_height, Space::Native),
            machine
                .params()
                .chain_id(context.epoch_height, Space::Ethereum),
        ),
        context.epoch_height,
        &machine.params().transition_heights,
        VerifyTxMode::Local(VerifyTxLocalMode::Full, &spec),
    ) {
        return Ok(Outcome::Rejected(static_rejection(error)));
    }
    let mut state = new_state(Arc::clone(&source), runtime.clone())?;
    let env = context.env(machine, &state, &tx);
    let mut calls = Calls::new(machine, context.number, context.epoch_height);
    let outcome = execution::transact(&mut state, machine, &env, &spec, &tx, &mut calls)?;
    let (failure, executed) = match outcome {
        ExecutionOutcome::NotExecutedDrop(error) => {
            return Ok(Outcome::Rejected(drop_rejection(error)));
        }
        ExecutionOutcome::NotExecutedToReconsiderPacking(error) => {
            return Ok(Outcome::Rejected(repack_rejection(error)));
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
        payer: address_from_cfx(tx.sender().address),
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
                calls.frames,
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
    calls: Vec<CallFrame<Address>>,
    logs: Vec<primitives::LogEntry>,
    payment: &FeePayment<Address>,
) -> Result<ChangeSet<Address>, Error> {
    let before = new_state(Arc::clone(&source), runtime)?;
    let accounts = account_diffs(&before, &after)?;
    let logs = logs
        .into_iter()
        .filter(|log| log.space == Space::Ethereum)
        .map(|log| Log {
            address: address_from_cfx(log.address),
            data: LogData::new_unchecked(
                log.topics.into_iter().map(b256_from_cfx).collect(),
                log.data.into(),
            ),
        })
        .collect();
    let trace = ExecutionTrace {
        calls,
        logs,
        accounts,
    };
    derive_changes(
        &trace,
        payment,
        &View::new(before, machine, env, spec, &source.budget),
        &View::new(after, machine, env, spec, &source.budget),
    )
}

fn account_diffs(before: &State, after: &State) -> Result<BTreeMap<Address, AccountDiff>, Error> {
    let mut accounts = BTreeMap::new();
    for (address, entry) in &after.committed_cache {
        if address.space != Space::Ethereum || !entry.is_dirty() {
            continue;
        }
        let Some(account) = entry.account() else {
            continue;
        };
        let mut storage = BTreeMap::new();
        for key in account.modified_storage_keys() {
            if key.len() != 32 {
                return Err(Error::Execution(
                    "eSpace storage key is not 32 bytes".into(),
                ));
            }
            let old = B256::from(u256_from_cfx(before.storage_at(address, &key)?));
            let new = B256::from(u256_from_cfx(after.storage_at(address, &key)?));
            if old != new {
                storage.insert(
                    B256::from_slice(&key),
                    Diff {
                        before: old,
                        after: new,
                    },
                );
            }
        }
        let nonce = |state: &State| -> Result<u64, Error> {
            state
                .nonce(address)?
                .try_into()
                .map_err(|_| Error::Execution("account nonce exceeds u64".into()))
        };
        let delegation = |state: &State| -> Result<Option<Address>, Error> {
            Ok(state
                .code(address)?
                .and_then(|code| primitives::transaction::extract_7702_payload(&code))
                .map(address_from_cfx))
        };
        accounts.insert(
            address_from_cfx(address.address),
            AccountDiff {
                balance: Diff {
                    before: u256_from_cfx(before.balance(address)?),
                    after: u256_from_cfx(after.balance(address)?),
                },
                nonce: Diff {
                    before: nonce(before)?,
                    after: nonce(after)?,
                },
                code_hash: Diff {
                    before: b256_from_cfx(before.code_hash(address)?),
                    after: b256_from_cfx(after.code_hash(address)?),
                },
                delegation: Diff {
                    before: delegation(before)?,
                    after: delegation(after)?,
                },
                storage,
            },
        );
    }
    Ok(accounts)
}

fn static_rejection(error: TransactionError) -> Rejection {
    use RejectionReason as R;
    let reason = match error {
        TransactionError::ChainIdMismatch { .. } => R::InvalidChainId,
        TransactionError::NotEnoughBaseGas { .. } => R::IntrinsicGasTooLow,
        TransactionError::PriortyGreaterThanMaxFee => R::TipAboveFeeCap,
        TransactionError::CreateInitCodeSizeLimit => R::InitCodeTooLarge,
        _ => R::InvalidTransaction,
    };
    Rejection {
        reason,
        message: error.to_string(),
    }
}

fn drop_rejection(error: TxDropError) -> Rejection {
    use RejectionReason as R;
    let reason = match error {
        TxDropError::OldNonce(..) => R::NonceTooLow,
        TxDropError::NotEnoughGasLimit { .. } => R::IntrinsicGasTooLow,
        TxDropError::SenderWithCode(_) => R::SenderNotEoa,
        TxDropError::InvalidRecipientAddress(_) => R::InvalidTransaction,
    };
    Rejection {
        reason,
        message: format!("{error:?}"),
    }
}

fn repack_rejection(error: ToRepackError) -> Rejection {
    use RejectionReason as R;
    let reason = match error {
        ToRepackError::InvalidNonce { .. } => R::NonceTooHigh,
        ToRepackError::SenderDoesNotExist | ToRepackError::NotEnoughBalance { .. } => {
            R::InsufficientFunds
        }
        ToRepackError::NotEnoughBaseFee { .. } => R::FeeCapTooLow,
        _ => R::InvalidTransaction,
    };
    Rejection {
        reason,
        message: format!("{error:?}"),
    }
}
