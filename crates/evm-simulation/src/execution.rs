use std::collections::BTreeMap;

use alloy::primitives::{Address, B256, U256};
use alloy_evm::EvmEnv;
use revm::{
    Context, InspectEvm, MainBuilder, MainContext,
    context::{Block, Transaction, TxEnv},
    context_interface::result::{EVMError, ExecutionResult, InvalidTransaction, ResultAndState},
    database::CacheDB,
    database_interface::{DatabaseCommit, DatabaseRef, WrapDatabaseRef},
    primitives::hardfork::SpecId,
    state::{AccountInfo, Bytecode, EvmState},
};
use simulation_core::{
    AccountDiff, ChangeSet, Diff, ExecutionStatus, ExecutionTrace, FeePayment, Log, ReadBudget,
    Rejection, RejectionReason, derive_changes,
};

use crate::{
    Error, Execution, Fee, Outcome, db::CachedAlloyDB, state::StateReader, tracer::CallTracer,
    transaction::skips_fee_checks,
};

/// Executes the transaction once and, if it succeeds, derives its changes.
pub(crate) fn execute(
    db: &CachedAlloyDB<'_>,
    env: &EvmEnv,
    tx: TxEnv,
    budget: &ReadBudget,
) -> Result<Outcome, Error> {
    let mut cfg = env.cfg_env.clone();
    cfg.disable_base_fee = skips_fee_checks(&tx);
    let block = &env.block_env;
    let caller = tx.caller;
    let gas_price = tx.effective_gas_price(u128::from(block.basefee));
    let blob_gas_price = (!tx.blob_hashes.is_empty())
        .then(|| block.blob_gasprice())
        .flatten();
    let blob_gas = tx.total_blob_gas();

    let mut evm = Context::mainnet()
        .with_db(WrapDatabaseRef(db))
        .with_cfg(cfg)
        .with_block(block.clone())
        .build_mainnet_with_inspector(CallTracer::default());
    let ResultAndState { result, state } = match evm.inspect_tx(tx) {
        Ok(output) => output,
        Err(EVMError::Transaction(error)) => return Ok(Outcome::Rejected(rejection(error))),
        Err(EVMError::Database(error)) => return Err(error.into()),
        Err(error) => return Err(Error::Execution(error.to_string())),
    };
    let calls = std::mem::take(&mut evm.inspector.calls);

    let gas_used = result.gas().tx_gas_used();
    let fee = Fee {
        gas_price,
        base_fee: block.basefee,
        blob_gas_price,
        amount: U256::from(gas_used) * U256::from(gas_price)
            + U256::from(blob_gas) * U256::from(blob_gas_price.unwrap_or_default()),
    };
    let beneficiary_price = if env.cfg_env.spec.is_enabled_in(SpecId::LONDON) {
        gas_price.saturating_sub(u128::from(block.basefee))
    } else {
        gas_price
    };
    let payment = FeePayment {
        payer: caller,
        amount: fee.amount,
        beneficiary: block.beneficiary,
        reward: U256::from(gas_used) * U256::from(beneficiary_price),
    };

    let status = match result {
        ExecutionResult::Success { output, logs, .. } => ExecutionStatus::Success {
            output: output.into_data(),
            changes: derive(db, env, budget, calls, logs, state, &payment),
        },
        ExecutionResult::Revert { output, .. } => ExecutionStatus::reverted(output),
        ExecutionResult::Halt { reason, .. } => ExecutionStatus::Halted {
            reason: format!("{reason:?}"),
        },
    };
    Ok(Outcome::Executed(Box::new(Execution {
        gas_used,
        fee,
        status,
    })))
}

