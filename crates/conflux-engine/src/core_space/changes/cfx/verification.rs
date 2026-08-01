use std::collections::BTreeMap;

use alloy_primitives::U256;
use cfx_executor::state::State;
use cfx_types::AddressSpaceUtil;
use contract_standards::StatePhase;
use simulation_changes::{Change, NativeMetadata};

use super::{CfxBalanceLocation, CfxOperation, CfxOperations};
use crate::{
    ConfluxEngineError,
    core_space::changes::{CoreSpaceChange, PositionedCoreSpaceChange},
    primitive::{address_to_cfx, u256_from_cfx},
};

#[derive(Debug, Clone)]
pub(crate) struct CfxStateSnapshot {
    balances: BTreeMap<CfxBalanceLocation, U256>,
    total_issued: U256,
    total_staking: U256,
}

pub(crate) fn read_cfx_state_snapshot(
    state: &State,
    phase: StatePhase,
    cfx_operations: &CfxOperations,
) -> Result<CfxStateSnapshot, ConfluxEngineError> {
    let mut balances = BTreeMap::new();
    for &location in &cfx_operations.balance_locations {
        let balance = match location {
            CfxBalanceLocation::Account { account } => state
                .balance(&address_to_cfx(account).with_native_space())
                .map_err(|error| ConfluxEngineError::StateAccess {
                    message: format!(
                        "failed to read {phase} Core Space balance for {location}: {error}"
                    ),
                })?,
            CfxBalanceLocation::Staking { account } => state
                .staking_balance(&address_to_cfx(account))
                .map_err(|error| ConfluxEngineError::StateAccess {
                    message: format!(
                        "failed to read {phase} Core Space balance for {location}: {error}"
                    ),
                })?,
            CfxBalanceLocation::GasSponsor { contract_address } => state
                .sponsor_balance_for_gas(&address_to_cfx(contract_address))
                .map_err(|error| ConfluxEngineError::StateAccess {
                    message: format!(
                        "failed to read {phase} Core Space balance for {location}: {error}"
                    ),
                })?,
        };
        balances.insert(location, u256_from_cfx(balance));
    }

    Ok(CfxStateSnapshot {
        balances,
        total_issued: u256_from_cfx(state.total_issued_tokens()),
        total_staking: u256_from_cfx(state.total_staking_tokens()),
    })
}

