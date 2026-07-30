use std::collections::BTreeMap;

use alloy_primitives::{Address, U256};
use cfx_executor::state::State;
use cfx_types::AddressSpaceUtil;
use contract_standards::StatePhase;
use simulation_changes::{Change, NativeMetadata, PositionedChange};

use super::{NativeEvidence, NativeOperation};
use crate::{
    ConfluxEngineError,
    primitive::{address_to_cfx, u256_from_cfx},
};

pub(crate) type NativeBalances = BTreeMap<Address, U256>;

pub(crate) fn read_native_balances(
    state: &State,
    phase: StatePhase,
    evidence: &NativeEvidence,
) -> Result<NativeBalances, ConfluxEngineError> {
    evidence
        .accounts
        .iter()
        .map(|&address| {
            let balance = state
                .balance(&address_to_cfx(address).with_evm_space())
                .map_err(|error| ConfluxEngineError::StateAccess {
                    message: format!(
                        "failed to read {phase} eSpace balance for {address}: {error}"
                    ),
                })?;
            Ok((address, u256_from_cfx(balance)))
        })
        .collect()
}

pub(crate) fn verify_native_changes(
    evidence: &NativeEvidence,
    before: &NativeBalances,
    after: &NativeBalances,
    fee: U256,
    burnt_fee: Option<U256>,
) -> Result<Vec<PositionedChange>, ConfluxEngineError> {
    if let Some(burnt_fee) = burnt_fee
        && burnt_fee > fee
    {
        return Err(ConfluxEngineError::analysis_failed(format!(
            "eSpace burnt fee exceeds total fee: burnt {burnt_fee}, total {fee}"
        )));
    }

    let mut replayed = before.clone();
    let mut changes = Vec::new();
    let mut gas_precharge = U256::ZERO;
    let mut gas_refund = U256::ZERO;

    for operation in &evidence.operations {
        match operation {
            NativeOperation::Transfer {
                position,
                from,
                to,
                amount,
            } => {
                decrease_balance(&mut replayed, *from, *amount)?;
                increase_balance(&mut replayed, *to, *amount)?;
                changes.push(PositionedChange::new(
                    *position,
                    Change::NativeTransfer {
                        from: *from,
                        to: *to,
                        raw_amount: *amount,
                        metadata: NativeMetadata::default(),
                    },
                ));
            }
            NativeOperation::GasPrecharge { payer, amount } => {
                gas_precharge = gas_precharge.checked_add(*amount).ok_or_else(|| {
                    ConfluxEngineError::analysis_failed(
                        "eSpace gas precharge overflowed during native analysis",
                    )
                })?;
                decrease_balance(&mut replayed, *payer, *amount)?;
            }
            NativeOperation::GasRefund { recipient, amount } => {
                gas_refund = gas_refund.checked_add(*amount).ok_or_else(|| {
                    ConfluxEngineError::analysis_failed(
                        "eSpace gas refund overflowed during native analysis",
                    )
                })?;
                increase_balance(&mut replayed, *recipient, *amount)?;
            }
        }
    }

    let traced_fee = gas_precharge.checked_sub(gas_refund).ok_or_else(|| {
        ConfluxEngineError::analysis_failed("eSpace gas refund exceeded precharge")
    })?;
    if traced_fee != fee {
        return Err(ConfluxEngineError::analysis_failed(format!(
            "eSpace gas settlement mismatch: traced {traced_fee}, execution fee {fee}"
        )));
    }

    for &address in &evidence.accounts {
        let replayed_balance = replayed.get(&address).copied().ok_or_else(|| {
            ConfluxEngineError::analysis_failed(format!(
                "replayed eSpace balance is missing for {address}"
            ))
        })?;
        let after_balance = after.get(&address).copied().ok_or_else(|| {
            ConfluxEngineError::analysis_failed(format!(
                "after eSpace balance is missing for {address}"
            ))
        })?;

        if replayed_balance != after_balance {
            return Err(ConfluxEngineError::analysis_failed(format!(
                "eSpace balance mismatch for {address}: replayed {replayed_balance}, after {after_balance}"
            )));
        }
    }

    Ok(changes)
}

fn decrease_balance(
    balances: &mut NativeBalances,
    address: Address,
    amount: U256,
) -> Result<(), ConfluxEngineError> {
    let balance = balances.get_mut(&address).ok_or_else(|| {
        ConfluxEngineError::analysis_failed(format!(
            "before eSpace balance is missing for {address}"
        ))
    })?;
    *balance = balance.checked_sub(amount).ok_or_else(|| {
        ConfluxEngineError::analysis_failed(format!(
            "eSpace balance underflow for {address}: balance {balance}, debit {amount}"
        ))
    })?;
    Ok(())
}

fn increase_balance(
    balances: &mut NativeBalances,
    address: Address,
    amount: U256,
) -> Result<(), ConfluxEngineError> {
    let balance = balances.get_mut(&address).ok_or_else(|| {
        ConfluxEngineError::analysis_failed(format!(
            "before eSpace balance is missing for {address}"
        ))
    })?;
    *balance = balance.checked_add(amount).ok_or_else(|| {
        ConfluxEngineError::analysis_failed(format!(
            "eSpace balance overflow for {address}: balance {balance}, credit {amount}"
        ))
    })?;
    Ok(())
}
