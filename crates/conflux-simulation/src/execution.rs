use crate::{Error, address::VmAddress, tracer::Tracer};
use cfx_executor::{
    executive::{
        ChargeCollateral, ExecutionOutcome, ExecutiveContext, ToRepackError, TransactOptions,
        TransactSettings, TxDropError,
    },
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
use simulation_core::{Rejection, RejectionReason};

/// Applies the upstream transaction checks that do not read account state.
pub(crate) fn verify_transaction_static(
    tx: &SignedTransaction,
    machine: &Machine,
    spec: &Spec,
    epoch: u64,
) -> Result<(), Rejection> {
    let params = machine.params();
    let chain_ids = params.chain_id.read().get_chain_id(epoch);
    let verifier =
        TransactionVerifier::new(TRANSACTION_DEFAULT_EPOCH_BOUND, TXPOOL_DEFAULT_NONCE_BITS);
    verifier
        .verify_transaction_common(
            tx,
            chain_ids,
            epoch,
            &params.transition_heights,
            VerifyTxMode::Local(VerifyTxLocalMode::Full, spec),
        )
        .map_err(static_rejection)
}

/// The only formal local execution of the user's transaction.
pub(crate) fn transact<A: VmAddress>(
    state: &mut State,
    machine: &Machine,
    env: &Env,
    spec: &Spec,
    tx: &SignedTransaction,
    tracer: &mut Tracer<'_, A>,
) -> Result<ExecutionOutcome, Error> {
    let retain_transient_storage = !spec.cip645.fix_eip1153;
    state.update_state_post_tx_execution(retain_transient_storage);
    let outcome = ExecutiveContext::new(state, env, machine, spec).transact(
        tx,
        TransactOptions {
            observer: tracer,
            settings: TransactSettings {
                charge_collateral: if tx.space() == Space::Native {
                    ChargeCollateral::Skip
                } else {
                    ChargeCollateral::Normal
                },
                ..TransactSettings::all_checks()
            },
        },
    )?;
    // A DB failure can leave executor checkpoints active. Only commit a normal
    // outcome; callers discard the State on error.
    state.update_state_post_tx_execution(retain_transient_storage);
    Ok(outcome)
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

pub(crate) fn drop_rejection(error: TxDropError) -> Rejection {
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

pub(crate) fn repack_rejection(error: ToRepackError) -> Rejection {
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