pub(crate) fn verify_cfx_changes(
    cfx_operations: &CfxOperations,
    before_state: &CfxStateSnapshot,
    after_state: &CfxStateSnapshot,
    expected_gas_fee_payer: CfxBalanceLocation,
    execution_fee: U256,
    burnt_fee: Option<U256>,
) -> Result<Vec<PositionedCoreSpaceChange>, ConfluxEngineError> {
    if let Some(burnt_fee) = burnt_fee
        && burnt_fee > execution_fee
    {
        return Err(ConfluxEngineError::analysis_failed(format!(
            "Core Space burnt fee exceeds total fee: burnt {burnt_fee}, total {execution_fee}"
        )));
    }

    let mut replayed_state = before_state.clone();
    let mut positioned_core_changes = Vec::new();
    let mut precharged_fee = U256::ZERO;
    let mut refunded_fee = U256::ZERO;

    for operation in &cfx_operations.operations {
        match operation {
            CfxOperation::AccountTransfer {
                position,
                from,
                to,
                amount,
            } => {
                replayed_state
                    .debit_balance(CfxBalanceLocation::Account { account: *from }, *amount)?;
                replayed_state
                    .credit_balance(CfxBalanceLocation::Account { account: *to }, *amount)?;
                positioned_core_changes.push(PositionedCoreSpaceChange::new(
                    *position,
                    CoreSpaceChange::StandardOrNative(Change::NativeTransfer {
                        from: *from,
                        to: *to,
                        raw_amount: *amount,
                        metadata: NativeMetadata::default(),
                    }),
                ));
            }
            CfxOperation::GasPrecharge { payer, amount } => {
                verify_gas_fee_location("precharge payer", *payer, expected_gas_fee_payer)?;
                precharged_fee = precharged_fee.checked_add(*amount).ok_or_else(|| {
                    ConfluxEngineError::analysis_failed(
                        "Core Space gas precharge overflowed during CFX analysis",
                    )
                })?;
                replayed_state.debit_balance(*payer, *amount)?;
            }
            CfxOperation::GasRefund { recipient, amount } => {
                verify_gas_fee_location("refund recipient", *recipient, expected_gas_fee_payer)?;
                refunded_fee = refunded_fee.checked_add(*amount).ok_or_else(|| {
                    ConfluxEngineError::analysis_failed(
                        "Core Space gas refund overflowed during CFX analysis",
                    )
                })?;
                replayed_state.credit_balance(*recipient, *amount)?;
            }
            CfxOperation::StakingDeposit {
                position,
                account,
                amount,
            } => {
                replayed_state
                    .debit_balance(CfxBalanceLocation::Account { account: *account }, *amount)?;
                replayed_state
                    .credit_balance(CfxBalanceLocation::Staking { account: *account }, *amount)?;
                replayed_state.total_staking = replayed_state
                    .total_staking
                    .checked_add(*amount)
                    .ok_or_else(|| {
                        ConfluxEngineError::analysis_failed(
                            "Core Space total staking overflowed while replaying a deposit",
                        )
                    })?;
                positioned_core_changes.push(PositionedCoreSpaceChange::new(
                    *position,
                    CoreSpaceChange::StakingDeposit {
                        account: *account,
                        raw_amount: *amount,
                    },
                ));
            }
            CfxOperation::StakingWithdrawal {
                position,
                account,
                principal_amount,
                reward_amount,
            } => {
                replayed_state.debit_balance(
                    CfxBalanceLocation::Staking { account: *account },
                    *principal_amount,
                )?;
                let withdrawal_credit =
                    principal_amount
                        .checked_add(*reward_amount)
                        .ok_or_else(|| {
                            ConfluxEngineError::analysis_failed(format!(
                                "Core Space staking withdrawal amount overflowed for {account}"
                            ))
                        })?;
                replayed_state.credit_balance(
                    CfxBalanceLocation::Account { account: *account },
                    withdrawal_credit,
                )?;
                replayed_state.total_staking = replayed_state
                    .total_staking
                    .checked_sub(*principal_amount)
                    .ok_or_else(|| {
                        ConfluxEngineError::analysis_failed(
                            "Core Space total staking underflowed while replaying a withdrawal",
                        )
                    })?;
                replayed_state.total_issued = replayed_state
                    .total_issued
                    .checked_add(*reward_amount)
                    .ok_or_else(|| {
                        ConfluxEngineError::analysis_failed(
                            "Core Space total issued overflowed while replaying staking interest",
                        )
                    })?;
                positioned_core_changes.push(PositionedCoreSpaceChange::new(
                    *position,
                    CoreSpaceChange::StakingWithdrawal {
                        account: *account,
                        raw_amount: *principal_amount,
                        reward_raw_amount: *reward_amount,
                    },
                ));
            }
            CfxOperation::NativeBurn {
                position,
                account,
                amount,
            } => {
                replayed_state
                    .debit_balance(CfxBalanceLocation::Account { account: *account }, *amount)?;
                replayed_state.debit_total_issued(*amount, "a native balance burn")?;
                positioned_core_changes.push(PositionedCoreSpaceChange::new(
                    *position,
                    CoreSpaceChange::NativeBurn {
                        from: *account,
                        raw_amount: *amount,
                    },
                ));
            }
            CfxOperation::StakingBurn {
                position,
                account,
                amount,
            } => {
                replayed_state
                    .debit_balance(CfxBalanceLocation::Staking { account: *account }, *amount)?;
                replayed_state.debit_total_issued(*amount, "a staking balance burn")?;
                positioned_core_changes.push(PositionedCoreSpaceChange::new(
                    *position,
                    CoreSpaceChange::StakingBurn {
                        account: *account,
                        raw_amount: *amount,
                    },
                ));
            }
        }
    }

    let net_fee_from_operations = precharged_fee.checked_sub(refunded_fee).ok_or_else(|| {
        ConfluxEngineError::analysis_failed("Core Space gas refund exceeded precharge")
    })?;
    if net_fee_from_operations != execution_fee {
        return Err(ConfluxEngineError::analysis_failed(format!(
            "Core Space gas settlement mismatch: traced {net_fee_from_operations}, execution fee {execution_fee}"
        )));
    }

    if let Some(burnt_fee) = burnt_fee {
        replayed_state.debit_total_issued(burnt_fee, "the CIP-1559 fee burn")?;
    }
    replayed_state.verify_matches(after_state)?;

    Ok(positioned_core_changes)
}

