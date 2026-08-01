use std::collections::BTreeMap;

use alloy_primitives::{Address, U256};
use cfx_executor::state::State;
use cfx_types::AddressSpaceUtil;
use contract_standards::StatePhase;
use simulation_changes::{Change, NativeMetadata};

use super::{
    CfxBalanceLocation, CfxOperation, CfxOperations, SponsorResourceLocation,
    SponsorshipCallOperation, SponsorshipRefundOperation, StoragePointConversionOperation,
};
use crate::{
    ConfluxEngineError,
    core_space::changes::{CoreSpaceChange, PositionedCoreSpaceChange, SponsoredResource},
    primitive::{address_to_cfx, u256_from_cfx},
};

#[derive(Debug, Clone)]
pub(crate) struct CfxStateValues {
    balances: BTreeMap<CfxBalanceLocation, U256>,
    sponsor_identities: BTreeMap<SponsorResourceLocation, Option<Address>>,
    storage_points: BTreeMap<Address, Option<StoragePointValues>>,
    total_issued: U256,
    total_staking: U256,
    storage_point_globals: Option<StoragePointGlobalValues>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StoragePointValues {
    unused: U256,
    used: U256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StoragePointGlobalValues {
    total_storage: U256,
    used_storage_points: U256,
    converted_storage_points: U256,
}

pub(crate) fn read_cfx_state_values(
    state: &State,
    phase: StatePhase,
    cfx_operations: &CfxOperations,
) -> Result<CfxStateValues, ConfluxEngineError> {
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
            CfxBalanceLocation::StorageSponsor { contract_address } => state
                .sponsor_balance_for_collateral(&address_to_cfx(contract_address))
                .map_err(|error| ConfluxEngineError::StateAccess {
                    message: format!(
                        "failed to read {phase} Core Space balance for {location}: {error}"
                    ),
                })?,
            CfxBalanceLocation::StorageCollateral { contract_address } => state
                .token_collateral_for_storage(&address_to_cfx(contract_address))
                .map_err(|error| ConfluxEngineError::StateAccess {
                    message: format!(
                        "failed to read {phase} Core Space balance for {location}: {error}"
                    ),
                })?,
        };
        balances.insert(location, u256_from_cfx(balance));
    }

    let mut sponsor_identities = BTreeMap::new();
    for &location in &cfx_operations.sponsor_resources {
        let contract_address = address_to_cfx(location.contract_address);
        let sponsor = match location.resource {
            SponsoredResource::Gas => state.sponsor_for_gas(&contract_address),
            SponsoredResource::StorageCollateral => state.sponsor_for_collateral(&contract_address),
        }
        .map_err(|error| ConfluxEngineError::StateAccess {
            message: format!(
                "failed to read {phase} Core Space {:?} sponsor identity for contract {}: {error}",
                location.resource, location.contract_address
            ),
        })?
        .map(crate::primitive::address_from_cfx);
        sponsor_identities.insert(location, sponsor);
    }

    let mut storage_points = BTreeMap::new();
    for &account in &cfx_operations.storage_point_accounts {
        let sponsor_info = state
            .sponsor_info(&address_to_cfx(account))
            .map_err(|error| ConfluxEngineError::StateAccess {
                message: format!(
                    "failed to read {phase} Core Space storage points for contract {account}: {error}"
                ),
            })?;
        let points = sponsor_info
            .and_then(|info| info.storage_points)
            .map(|points| StoragePointValues {
                unused: u256_from_cfx(points.unused),
                used: u256_from_cfx(points.used),
            });
        storage_points.insert(account, points);
    }

    let storage_point_globals =
        cfx_operations
            .requires_storage_point_globals
            .then(|| StoragePointGlobalValues {
                total_storage: u256_from_cfx(state.total_storage_tokens()),
                used_storage_points: u256_from_cfx(state.used_storage_points()),
                converted_storage_points: u256_from_cfx(state.converted_storage_points()),
            });

    Ok(CfxStateValues {
        balances,
        sponsor_identities,
        storage_points,
        total_issued: u256_from_cfx(state.total_issued_tokens()),
        total_staking: u256_from_cfx(state.total_staking_tokens()),
        storage_point_globals,
    })
}