fn derive(
    db: &CachedAlloyDB<'_>,
    env: &EvmEnv,
    budget: &ReadBudget,
    calls: Vec<simulation_core::CallFrame<Address>>,
    logs: Vec<alloy::primitives::Log>,
    state: EvmState,
    payment: &FeePayment<Address>,
) -> Result<ChangeSet<Address>, Error> {
    let trace = ExecutionTrace {
        calls,
        logs: logs
            .into_iter()
            .map(|log| Log {
                address: log.address,
                data: log.data,
            })
            .collect(),
        accounts: account_diffs(db, &state)?,
    };
    // Both sides are overlays on the same cache; only the side after the
    // execution holds its writes.
    let before = CacheDB::new(db);
    let mut after = CacheDB::new(db);
    after.commit(state);
    derive_changes(
        &trace,
        payment,
        &StateReader::new(&before, env, budget),
        &StateReader::new(&after, env, budget),
    )
}

/// Accounts touched by the execution. Values before it come from `db`, which
/// already holds every account the execution loaded.
fn account_diffs(
    db: &CachedAlloyDB<'_>,
    state: &EvmState,
) -> Result<BTreeMap<Address, AccountDiff>, Error> {
    let mut accounts = BTreeMap::new();
    for (address, account) in state {
        if !account.is_touched() {
            continue;
        }
        let before = db.basic_ref(*address)?.unwrap_or_default();
        let destroyed = account.is_selfdestructed();
        let after = if destroyed {
            &AccountInfo::default()
        } else {
            &account.info
        };
        let storage: BTreeMap<_, _> = account
            .storage
            .iter()
            .filter_map(|(slot, value)| {
                let after = if destroyed {
                    U256::ZERO
                } else {
                    value.present_value
                };
                (value.original_value != after).then(|| {
                    let diff = Diff {
                        before: B256::from(value.original_value),
                        after: B256::from(after),
                    };
                    (B256::from(*slot), diff)
                })
            })
            .collect();
        let diff = AccountDiff {
            balance: Diff {
                before: before.balance,
                after: after.balance,
            },
            nonce: Diff {
                before: before.nonce,
                after: after.nonce,
            },
            code_hash: Diff {
                before: before.code_hash,
                after: after.code_hash,
            },
            delegation: Diff {
                before: delegation(&before),
                after: delegation(after),
            },
            storage,
        };
        accounts.insert(*address, diff);
    }
    Ok(accounts)
}

fn delegation(account: &AccountInfo) -> Option<Address> {
    account.code.as_ref().and_then(Bytecode::eip7702_address)
}

fn rejection(error: InvalidTransaction) -> Rejection {
    use InvalidTransaction as E;
    let reason = match &error {
        E::NonceTooLow { .. } => RejectionReason::NonceTooLow,
        E::NonceTooHigh { .. } => RejectionReason::NonceTooHigh,
        E::NonceOverflowInTransaction => RejectionReason::NonceMax,
        E::LackOfFundForMaxFee { .. } | E::OverflowPaymentInTransaction => {
            RejectionReason::InsufficientFunds
        }
        E::GasPriceLessThanBasefee => RejectionReason::FeeCapTooLow,
        E::PriorityFeeGreaterThanMaxFee => RejectionReason::TipAboveFeeCap,
        E::BlobGasPriceGreaterThanMax { .. } => RejectionReason::BlobFeeCapTooLow,
        E::CallGasCostMoreThanGasLimit { .. } | E::GasFloorMoreThanGasLimit { .. } => {
            RejectionReason::IntrinsicGasTooLow
        }
        E::CallerGasLimitMoreThanBlock | E::TxGasLimitGreaterThanCap { .. } => {
            RejectionReason::GasLimitTooHigh
        }
        E::RejectCallerWithCode => RejectionReason::SenderNotEoa,
        E::InvalidChainId | E::MissingChainId => RejectionReason::InvalidChainId,
        E::CreateInitCodeSizeLimit => RejectionReason::InitCodeTooLarge,
        _ => RejectionReason::InvalidTransaction,
    };
    Rejection {
        reason,
        message: error.to_string(),
    }
}
