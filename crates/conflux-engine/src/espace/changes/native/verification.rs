use std::collections::BTreeMap;

use alloy_primitives::{Address, U256};
use cfx_executor::state::State;
use cfx_types::AddressSpaceUtil;
use contract_standards::StatePhase;
use simulation_changes::{Change, NativeMetadata, PositionedChange};

use super::{NativeOperation, NativeOperations};
use crate::{
    ConfluxEngineError,
    primitive::{address_to_cfx, u256_from_cfx},
};

pub(crate) type NativeBalances = BTreeMap<Address, U256>;

pub(crate) fn read_native_balances(
    state: &State,
    phase: StatePhase,
    native_operations: &NativeOperations,
) -> Result<NativeBalances, ConfluxEngineError> {
    native_operations
        .balance_accounts
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
    native_operations: &NativeOperations,
    before_balances: &NativeBalances,
    after_balances: &NativeBalances,
    execution_fee: U256,
    burnt_fee: Option<U256>,
) -> Result<Vec<PositionedChange>, ConfluxEngineError> {
    if let Some(burnt_fee) = burnt_fee
        && burnt_fee > execution_fee
    {
        return Err(ConfluxEngineError::analysis_failed(format!(
            "eSpace burnt fee exceeds total fee: burnt {burnt_fee}, total {execution_fee}"
        )));
    }

    let mut replayed_balances = before_balances.clone();
    let mut positioned_changes = Vec::new();
    let mut precharged_fee = U256::ZERO;
    let mut refunded_fee = U256::ZERO;

    for operation in &native_operations.operations {
        match operation {
            NativeOperation::AccountTransfer {
                position,
                from,
                to,
                amount,
            } => {
                decrease_balance(&mut replayed_balances, *from, *amount)?;
                increase_balance(&mut replayed_balances, *to, *amount)?;
                positioned_changes.push(PositionedChange::new(
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
                precharged_fee = precharged_fee.checked_add(*amount).ok_or_else(|| {
                    ConfluxEngineError::analysis_failed(
                        "eSpace gas precharge overflowed during native analysis",
                    )
                })?;
                decrease_balance(&mut replayed_balances, *payer, *amount)?;
            }
            NativeOperation::GasRefund { recipient, amount } => {
                refunded_fee = refunded_fee.checked_add(*amount).ok_or_else(|| {
                    ConfluxEngineError::analysis_failed(
                        "eSpace gas refund overflowed during native analysis",
                    )
                })?;
                increase_balance(&mut replayed_balances, *recipient, *amount)?;
            }
        }
    }

    let net_fee_from_operations = precharged_fee.checked_sub(refunded_fee).ok_or_else(|| {
        ConfluxEngineError::analysis_failed("eSpace gas refund exceeded precharge")
    })?;
    if net_fee_from_operations != execution_fee {
        return Err(ConfluxEngineError::analysis_failed(format!(
            "eSpace gas settlement mismatch: traced {net_fee_from_operations}, execution fee {execution_fee}"
        )));
    }

    for &address in &native_operations.balance_accounts {
        let replayed_balance = replayed_balances.get(&address).copied().ok_or_else(|| {
            ConfluxEngineError::analysis_failed(format!(
                "replayed eSpace balance is missing for {address}"
            ))
        })?;
        let after_balance = after_balances.get(&address).copied().ok_or_else(|| {
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

    Ok(positioned_changes)
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