pub(crate) fn verify_cfx_changes(
    cfx_operations: &CfxOperations,
    before_state: &CfxStateValues,
    after_state: &CfxStateValues,
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
            CfxOperation::SponsorshipCall(call) => {
                replayed_state.apply_sponsorship_call(call, &mut positioned_core_changes)?;
            }
            CfxOperation::SponsorshipStandaloneRefund(refund) => {
                replayed_state
                    .apply_standalone_sponsorship_refund(refund, &mut positioned_core_changes)?;
            }
            CfxOperation::StoragePointConversion(conversion) => {
                replayed_state
                    .apply_storage_point_conversion(conversion, &mut positioned_core_changes)?;
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

impl CfxStateValues {
    fn apply_sponsorship_call(
        &mut self,
        call: &SponsorshipCallOperation,
        positioned_changes: &mut Vec<PositionedCoreSpaceChange>,
    ) -> Result<(), ConfluxEngineError> {
        let resource_location = SponsorResourceLocation {
            resource: call.resource,
            contract_address: call.contract_address,
        };
        let new_sponsor = (!call.sponsor.is_zero()).then_some(call.sponsor);
        if !call.gross_deposit_amount.is_zero() && new_sponsor.is_none() {
            return Err(ConfluxEngineError::analysis_failed(
                "Core Space nonzero sponsorship deposit had no sponsor identity",
            ));
        }

        self.debit_balance(
            CfxBalanceLocation::Account {
                account: call.sponsor,
            },
            call.gross_deposit_amount,
        )?;

        let current_sponsor = self.sponsor_identity(resource_location)?;
        if let Some(refund) = call.refund {
            if refund.resource != call.resource
                || refund.contract_address != call.contract_address
                || current_sponsor != Some(refund.sponsor)
                || Some(refund.sponsor) == new_sponsor
            {
                return Err(ConfluxEngineError::analysis_failed(format!(
                    "Core Space {:?} sponsorship replacement identity did not match its refund",
                    call.resource
                )));
            }
            let pool_location = call.resource.pool_location(call.contract_address);
            self.verify_exact_balance(
                pool_location,
                refund.pool_refund_amount,
                "replacement refund",
            )?;
            let direct_compensation = refund
                .gross_refund_amount
                .checked_sub(refund.pool_refund_amount)
                .ok_or_else(|| {
                    ConfluxEngineError::analysis_failed(
                        "Core Space sponsorship gross refund was smaller than its pool refund",
                    )
                })?;
            match call.resource {
                SponsoredResource::Gas if !direct_compensation.is_zero() => {
                    return Err(ConfluxEngineError::analysis_failed(
                        "Core Space gas sponsorship replacement contained direct collateral compensation",
                    ));
                }
                SponsoredResource::StorageCollateral => {
                    self.verify_exact_balance(
                        CfxBalanceLocation::StorageCollateral {
                            contract_address: call.contract_address,
                        },
                        direct_compensation,
                        "replacement collateral compensation",
                    )?;
                }
                SponsoredResource::Gas => {}
            }
            self.debit_balance(pool_location, refund.pool_refund_amount)?;
            self.credit_balance(
                CfxBalanceLocation::Account {
                    account: refund.sponsor,
                },
                refund.gross_refund_amount,
            )?;
            if !refund.gross_refund_amount.is_zero() {
                positioned_changes.push(PositionedCoreSpaceChange::new(
                    refund.position,
                    CoreSpaceChange::SponsorshipRefund {
                        sponsored_resource: refund.resource,
                        sponsor: refund.sponsor,
                        contract_address: refund.contract_address,
                        raw_amount: refund.gross_refund_amount,
                    },
                ));
            }
        } else if current_sponsor.is_some() && current_sponsor != new_sponsor {
            return Err(ConfluxEngineError::analysis_failed(format!(
                "Core Space {:?} sponsorship identity changed without a refund transit",
                call.resource
            )));
        }

        self.credit_balance(
            call.resource.pool_location(call.contract_address),
            call.pool_deposit_amount,
        )?;
        self.set_sponsor_identity(resource_location, new_sponsor)?;
        if !call.gross_deposit_amount.is_zero() {
            positioned_changes.push(PositionedCoreSpaceChange::new(
                call.position,
                CoreSpaceChange::SponsorshipDeposit {
                    sponsored_resource: call.resource,
                    sponsor: call.sponsor,
                    contract_address: call.contract_address,
                    raw_amount: call.gross_deposit_amount,
                },
            ));
        }
        Ok(())
    }

    fn apply_standalone_sponsorship_refund(
        &mut self,
        refund: &SponsorshipRefundOperation,
        positioned_changes: &mut Vec<PositionedCoreSpaceChange>,
    ) -> Result<(), ConfluxEngineError> {
        let resource_location = SponsorResourceLocation {
            resource: refund.resource,
            contract_address: refund.contract_address,
        };
        if self.sponsor_identity(resource_location)? != Some(refund.sponsor) {
            return Err(ConfluxEngineError::analysis_failed(format!(
                "Core Space standalone {:?} sponsorship refund did not match the sponsor identity",
                refund.resource
            )));
        }
        let pool_location = refund.resource.pool_location(refund.contract_address);
        self.verify_exact_balance(
            pool_location,
            refund.pool_refund_amount,
            "standalone refund",
        )?;
        self.debit_balance(pool_location, refund.pool_refund_amount)?;
        self.credit_balance(
            CfxBalanceLocation::Account {
                account: refund.sponsor,
            },
            refund.gross_refund_amount,
        )?;
        self.set_sponsor_identity(resource_location, None)?;
        if !refund.gross_refund_amount.is_zero() {
            positioned_changes.push(PositionedCoreSpaceChange::new(
                refund.position,
                CoreSpaceChange::SponsorshipRefund {
                    sponsored_resource: refund.resource,
                    sponsor: refund.sponsor,
                    contract_address: refund.contract_address,
                    raw_amount: refund.gross_refund_amount,
                },
            ));
        }
        Ok(())
    }

    fn apply_storage_point_conversion(
        &mut self,
        conversion: &StoragePointConversionOperation,
        positioned_changes: &mut Vec<PositionedCoreSpaceChange>,
    ) -> Result<(), ConfluxEngineError> {
        let converted_amount = conversion
            .from_sponsor_pool
            .checked_add(conversion.from_storage_collateral)
            .ok_or_else(|| {
                ConfluxEngineError::analysis_failed(
                    "Core Space storage-point conversion amount overflowed",
                )
            })?;
        if converted_amount.is_zero() {
            return Err(ConfluxEngineError::analysis_failed(
                "Core Space storage-point conversion had a zero amount",
            ));
        }

        self.debit_balance(
            CfxBalanceLocation::StorageSponsor {
                contract_address: conversion.contract_address,
            },
            conversion.from_sponsor_pool,
        )?;
        self.debit_balance(
            CfxBalanceLocation::StorageCollateral {
                contract_address: conversion.contract_address,
            },
            conversion.from_storage_collateral,
        )?;

        let storage_points = self
            .storage_points
            .get_mut(&conversion.contract_address)
            .ok_or_else(|| {
                ConfluxEngineError::analysis_failed(format!(
                    "before Core Space storage points are missing for contract {}",
                    conversion.contract_address
                ))
            })?;
        match storage_points {
            Some(points) => {
                if !conversion.from_storage_collateral.is_zero() {
                    return Err(ConfluxEngineError::analysis_failed(format!(
                        "Core Space initialized contract {} converted token collateral again",
                        conversion.contract_address
                    )));
                }
                points.unused = points
                    .unused
                    .checked_add(conversion.from_sponsor_pool)
                    .ok_or_else(|| {
                        ConfluxEngineError::analysis_failed(format!(
                            "Core Space unused storage points overflowed for contract {}",
                            conversion.contract_address
                        ))
                    })?;
            }
            None => {
                *storage_points = Some(StoragePointValues {
                    unused: conversion.from_sponsor_pool,
                    used: conversion.from_storage_collateral,
                });
            }
        }

        self.debit_total_issued(converted_amount, "a storage-point conversion")?;
        let globals = self.storage_point_globals.as_mut().ok_or_else(|| {
            ConfluxEngineError::analysis_failed(
                "before Core Space storage-point globals are missing",
            )
        })?;
        globals.total_storage = globals
            .total_storage
            .checked_sub(conversion.from_storage_collateral)
            .ok_or_else(|| {
                ConfluxEngineError::analysis_failed(
                    "Core Space total storage underflowed during storage-point conversion",
                )
            })?;
        globals.used_storage_points = globals
            .used_storage_points
            .checked_add(conversion.from_storage_collateral)
            .ok_or_else(|| {
                ConfluxEngineError::analysis_failed(
                    "Core Space used storage points overflowed during conversion",
                )
            })?;
        globals.converted_storage_points = globals
            .converted_storage_points
            .checked_add(converted_amount)
            .ok_or_else(|| {
                ConfluxEngineError::analysis_failed(
                    "Core Space converted storage points overflowed during conversion",
                )
            })?;

        positioned_changes.push(PositionedCoreSpaceChange::new(
            conversion.position,
            CoreSpaceChange::StoragePointConversion {
                contract_address: conversion.contract_address,
                converted_cfx_raw_amount: converted_amount,
            },
        ));
        Ok(())
    }

    fn sponsor_identity(
        &self,
        location: SponsorResourceLocation,
    ) -> Result<Option<Address>, ConfluxEngineError> {
        self.sponsor_identities
            .get(&location)
            .copied()
            .ok_or_else(|| {
                ConfluxEngineError::analysis_failed(format!(
                    "before Core Space {:?} sponsor identity is missing for contract {}",
                    location.resource, location.contract_address
                ))
            })
    }

    fn set_sponsor_identity(
        &mut self,
        location: SponsorResourceLocation,
        sponsor: Option<Address>,
    ) -> Result<(), ConfluxEngineError> {
        let current = self.sponsor_identities.get_mut(&location).ok_or_else(|| {
            ConfluxEngineError::analysis_failed(format!(
                "before Core Space {:?} sponsor identity is missing for contract {}",
                location.resource, location.contract_address
            ))
        })?;
        *current = sponsor;
        Ok(())
    }

    fn verify_exact_balance(
        &self,
        location: CfxBalanceLocation,
        expected: U256,
        operation: &str,
    ) -> Result<(), ConfluxEngineError> {
        let actual = self.balances.get(&location).ok_or_else(|| {
            ConfluxEngineError::analysis_failed(format!(
                "before Core Space CFX balance is missing for {location}"
            ))
        })?;
        if *actual != expected {
            return Err(ConfluxEngineError::analysis_failed(format!(
                "Core Space sponsorship {operation} did not consume the complete {location}: balance {actual}, refund {expected}"
            )));
        }
        Ok(())
    }

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

        for (location, replayed_sponsor) in &self.sponsor_identities {
            let after_sponsor = after_state
                .sponsor_identities
                .get(location)
                .ok_or_else(|| {
                    ConfluxEngineError::analysis_failed(format!(
                        "after Core Space {:?} sponsor identity is missing for contract {}",
                        location.resource, location.contract_address
                    ))
                })?;
            if replayed_sponsor != after_sponsor {
                return Err(ConfluxEngineError::analysis_failed(format!(
                    "Core Space {:?} sponsor identity mismatch for contract {}: replayed {:?}, after {:?}",
                    location.resource, location.contract_address, replayed_sponsor, after_sponsor
                )));
            }
        }

        for (account, replayed_points) in &self.storage_points {
            let after_points = after_state.storage_points.get(account).ok_or_else(|| {
                ConfluxEngineError::analysis_failed(format!(
                    "after Core Space storage points are missing for contract {account}"
                ))
            })?;
            if replayed_points != after_points {
                return Err(ConfluxEngineError::analysis_failed(format!(
                    "Core Space storage-point pocket mismatch for contract {account}: replayed {replayed_points:?}, after {after_points:?}"
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
        if self.storage_point_globals != after_state.storage_point_globals {
            return Err(ConfluxEngineError::analysis_failed(format!(
                "Core Space storage-point global mismatch: replayed {:?}, after {:?}",
                self.storage_point_globals, after_state.storage_point_globals
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