impl CfxStateSnapshot {
    fn debit_balance(
        &mut self,
        location: CfxBalanceLocation,
        amount: U256,
    ) -> Result<(), ConfluxEngineError> {
        let balance = self.balances.get_mut(&location).ok_or_else(|| {
            ConfluxEngineError::analysis_failed(format!(
                "before Core Space CFX balance is missing for {location}"
            ))
        })?;
        *balance = balance.checked_sub(amount).ok_or_else(|| {
            ConfluxEngineError::analysis_failed(format!(
                "Core Space CFX balance underflow for {location}: balance {balance}, debit {amount}"
            ))
        })?;
        Ok(())
    }

    fn credit_balance(
        &mut self,
        location: CfxBalanceLocation,
        amount: U256,
    ) -> Result<(), ConfluxEngineError> {
        let balance = self.balances.get_mut(&location).ok_or_else(|| {
            ConfluxEngineError::analysis_failed(format!(
                "before Core Space CFX balance is missing for {location}"
            ))
        })?;
        *balance = balance.checked_add(amount).ok_or_else(|| {
            ConfluxEngineError::analysis_failed(format!(
                "Core Space CFX balance overflow for {location}: balance {balance}, credit {amount}"
            ))
        })?;
        Ok(())
    }

    fn debit_total_issued(
        &mut self,
        amount: U256,
        debit_reason: &str,
    ) -> Result<(), ConfluxEngineError> {
        self.total_issued = self.total_issued.checked_sub(amount).ok_or_else(|| {
            ConfluxEngineError::analysis_failed(format!(
                "Core Space total issued underflowed while replaying {debit_reason}"
            ))
        })?;
        Ok(())
    }

    fn verify_matches(&self, after_state: &Self) -> Result<(), ConfluxEngineError> {
        for (location, replayed_balance) in &self.balances {
            let after_balance = after_state.balances.get(location).ok_or_else(|| {
                ConfluxEngineError::analysis_failed(format!(
                    "after Core Space CFX balance is missing for {location}"
                ))
            })?;
            if replayed_balance != after_balance {
                return Err(ConfluxEngineError::analysis_failed(format!(
                    "Core Space CFX balance mismatch for {location}: replayed {replayed_balance}, after {after_balance}"
                )));
            }
        }

        if self.total_issued != after_state.total_issued {
            return Err(ConfluxEngineError::analysis_failed(format!(
                "Core Space total issued mismatch: replayed {}, after {}",
                self.total_issued, after_state.total_issued
            )));
        }
        if self.total_staking != after_state.total_staking {
            return Err(ConfluxEngineError::analysis_failed(format!(
                "Core Space total staking mismatch: replayed {}, after {}",
                self.total_staking, after_state.total_staking
            )));
        }

        Ok(())
    }
}

fn verify_gas_fee_location(
    location_role: &str,
    observed_location: CfxBalanceLocation,
    expected_location: CfxBalanceLocation,
) -> Result<(), ConfluxEngineError> {
    if observed_location == expected_location {
        return Ok(());
    }

    Err(ConfluxEngineError::analysis_failed(format!(
        "Core Space gas {location_role} mismatch: observed {observed_location}, expected {expected_location}"
    )))
}
