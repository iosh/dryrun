use std::collections::BTreeMap;

use alloy_primitives::{Address, U256};
use cfx_executor::state::State;
use cfx_types::{AddressSpaceUtil, address_util::AddressUtil};
use contract_standards::StatePhase;
use simulation_changes::{Change, NativeMetadata};

use super::{
    CfxBalanceLocation, CfxOperation, CfxOperations, CrossSpaceTransferOperation,
    SponsorResourceLocation, SponsorshipAccessCallerRole, SponsorshipAccessRuleKey,
    SponsorshipAccessRuleUpdate, SponsorshipFundingOperation, SponsorshipFundingTerms,
    SponsorshipRefundOperation, StorageCollateralReleaseOperation, StoragePointConversionOperation,
    cross_space_balance_location,
};
use crate::{
    ConfluxEngineError,
    core_space::changes::{
        CoreSpaceChange, CrossSpaceAddress, PositionedCoreSpaceChange, SponsoredResource,
        SponsorshipConfiguration, SponsorshipEligibilityTarget,
    },
    primitive::{address_to_cfx, u256_from_cfx},
    state::SponsorWhitelistStorageKey,
};

#[derive(Debug, Clone)]
pub(crate) struct CfxStateValues {
    balances: BTreeMap<CfxBalanceLocation, U256>,
    sponsor_identities: BTreeMap<SponsorResourceLocation, Option<Address>>,
    gas_fee_upper_bounds: BTreeMap<Address, U256>,
    sponsorship_access_rules: BTreeMap<SponsorshipAccessRuleKey, bool>,
    sponsorship_contract_admins: BTreeMap<Address, Address>,
    storage_points: BTreeMap<Address, Option<StoragePointValues>>,
    total_issued: U256,
    total_staking: U256,
    total_espace_tokens: Option<U256>,
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
            CfxBalanceLocation::CoreSpaceAccount { account } => state
                .balance(&address_to_cfx(account).with_native_space())
                .map_err(|error| ConfluxEngineError::StateAccess {
                    message: format!(
                        "failed to read {phase} Core Space balance for {location}: {error}"
                    ),
                })?,
            CfxBalanceLocation::EspaceAccount { account } => state
                .balance(&address_to_cfx(account).with_evm_space())
                .map_err(|error| ConfluxEngineError::StateAccess {
                    message: format!(
                        "failed to read {phase} Core simulation balance for {location}: {error}"
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

    let mut gas_fee_upper_bounds = BTreeMap::new();
    for &contract_address in &cfx_operations.contracts_requiring_gas_fee_upper_bound {
        let gas_fee_upper_bound = state
            .sponsor_gas_bound(&address_to_cfx(contract_address))
            .map_err(|error| ConfluxEngineError::StateAccess {
                message: format!(
                    "failed to read {phase} Core Space gas sponsor bound for contract {contract_address}: {error}"
                ),
            })?;
        gas_fee_upper_bounds.insert(contract_address, u256_from_cfx(gas_fee_upper_bound));
    }

    let mut sponsorship_contract_admins = BTreeMap::new();
    for &contract_address in &cfx_operations.admin_managed_sponsorship_contracts {
        let admin = state.admin(&address_to_cfx(contract_address)).map_err(|error| {
            ConfluxEngineError::StateAccess {
                message: format!(
                    "failed to read {phase} Core Space admin for sponsorship access rules on contract {contract_address}: {error}"
                ),
            }
        })?;
        sponsorship_contract_admins
            .insert(contract_address, crate::primitive::address_from_cfx(admin));
    }

    let sponsor_contract =
        cfx_parameters::internal_contract_addresses::SPONSOR_WHITELIST_CONTROL_CONTRACT_ADDRESS;
    let mut sponsorship_access_rules = BTreeMap::new();
    for &rule_key in &cfx_operations.sponsorship_access_rule_keys {
        let account_address = match rule_key.account_scope {
            SponsorshipEligibilityTarget::Account(account_address) => {
                address_to_cfx(account_address)
            }
            SponsorshipEligibilityTarget::AllAccounts => cfx_types::Address::zero(),
        };
        let storage_key = SponsorWhitelistStorageKey {
            contract_address: address_to_cfx(rule_key.contract_address),
            account_address,
        };
        let raw_value = state
            .storage_at(
                &sponsor_contract.with_native_space(),
                &storage_key.raw_storage_key(),
            )
            .map_err(|error| ConfluxEngineError::StateAccess {
                message: format!(
                    "failed to read {phase} Core Space sponsorship access rule for contract {}: {error}",
                    rule_key.contract_address
                ),
            })?;
        let enabled = match raw_value {
            value if value.is_zero() => false,
            value if value == cfx_types::U256::one() => true,
            value => {
                return Err(ConfluxEngineError::analysis_failed(format!(
                    "{phase} Core Space sponsorship access rule for contract {} had non-boolean raw value {value}",
                    rule_key.contract_address
                )));
            }
        };
        sponsorship_access_rules.insert(rule_key, enabled);
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
    let total_espace_tokens = cfx_operations
        .requires_total_espace_tokens
        .then(|| u256_from_cfx(state.total_espace_tokens()));

    Ok(CfxStateValues {
        balances,
        sponsor_identities,
        gas_fee_upper_bounds,
        sponsorship_access_rules,
        sponsorship_contract_admins,
        storage_points,
        total_issued: u256_from_cfx(state.total_issued_tokens()),
        total_staking: u256_from_cfx(state.total_staking_tokens()),
        total_espace_tokens,
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
            CfxOperation::CoreSpaceBalanceTransfer {
                position,
                from,
                to,
                amount,
            } => {
                replayed_state.debit_balance(
                    CfxBalanceLocation::CoreSpaceAccount { account: *from },
                    *amount,
                )?;
                replayed_state.credit_balance(
                    CfxBalanceLocation::CoreSpaceAccount { account: *to },
                    *amount,
                )?;
                positioned_core_changes.push(PositionedCoreSpaceChange::new(
                    *position,
                    CoreSpaceChange::Asset(Change::NativeTransfer {
                        from: *from,
                        to: *to,
                        raw_amount: *amount,
                        metadata: NativeMetadata::default(),
                    }),
                ));
            }
            CfxOperation::EspaceBalanceTransfer { from, to, amount } => {
                replayed_state.debit_balance(
                    CfxBalanceLocation::EspaceAccount { account: *from },
                    *amount,
                )?;
                replayed_state
                    .credit_balance(CfxBalanceLocation::EspaceAccount { account: *to }, *amount)?;
            }
            CfxOperation::CrossSpaceTransfer(transfer) => {
                replayed_state
                    .apply_cross_space_transfer(transfer, &mut positioned_core_changes)?;
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
                replayed_state.debit_balance(
                    CfxBalanceLocation::CoreSpaceAccount { account: *account },
                    *amount,
                )?;
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
                    CfxBalanceLocation::CoreSpaceAccount { account: *account },
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
                replayed_state.debit_balance(
                    CfxBalanceLocation::CoreSpaceAccount { account: *account },
                    *amount,
                )?;
                replayed_state.debit_total_issued(*amount, "a native balance burn")?;
                positioned_core_changes.push(PositionedCoreSpaceChange::new(
                    *position,
                    CoreSpaceChange::NativeBurn {
                        from: *account,
                        raw_amount: *amount,
                        metadata: NativeMetadata::default(),
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
            CfxOperation::SponsorshipFunding(funding) => {
                replayed_state.apply_sponsorship_funding(funding, &mut positioned_core_changes)?;
            }
            CfxOperation::SponsorshipStandaloneRefund(refund) => {
                replayed_state
                    .apply_standalone_sponsorship_refund(refund, &mut positioned_core_changes)?;
            }
            CfxOperation::SponsorshipAccessRule(update) => {
                replayed_state
                    .apply_sponsorship_access_rule_update(update, &mut positioned_core_changes)?;
            }
            CfxOperation::StoragePointConversion(conversion) => {
                replayed_state
                    .apply_storage_point_conversion(conversion, &mut positioned_core_changes)?;
            }
            CfxOperation::StorageCollateralRelease(release) => {
                replayed_state.apply_storage_collateral_release(release)?;
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
    fn apply_cross_space_transfer(
        &mut self,
        transfer: &CrossSpaceTransferOperation,
        positioned_changes: &mut Vec<PositionedCoreSpaceChange>,
    ) -> Result<(), ConfluxEngineError> {
        self.debit_balance(cross_space_balance_location(transfer.from), transfer.amount)?;
        self.credit_balance(cross_space_balance_location(transfer.to), transfer.amount)?;

        let total_espace_tokens = self.total_espace_tokens.as_mut().ok_or_else(|| {
            ConfluxEngineError::analysis_failed(
                "before Core cross-space total eSpace tokens are missing",
            )
        })?;
        match (transfer.from, transfer.to) {
            (CrossSpaceAddress::CoreSpace(_), CrossSpaceAddress::Espace(_)) => {
                *total_espace_tokens = total_espace_tokens
                    .checked_add(transfer.amount)
                    .ok_or_else(|| {
                        ConfluxEngineError::analysis_failed(
                            "Core total eSpace tokens overflowed during a cross-space transfer",
                        )
                    })?;
            }
            (CrossSpaceAddress::Espace(_), CrossSpaceAddress::CoreSpace(_)) => {
                *total_espace_tokens = total_espace_tokens
                    .checked_sub(transfer.amount)
                    .ok_or_else(|| {
                        ConfluxEngineError::analysis_failed(
                            "Core total eSpace tokens underflowed during a cross-space withdrawal",
                        )
                    })?;
            }
            _ => {
                return Err(ConfluxEngineError::analysis_failed(
                    "Core cross-space transfer used two endpoints in the same space",
                ));
            }
        }
        positioned_changes.push(PositionedCoreSpaceChange::new(
            transfer.position,
            CoreSpaceChange::CrossSpaceTransfer {
                from: transfer.from,
                to: transfer.to,
                raw_amount: transfer.amount,
            },
        ));
        Ok(())
    }

    fn apply_sponsorship_funding(
        &mut self,
        funding: &SponsorshipFundingOperation,
        positioned_changes: &mut Vec<PositionedCoreSpaceChange>,
    ) -> Result<(), ConfluxEngineError> {
        let sponsored_resource = funding.funding_terms.sponsored_resource();
        let resource_location = SponsorResourceLocation {
            resource: sponsored_resource,
            contract_address: funding.contract_address,
        };
        let new_sponsor = (!funding.sponsor.is_zero()).then_some(funding.sponsor);
        if !funding.gross_deposit_amount.is_zero() && new_sponsor.is_none() {
            return Err(ConfluxEngineError::analysis_failed(
                "Core Space nonzero sponsorship deposit had no sponsor identity",
            ));
        }

        self.debit_balance(
            CfxBalanceLocation::CoreSpaceAccount {
                account: funding.sponsor,
            },
            funding.gross_deposit_amount,
        )?;

        let current_sponsor = self.sponsor_identity(resource_location)?;
        if let Some(refund) = funding.refund {
            if refund.resource != sponsored_resource
                || refund.contract_address != funding.contract_address
                || current_sponsor != Some(refund.sponsor)
                || Some(refund.sponsor) == new_sponsor
            {
                return Err(ConfluxEngineError::analysis_failed(format!(
                    "Core Space {:?} sponsorship replacement identity did not match its refund",
                    sponsored_resource
                )));
            }
            let pool_location = sponsored_resource.pool_location(funding.contract_address);
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
            match sponsored_resource {
                SponsoredResource::Gas if !direct_compensation.is_zero() => {
                    return Err(ConfluxEngineError::analysis_failed(
                        "Core Space gas sponsorship replacement contained direct collateral compensation",
                    ));
                }
                SponsoredResource::StorageCollateral => {
                    self.verify_exact_balance(
                        CfxBalanceLocation::StorageCollateral {
                            contract_address: funding.contract_address,
                        },
                        direct_compensation,
                        "replacement collateral compensation",
                    )?;
                }
                SponsoredResource::Gas => {}
            }
            self.debit_balance(pool_location, refund.pool_refund_amount)?;
            self.credit_balance(
                CfxBalanceLocation::CoreSpaceAccount {
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
                sponsored_resource
            )));
        }

        self.credit_balance(
            sponsored_resource.pool_location(funding.contract_address),
            funding.pool_deposit_amount,
        )?;
        self.set_sponsor_identity(resource_location, new_sponsor)?;
        let changed_configuration = match funding.funding_terms {
            SponsorshipFundingTerms::Gas {
                gas_fee_upper_bound,
            } => {
                let gas_fee_upper_bound_before =
                    self.gas_fee_upper_bound(funding.contract_address)?;
                let gas_fee_upper_bound_after =
                    if current_sponsor == new_sponsor && funding.pool_deposit_amount.is_zero() {
                        gas_fee_upper_bound_before
                    } else {
                        gas_fee_upper_bound
                    };
                self.set_gas_fee_upper_bound(funding.contract_address, gas_fee_upper_bound_after)?;
                (current_sponsor != new_sponsor
                    || gas_fee_upper_bound_before != gas_fee_upper_bound_after)
                    .then_some(SponsorshipConfiguration::Gas {
                        sponsor_before: current_sponsor,
                        sponsor_after: new_sponsor,
                        max_sponsored_gas_fee_raw_amount_before: gas_fee_upper_bound_before,
                        max_sponsored_gas_fee_raw_amount_after: gas_fee_upper_bound_after,
                    })
            }
            SponsorshipFundingTerms::StorageCollateral => (current_sponsor != new_sponsor)
                .then_some(SponsorshipConfiguration::StorageCollateral {
                    sponsor_before: current_sponsor,
                    sponsor_after: new_sponsor,
                }),
        };
        if let Some(configuration) = changed_configuration {
            positioned_changes.push(PositionedCoreSpaceChange::new(
                funding.position,
                CoreSpaceChange::SponsorshipConfiguration {
                    contract_address: funding.contract_address,
                    configuration,
                },
            ));
        }
        if !funding.gross_deposit_amount.is_zero() {
            positioned_changes.push(PositionedCoreSpaceChange::new(
                funding.position,
                CoreSpaceChange::SponsorshipDeposit {
                    sponsored_resource,
                    sponsor: funding.sponsor,
                    contract_address: funding.contract_address,
                    raw_amount: funding.gross_deposit_amount,
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
            CfxBalanceLocation::CoreSpaceAccount {
                account: refund.sponsor,
            },
            refund.gross_refund_amount,
        )?;
        self.set_sponsor_identity(resource_location, None)?;
        let configuration = match refund.resource {
            SponsoredResource::Gas => {
                let gas_fee_upper_bound_before =
                    self.gas_fee_upper_bound(refund.contract_address)?;
                self.set_gas_fee_upper_bound(refund.contract_address, U256::ZERO)?;
                SponsorshipConfiguration::Gas {
                    sponsor_before: Some(refund.sponsor),
                    sponsor_after: None,
                    max_sponsored_gas_fee_raw_amount_before: gas_fee_upper_bound_before,
                    max_sponsored_gas_fee_raw_amount_after: U256::ZERO,
                }
            }
            SponsoredResource::StorageCollateral => SponsorshipConfiguration::StorageCollateral {
                sponsor_before: Some(refund.sponsor),
                sponsor_after: None,
            },
        };
        positioned_changes.push(PositionedCoreSpaceChange::new(
            refund.position,
            CoreSpaceChange::SponsorshipConfiguration {
                contract_address: refund.contract_address,
                configuration,
            },
        ));
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

    fn apply_sponsorship_access_rule_update(
        &mut self,
        update: &SponsorshipAccessRuleUpdate,
        positioned_changes: &mut Vec<PositionedCoreSpaceChange>,
    ) -> Result<(), ConfluxEngineError> {
        if update.caller_role == SponsorshipAccessCallerRole::ContractAdmin {
            if !address_to_cfx(update.contract_address).is_contract_address() {
                return Ok(());
            }
            let admin = self
                .sponsorship_contract_admins
                .get(&update.contract_address)
                .copied()
                .ok_or_else(|| {
                    ConfluxEngineError::analysis_failed(format!(
                        "before Core Space sponsorship access-rule admin is missing for contract {}",
                        update.contract_address
                    ))
                })?;
            if admin != update.caller_address {
                return Ok(());
            }
        }

        let rule_key = update.key();
        let enabled_before = self
            .sponsorship_access_rules
            .get_mut(&rule_key)
            .ok_or_else(|| {
                ConfluxEngineError::analysis_failed(format!(
                    "before Core Space sponsorship access rule is missing for contract {}",
                    update.contract_address
                ))
            })?;
        if *enabled_before == update.enabled_after {
            return Ok(());
        }
        let previous = *enabled_before;
        *enabled_before = update.enabled_after;
        positioned_changes.push(PositionedCoreSpaceChange::new(
            update.position,
            CoreSpaceChange::SponsorshipEligibilityRule {
                contract_address: update.contract_address,
                applies_to: update.account_scope,
                enabled_before: previous,
                enabled_after: update.enabled_after,
            },
        ));
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

    fn apply_storage_collateral_release(
        &mut self,
        release: &StorageCollateralReleaseOperation,
    ) -> Result<(), ConfluxEngineError> {
        let collateral_location = CfxBalanceLocation::StorageCollateral {
            contract_address: release.contract_address,
        };
        let token_collateral = self
            .balances
            .get(&collateral_location)
            .copied()
            .ok_or_else(|| {
                ConfluxEngineError::analysis_failed(format!(
                    "before Core Space token storage collateral is missing for contract {}",
                    release.contract_address
                ))
            })?;
        let refundable_amount = token_collateral.min(release.total_released_amount);
        let burnt_amount = release
            .total_released_amount
            .checked_sub(refundable_amount)
            .ok_or_else(|| {
                ConfluxEngineError::analysis_failed(
                    "Core Space storage release refundable amount exceeded its total",
                )
            })?;

        let storage_point_refund = match self.storage_points.get_mut(&release.contract_address) {
            Some(Some(points)) => {
                let refund = points.used.min(refundable_amount);
                points.used = points.used.checked_sub(refund).ok_or_else(|| {
                    ConfluxEngineError::analysis_failed(format!(
                        "Core Space used storage points underflowed for contract {}",
                        release.contract_address
                    ))
                })?;
                points.unused = points.unused.checked_add(refund).ok_or_else(|| {
                    ConfluxEngineError::analysis_failed(format!(
                        "Core Space unused storage points overflowed for contract {}",
                        release.contract_address
                    ))
                })?;
                refund
            }
            Some(None) => U256::ZERO,
            None => {
                return Err(ConfluxEngineError::analysis_failed(format!(
                    "before Core Space storage points are missing for contract {}",
                    release.contract_address
                )));
            }
        };

        let expected_non_point_amount = release
            .total_released_amount
            .checked_sub(storage_point_refund)
            .ok_or_else(|| {
                ConfluxEngineError::analysis_failed(
                    "Core Space storage-point refund exceeded its total release",
                )
            })?;
        if release.observed_non_point_amount != expected_non_point_amount {
            return Err(ConfluxEngineError::analysis_failed(format!(
                "Core Space storage release movement mismatch for contract {}: observed {}, expected {}",
                release.contract_address,
                release.observed_non_point_amount,
                expected_non_point_amount
            )));
        }

        let token_refund = refundable_amount
            .checked_sub(storage_point_refund)
            .ok_or_else(|| {
                ConfluxEngineError::analysis_failed(
                    "Core Space storage-point refund exceeded refundable token collateral",
                )
            })?;
        self.debit_balance(collateral_location, token_refund)?;
        self.credit_balance(
            CfxBalanceLocation::StorageSponsor {
                contract_address: release.contract_address,
            },
            token_refund,
        )?;
        self.debit_total_issued(burnt_amount, "a storage collateral release")?;

        let total_storage_debit = release
            .total_released_amount
            .checked_sub(storage_point_refund)
            .ok_or_else(|| {
                ConfluxEngineError::analysis_failed(
                    "Core Space storage-point refund exceeded total storage release",
                )
            })?;
        let globals = self.storage_point_globals.as_mut().ok_or_else(|| {
            ConfluxEngineError::analysis_failed(
                "before Core Space storage-point globals are missing for a storage release",
            )
        })?;
        globals.total_storage = globals
            .total_storage
            .checked_sub(total_storage_debit)
            .ok_or_else(|| {
                ConfluxEngineError::analysis_failed(
                    "Core Space total storage underflowed during a storage release",
                )
            })?;
        globals.used_storage_points = globals
            .used_storage_points
            .checked_sub(storage_point_refund)
            .ok_or_else(|| {
                ConfluxEngineError::analysis_failed(
                    "Core Space used storage points underflowed during a storage release",
                )
            })?;

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

    fn gas_fee_upper_bound(&self, contract_address: Address) -> Result<U256, ConfluxEngineError> {
        self.gas_fee_upper_bounds
            .get(&contract_address)
            .copied()
            .ok_or_else(|| {
                ConfluxEngineError::analysis_failed(format!(
                    "before Core Space gas sponsor bound is missing for contract {contract_address}"
                ))
            })
    }

    fn set_gas_fee_upper_bound(
        &mut self,
        contract_address: Address,
        gas_fee_upper_bound: U256,
    ) -> Result<(), ConfluxEngineError> {
        let current = self
            .gas_fee_upper_bounds
            .get_mut(&contract_address)
            .ok_or_else(|| {
                ConfluxEngineError::analysis_failed(format!(
                    "before Core Space gas sponsor bound is missing for contract {contract_address}"
                ))
            })?;
        *current = gas_fee_upper_bound;
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
                "before Core simulation CFX balance is missing for {location}"
            ))
        })?;
        *balance = balance.checked_sub(amount).ok_or_else(|| {
            ConfluxEngineError::analysis_failed(format!(
                "Core simulation CFX balance underflow for {location}: balance {balance}, debit {amount}"
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
                "before Core simulation CFX balance is missing for {location}"
            ))
        })?;
        *balance = balance.checked_add(amount).ok_or_else(|| {
            ConfluxEngineError::analysis_failed(format!(
                "Core simulation CFX balance overflow for {location}: balance {balance}, credit {amount}"
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
                    "after Core simulation CFX balance is missing for {location}"
                ))
            })?;
            if replayed_balance != after_balance {
                return Err(ConfluxEngineError::analysis_failed(format!(
                    "Core simulation CFX balance mismatch for {location}: replayed {replayed_balance}, after {after_balance}"
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

        for (contract_address, replayed_gas_fee_upper_bound) in &self.gas_fee_upper_bounds {
            let after_gas_fee_upper_bound = after_state
                .gas_fee_upper_bounds
                .get(contract_address)
                .ok_or_else(|| {
                    ConfluxEngineError::analysis_failed(format!(
                        "after Core Space gas sponsor bound is missing for contract {contract_address}"
                    ))
                })?;
            if replayed_gas_fee_upper_bound != after_gas_fee_upper_bound {
                return Err(ConfluxEngineError::analysis_failed(format!(
                    "Core Space gas sponsor bound mismatch for contract {contract_address}: replayed {replayed_gas_fee_upper_bound}, after {after_gas_fee_upper_bound}"
                )));
            }
        }

        for (rule_key, replayed_rule) in &self.sponsorship_access_rules {
            let after_rule = after_state
                .sponsorship_access_rules
                .get(rule_key)
                .ok_or_else(|| {
                    ConfluxEngineError::analysis_failed(format!(
                        "after Core Space sponsorship access rule is missing for contract {}",
                        rule_key.contract_address
                    ))
                })?;
            if replayed_rule != after_rule {
                return Err(ConfluxEngineError::analysis_failed(format!(
                    "Core Space sponsorship access rule mismatch for contract {}: replayed {replayed_rule}, after {after_rule}",
                    rule_key.contract_address
                )));
            }
        }

        for (contract_address, before_admin) in &self.sponsorship_contract_admins {
            let after_admin = after_state
                .sponsorship_contract_admins
                .get(contract_address)
                .ok_or_else(|| {
                    ConfluxEngineError::analysis_failed(format!(
                        "after Core Space sponsorship access-rule admin is missing for contract {contract_address}"
                    ))
                })?;
            if before_admin != after_admin {
                return Err(ConfluxEngineError::analysis_failed(format!(
                    "Core Space sponsorship access-rule admin changed during analysis for contract {contract_address}"
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
        if self.total_espace_tokens != after_state.total_espace_tokens {
            return Err(ConfluxEngineError::analysis_failed(format!(
                "Core total eSpace tokens mismatch: replayed {:?}, after {:?}",
                self.total_espace_tokens, after_state.total_espace_tokens
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
