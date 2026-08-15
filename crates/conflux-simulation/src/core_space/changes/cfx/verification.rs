use std::collections::BTreeMap;

use alloy_primitives::{Address, U256};
use cfx_executor::state::State;
use cfx_types::{AddressSpaceUtil, address_util::AddressUtil};

use super::{
    AdminOperation, BasicCfxOperation, CfxBalanceLocation, CfxOperation, CfxOperations,
    ContractAdminSetOperation, CrossSpaceTransferOperation, SponsorResourceLocation,
    SponsoredResource, SponsorshipAccessCallerRole, SponsorshipAccessRuleKey,
    SponsorshipAccessRuleUpdate, SponsorshipFundingOperation, SponsorshipFundingTerms,
    SponsorshipOperation, SponsorshipRefundOperation, StorageCollateralReleaseOperation,
    StoragePointConversionOperation,
};
use crate::{
    core_space::CoreSpaceChangesError,
    core_space::changes::{
        PendingCoreSpaceChange, PendingCrossSpaceAddress, PendingSponsorshipAccessRuleScope,
        PendingSponsorshipReplacement, PositionedCoreSpaceChange,
        SponsoredResource as PublicSponsoredResource,
        SponsorshipFundingTerms as PublicSponsorshipFundingTerms, StatePhase,
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
    contract_admins: BTreeMap<Address, Address>,
    contract_exists: BTreeMap<Address, bool>,
    storage_points: BTreeMap<Address, Option<StoragePointValues>>,
    total_issued: U256,
    total_staking: U256,
    total_espace_tokens: Option<U256>,
    storage_point_globals: Option<StoragePointGlobalValues>,
}

impl CfxStateValues {
    pub(crate) fn staking_balance(&self, account: Address) -> Result<U256, CoreSpaceChangesError> {
        self.balances
            .get(&CfxBalanceLocation::Staking { account })
            .copied()
            .ok_or_else(|| {
                CoreSpaceChangesError::internal_invariant(format!(
                    "Core Space staking balance was not collected for {account}"
                ))
            })
    }
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
) -> Result<CfxStateValues, CoreSpaceChangesError> {
    let mut balances = BTreeMap::new();
    for &location in &cfx_operations.balance_locations {
        let balance = match location {
            CfxBalanceLocation::CoreSpaceAccount { account } => state
                .balance(&address_to_cfx(account).with_native_space())
                .map_err(|error| {
                    CoreSpaceChangesError::state_read(
                        format!("read {phase} Core Space balance for {location}"),
                        error,
                    )
                })?,
            CfxBalanceLocation::EspaceAccount { account } => state
                .balance(&address_to_cfx(account).with_evm_space())
                .map_err(|error| {
                    CoreSpaceChangesError::state_read(
                        format!("read {phase} Core simulation balance for {location}"),
                        error,
                    )
                })?,
            CfxBalanceLocation::Staking { account } => state
                .staking_balance(&address_to_cfx(account))
                .map_err(|error| {
                    CoreSpaceChangesError::state_read(
                        format!("read {phase} Core Space balance for {location}"),
                        error,
                    )
                })?,
            CfxBalanceLocation::GasSponsor { contract_address } => state
                .sponsor_balance_for_gas(&address_to_cfx(contract_address))
                .map_err(|error| {
                    CoreSpaceChangesError::state_read(
                        format!("read {phase} Core Space balance for {location}"),
                        error,
                    )
                })?,
            CfxBalanceLocation::StorageSponsor { contract_address } => state
                .sponsor_balance_for_collateral(&address_to_cfx(contract_address))
                .map_err(|error| {
                    CoreSpaceChangesError::state_read(
                        format!("read {phase} Core Space balance for {location}"),
                        error,
                    )
                })?,
            CfxBalanceLocation::StorageCollateral { contract_address } => state
                .token_collateral_for_storage(&address_to_cfx(contract_address))
                .map_err(|error| {
                    CoreSpaceChangesError::state_read(
                        format!("read {phase} Core Space balance for {location}"),
                        error,
                    )
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
        .map_err(|error| {
            CoreSpaceChangesError::state_read(
                format!(
                    "read {phase} Core Space {:?} sponsor identity for contract {}",
                    location.resource, location.contract_address
                ),
                error,
            )
        })?
        .map(crate::primitive::address_from_cfx);
        sponsor_identities.insert(location, sponsor);
    }

    let mut gas_fee_upper_bounds = BTreeMap::new();
    for &contract_address in &cfx_operations.contracts_requiring_gas_fee_upper_bound {
        let gas_fee_upper_bound = state
            .sponsor_gas_bound(&address_to_cfx(contract_address))
            .map_err(|error| {
                CoreSpaceChangesError::state_read(
                    format!(
                        "read {phase} Core Space gas sponsor bound for contract {contract_address}"
                    ),
                    error,
                )
            })?;
        gas_fee_upper_bounds.insert(contract_address, u256_from_cfx(gas_fee_upper_bound));
    }

    let mut contract_admins = BTreeMap::new();
    let mut contract_exists = BTreeMap::new();
    for &contract_address in &cfx_operations.contract_admins {
        let contract_address_with_space = address_to_cfx(contract_address).with_native_space();
        let exists = state
            .exists(&contract_address_with_space)
            .map_err(|error| {
                CoreSpaceChangesError::state_read(
                    format!("read {phase} Core Space existence for contract {contract_address}"),
                    error,
                )
            })?;
        let admin = state
            .admin(&contract_address_with_space.address)
            .map_err(|error| {
                CoreSpaceChangesError::state_read(
                    format!("read {phase} Core Space admin for contract {contract_address}"),
                    error,
                )
            })?;
        contract_admins.insert(contract_address, crate::primitive::address_from_cfx(admin));
        contract_exists.insert(contract_address, exists);
    }

    let sponsor_contract =
        cfx_parameters::internal_contract_addresses::SPONSOR_WHITELIST_CONTROL_CONTRACT_ADDRESS;
    let mut sponsorship_access_rules = BTreeMap::new();
    for &rule_key in &cfx_operations.sponsorship_access_rule_keys {
        let account_address = match rule_key.account_scope {
            PendingSponsorshipAccessRuleScope::Account(account_address) => {
                address_to_cfx(account_address)
            }
            PendingSponsorshipAccessRuleScope::AllAccounts => cfx_types::Address::zero(),
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
            .map_err(|error| {
                CoreSpaceChangesError::state_read(
                    format!(
                        "read {phase} Core Space sponsorship access rule for contract {}",
                        rule_key.contract_address
                    ),
                    error,
                )
            })?;
        let enabled = match raw_value {
            value if value.is_zero() => false,
            value if value == cfx_types::U256::one() => true,
            value => {
                return Err(CoreSpaceChangesError::inconsistent_execution(format!(
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
            .map_err(|error| {
                CoreSpaceChangesError::state_read(
                    format!("read {phase} Core Space storage points for contract {account}"),
                    error,
                )
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
        contract_admins,
        contract_exists,
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
) -> Result<Vec<PositionedCoreSpaceChange>, CoreSpaceChangesError> {
    if let Some(burnt_fee) = burnt_fee
        && burnt_fee > execution_fee
    {
        return Err(CoreSpaceChangesError::inconsistent_execution(format!(
            "Core Space burnt fee exceeds total fee: burnt {burnt_fee}, total {execution_fee}"
        )));
    }

    let mut replayed_state = before_state.clone();
    let mut positioned_core_changes = Vec::new();
    let mut precharged_fee = U256::ZERO;
    let mut refunded_fee = U256::ZERO;

    for operation in &cfx_operations.operations {
        match operation {
            CfxOperation::Basic(BasicCfxOperation::CoreSpaceBalanceTransfer {
                position,
                from,
                to,
                amount,
            }) => {
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
                    PendingCoreSpaceChange::NativeTransfer {
                        from: *from,
                        to: *to,
                        raw_amount: *amount,
                    },
                ));
            }
            CfxOperation::Basic(BasicCfxOperation::EspaceBalanceTransfer { from, to, amount }) => {
                replayed_state.debit_balance(
                    CfxBalanceLocation::EspaceAccount { account: *from },
                    *amount,
                )?;
                replayed_state
                    .credit_balance(CfxBalanceLocation::EspaceAccount { account: *to }, *amount)?;
            }
            CfxOperation::CrossSpace(transfer) => {
                replayed_state
                    .apply_cross_space_transfer(transfer, &mut positioned_core_changes)?;
            }
            CfxOperation::Basic(BasicCfxOperation::GasPrecharge { payer, amount }) => {
                verify_gas_fee_location("precharge payer", *payer, expected_gas_fee_payer)?;
                precharged_fee = precharged_fee.checked_add(*amount).ok_or_else(|| {
                    CoreSpaceChangesError::inconsistent_execution(
                        "Core Space gas precharge overflowed during CFX analysis",
                    )
                })?;
                replayed_state.debit_balance(*payer, *amount)?;
            }
            CfxOperation::Basic(BasicCfxOperation::GasRefund { recipient, amount }) => {
                verify_gas_fee_location("refund recipient", *recipient, expected_gas_fee_payer)?;
                refunded_fee = refunded_fee.checked_add(*amount).ok_or_else(|| {
                    CoreSpaceChangesError::inconsistent_execution(
                        "Core Space gas refund overflowed during CFX analysis",
                    )
                })?;
                replayed_state.credit_balance(*recipient, *amount)?;
            }
            CfxOperation::Basic(BasicCfxOperation::StakingDeposit { account, amount }) => {
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
                        CoreSpaceChangesError::inconsistent_execution(
                            "Core Space total staking overflowed while replaying a deposit",
                        )
                    })?;
            }
            CfxOperation::Basic(BasicCfxOperation::StakingWithdrawal {
                account,
                principal_amount,
                reward_amount,
            }) => {
                replayed_state.debit_balance(
                    CfxBalanceLocation::Staking { account: *account },
                    *principal_amount,
                )?;
                let withdrawal_credit =
                    principal_amount
                        .checked_add(*reward_amount)
                        .ok_or_else(|| {
                            CoreSpaceChangesError::inconsistent_execution(format!(
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
                        CoreSpaceChangesError::inconsistent_execution(
                            "Core Space total staking underflowed while replaying a withdrawal",
                        )
                    })?;
                replayed_state.total_issued = replayed_state
                    .total_issued
                    .checked_add(*reward_amount)
                    .ok_or_else(|| {
                        CoreSpaceChangesError::inconsistent_execution(
                            "Core Space total issued overflowed while replaying staking interest",
                        )
                    })?;
            }
            CfxOperation::Basic(BasicCfxOperation::NativeBurn {
                position,
                account,
                amount,
            }) => {
                replayed_state.debit_balance(
                    CfxBalanceLocation::CoreSpaceAccount { account: *account },
                    *amount,
                )?;
                replayed_state.debit_total_issued(*amount, "a native balance burn")?;
                positioned_core_changes.push(PositionedCoreSpaceChange::new(
                    *position,
                    PendingCoreSpaceChange::NativeBurn {
                        from: *account,
                        raw_amount: *amount,
                    },
                ));
            }
            CfxOperation::Admin(AdminOperation::Initialize {
                contract_address,
                admin,
            }) => {
                replayed_state.initialize_contract_admin(*contract_address, *admin);
            }
            CfxOperation::Admin(AdminOperation::Set(update)) => {
                replayed_state.apply_contract_admin_set(update, &mut positioned_core_changes)?;
            }
            CfxOperation::Sponsorship(SponsorshipOperation::Funding(funding)) => {
                replayed_state.apply_sponsorship_funding(funding, &mut positioned_core_changes)?;
            }
            CfxOperation::Sponsorship(SponsorshipOperation::StandaloneRefund(refund)) => {
                replayed_state.apply_standalone_sponsorship_refund(refund)?;
            }
            CfxOperation::Sponsorship(SponsorshipOperation::AccessRule(update)) => {
                replayed_state
                    .apply_sponsorship_access_rule_update(update, &mut positioned_core_changes)?;
            }
            CfxOperation::Sponsorship(SponsorshipOperation::StoragePointConversion(conversion)) => {
                replayed_state
                    .apply_storage_point_conversion(conversion, &mut positioned_core_changes)?;
            }
            CfxOperation::Basic(BasicCfxOperation::StorageCollateralRelease(release)) => {
                replayed_state.apply_storage_collateral_release(release)?;
            }
        }
    }

    let net_fee_from_operations = precharged_fee.checked_sub(refunded_fee).ok_or_else(|| {
        CoreSpaceChangesError::inconsistent_execution("Core Space gas refund exceeded precharge")
    })?;
    if net_fee_from_operations != execution_fee {
        return Err(CoreSpaceChangesError::inconsistent_execution(format!(
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
    fn initialize_contract_admin(&mut self, contract_address: Address, admin: Address) {
        if let Some(current_admin) = self.contract_admins.get_mut(&contract_address) {
            *current_admin = admin;
        }
        if let Some(exists) = self.contract_exists.get_mut(&contract_address) {
            *exists = true;
        }
    }

    fn apply_contract_admin_set(
        &mut self,
        update: &ContractAdminSetOperation,
        positioned_changes: &mut Vec<PositionedCoreSpaceChange>,
    ) -> Result<(), CoreSpaceChangesError> {
        let contract_address = address_to_cfx(update.contract_address);
        let new_admin = address_to_cfx(update.new_admin);
        if !contract_address.is_contract_address()
            || !(new_admin.is_user_account_address() || new_admin.is_null_address())
        {
            return Ok(());
        }
        let exists = self
            .contract_exists
            .get(&update.contract_address)
            .copied()
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(format!(
                    "before Core Space contract existence is missing for {}",
                    update.contract_address
                ))
            })?;
        if !exists {
            return Ok(());
        }

        let current_admin = self
            .contract_admins
            .get_mut(&update.contract_address)
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(format!(
                    "before Core Space admin is missing for contract {}",
                    update.contract_address
                ))
            })?;
        let creation_clear = update.is_creation_frame && update.new_admin.is_zero();
        if *current_admin != update.caller && !creation_clear {
            return Ok(());
        }
        if *current_admin == update.new_admin {
            return Ok(());
        }

        *current_admin = update.new_admin;
        positioned_changes.push(PositionedCoreSpaceChange::new(
            update.position,
            PendingCoreSpaceChange::ContractAdminSet {
                contract_address: update.contract_address,
                admin: (!update.new_admin.is_zero()).then_some(update.new_admin),
            },
        ));
        Ok(())
    }

    fn apply_cross_space_transfer(
        &mut self,
        transfer: &CrossSpaceTransferOperation,
        positioned_changes: &mut Vec<PositionedCoreSpaceChange>,
    ) -> Result<(), CoreSpaceChangesError> {
        match *transfer {
            CrossSpaceTransferOperation::ToEspace {
                position,
                core_sender,
                mapped_sender,
                receiver,
                amount,
            } => {
                self.debit_balance(
                    CfxBalanceLocation::CoreSpaceAccount {
                        account: core_sender,
                    },
                    amount,
                )?;
                self.credit_balance(
                    CfxBalanceLocation::EspaceAccount {
                        account: mapped_sender,
                    },
                    amount,
                )?;
                self.debit_balance(
                    CfxBalanceLocation::EspaceAccount {
                        account: mapped_sender,
                    },
                    amount,
                )?;
                self.credit_balance(
                    CfxBalanceLocation::EspaceAccount { account: receiver },
                    amount,
                )?;
                let total_espace_tokens = self.total_espace_tokens.as_mut().ok_or_else(|| {
                    CoreSpaceChangesError::inconsistent_execution(
                        "before Core cross-space total eSpace tokens are missing",
                    )
                })?;
                *total_espace_tokens =
                    total_espace_tokens.checked_add(amount).ok_or_else(|| {
                        CoreSpaceChangesError::inconsistent_execution(
                            "Core total eSpace tokens overflowed during a cross-space transfer",
                        )
                    })?;
                if !amount.is_zero() {
                    positioned_changes.push(PositionedCoreSpaceChange::new(
                        position,
                        PendingCoreSpaceChange::CrossSpaceNativeTransfer {
                            from: PendingCrossSpaceAddress::CoreSpace(core_sender),
                            to: PendingCrossSpaceAddress::Espace(receiver),
                            raw_amount: amount,
                        },
                    ));
                }
            }
            CrossSpaceTransferOperation::ToCoreSpace {
                position,
                mapped_sender,
                core_receiver,
                amount,
            } => {
                self.debit_balance(
                    CfxBalanceLocation::EspaceAccount {
                        account: mapped_sender,
                    },
                    amount,
                )?;
                self.credit_balance(
                    CfxBalanceLocation::CoreSpaceAccount {
                        account: core_receiver,
                    },
                    amount,
                )?;
                let total_espace_tokens = self.total_espace_tokens.as_mut().ok_or_else(|| {
                    CoreSpaceChangesError::inconsistent_execution(
                        "before Core cross-space total eSpace tokens are missing",
                    )
                })?;
                *total_espace_tokens =
                    total_espace_tokens.checked_sub(amount).ok_or_else(|| {
                        CoreSpaceChangesError::inconsistent_execution(
                            "Core total eSpace tokens underflowed during a cross-space withdrawal",
                        )
                    })?;
                if !amount.is_zero() {
                    positioned_changes.push(PositionedCoreSpaceChange::new(
                        position,
                        PendingCoreSpaceChange::CrossSpaceNativeTransfer {
                            from: PendingCrossSpaceAddress::Espace(mapped_sender),
                            to: PendingCrossSpaceAddress::CoreSpace(core_receiver),
                            raw_amount: amount,
                        },
                    ));
                }
            }
        }
        Ok(())
    }

    fn apply_sponsorship_funding(
        &mut self,
        funding: &SponsorshipFundingOperation,
        positioned_changes: &mut Vec<PositionedCoreSpaceChange>,
    ) -> Result<(), CoreSpaceChangesError> {
        let sponsored_resource = funding.funding_terms.sponsored_resource();
        let resource_location = SponsorResourceLocation {
            resource: sponsored_resource,
            contract_address: funding.contract_address,
        };
        let new_sponsor = (!funding.sponsor.is_zero()).then_some(funding.sponsor);
        if !funding.gross_deposit_amount.is_zero() && new_sponsor.is_none() {
            return Err(CoreSpaceChangesError::inconsistent_execution(
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
        let replacement = if let Some(refund) = funding.refund {
            if refund.resource != sponsored_resource
                || refund.contract_address != funding.contract_address
                || current_sponsor != Some(refund.sponsor)
                || Some(refund.sponsor) == new_sponsor
            {
                return Err(CoreSpaceChangesError::inconsistent_execution(format!(
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
                    CoreSpaceChangesError::inconsistent_execution(
                        "Core Space sponsorship gross refund was smaller than its pool refund",
                    )
                })?;
            match sponsored_resource {
                SponsoredResource::Gas if !direct_compensation.is_zero() => {
                    return Err(CoreSpaceChangesError::inconsistent_execution(
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

            Some(match sponsored_resource {
                SponsoredResource::Gas => PendingSponsorshipReplacement::Gas {
                    previous_sponsor: refund.sponsor,
                    pool_refunded_amount: refund.pool_refund_amount,
                },
                SponsoredResource::StorageCollateral => {
                    PendingSponsorshipReplacement::StorageCollateral {
                        previous_sponsor: refund.sponsor,
                        pool_refunded_amount: refund.pool_refund_amount,
                        collateral_compensation_amount: direct_compensation,
                    }
                }
            })
        } else if current_sponsor.is_some() && current_sponsor != new_sponsor {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space {:?} sponsorship identity changed without a refund transit",
                sponsored_resource
            )));
        } else {
            None
        };

        self.credit_balance(
            sponsored_resource.pool_location(funding.contract_address),
            funding.pool_deposit_amount,
        )?;
        self.set_sponsor_identity(resource_location, new_sponsor)?;
        let (public_resource, public_terms, configuration_changed) = match funding.funding_terms {
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
                (
                    PublicSponsoredResource::Gas,
                    PublicSponsorshipFundingTerms::Gas {
                        gas_fee_upper_bound: gas_fee_upper_bound_after,
                    },
                    current_sponsor != new_sponsor
                        || gas_fee_upper_bound_before != gas_fee_upper_bound_after,
                )
            }
            SponsorshipFundingTerms::StorageCollateral => (
                PublicSponsoredResource::StorageCollateral,
                PublicSponsorshipFundingTerms::StorageCollateral,
                current_sponsor != new_sponsor,
            ),
        };
        if !funding.gross_deposit_amount.is_zero() || configuration_changed || replacement.is_some()
        {
            positioned_changes.push(PositionedCoreSpaceChange::new(
                funding.position,
                PendingCoreSpaceChange::SponsorshipFunding {
                    resource: public_resource,
                    contract_address: funding.contract_address,
                    sponsor: funding.sponsor,
                    contributed_amount: funding.gross_deposit_amount,
                    pool_credited_amount: funding.pool_deposit_amount,
                    terms: public_terms,
                    replacement,
                },
            ));
        }
        Ok(())
    }

    fn apply_standalone_sponsorship_refund(
        &mut self,
        refund: &SponsorshipRefundOperation,
    ) -> Result<(), CoreSpaceChangesError> {
        if refund.gross_refund_amount != refund.pool_refund_amount {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "Core Space standalone sponsorship refund included a non-pool amount",
            ));
        }
        let resource_location = SponsorResourceLocation {
            resource: refund.resource,
            contract_address: refund.contract_address,
        };
        if self.sponsor_identity(resource_location)? != Some(refund.sponsor) {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
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
        // A refund transfer alone does not prove a public sponsorship termination.
        match refund.resource {
            SponsoredResource::Gas => {
                self.set_gas_fee_upper_bound(refund.contract_address, U256::ZERO)?;
            }
            SponsoredResource::StorageCollateral => {}
        }
        Ok(())
    }

    fn apply_sponsorship_access_rule_update(
        &mut self,
        update: &SponsorshipAccessRuleUpdate,
        positioned_changes: &mut Vec<PositionedCoreSpaceChange>,
    ) -> Result<(), CoreSpaceChangesError> {
        match update.caller_role {
            SponsorshipAccessCallerRole::SponsoredContract => {
                if !address_to_cfx(update.caller_address).is_contract_address() {
                    return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                        "Core Space direct sponsorship access-rule caller {} was not a contract address",
                        update.caller_address
                    )));
                }
            }
            SponsorshipAccessCallerRole::ContractAdmin => {
                if !address_to_cfx(update.contract_address).is_contract_address() {
                    return Ok(());
                }
                let exists = self
                    .contract_exists
                    .get(&update.contract_address)
                    .copied()
                    .ok_or_else(|| {
                        CoreSpaceChangesError::inconsistent_execution(format!(
                            "before Core Space contract existence is missing for sponsorship access rules on contract {}",
                            update.contract_address
                        ))
                    })?;
                if !exists {
                    return Ok(());
                }
                let admin = self
                    .contract_admins
                    .get(&update.contract_address)
                    .copied()
                    .ok_or_else(|| {
                        CoreSpaceChangesError::inconsistent_execution(format!(
                            "before Core Space sponsorship access-rule admin is missing for contract {}",
                            update.contract_address
                        ))
                    })?;
                if admin != update.caller_address {
                    return Ok(());
                }
            }
        }

        let rule_key = update.key();
        let enabled_before = self
            .sponsorship_access_rules
            .get_mut(&rule_key)
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(format!(
                    "before Core Space sponsorship access rule is missing for contract {}",
                    update.contract_address
                ))
            })?;
        if *enabled_before == update.enabled_after {
            return Ok(());
        }
        *enabled_before = update.enabled_after;
        positioned_changes.push(PositionedCoreSpaceChange::new(
            update.position,
            PendingCoreSpaceChange::SponsorshipAccessRuleSet {
                contract_address: update.contract_address,
                scope: update.account_scope,
                enabled: update.enabled_after,
            },
        ));
        Ok(())
    }

    fn apply_storage_point_conversion(
        &mut self,
        conversion: &StoragePointConversionOperation,
        positioned_changes: &mut Vec<PositionedCoreSpaceChange>,
    ) -> Result<(), CoreSpaceChangesError> {
        let converted_amount = conversion
            .from_sponsor_pool
            .checked_add(conversion.from_storage_collateral)
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(
                    "Core Space storage-point conversion amount overflowed",
                )
            })?;
        if converted_amount.is_zero() {
            return Err(CoreSpaceChangesError::inconsistent_execution(
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
                CoreSpaceChangesError::inconsistent_execution(format!(
                    "before Core Space storage points are missing for contract {}",
                    conversion.contract_address
                ))
            })?;
        match storage_points {
            Some(points) => {
                if !conversion.from_storage_collateral.is_zero() {
                    return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                        "Core Space initialized contract {} converted token collateral again",
                        conversion.contract_address
                    )));
                }
                points.unused = points
                    .unused
                    .checked_add(conversion.from_sponsor_pool)
                    .ok_or_else(|| {
                        CoreSpaceChangesError::inconsistent_execution(format!(
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
            CoreSpaceChangesError::inconsistent_execution(
                "before Core Space storage-point globals are missing",
            )
        })?;
        globals.total_storage = globals
            .total_storage
            .checked_sub(conversion.from_storage_collateral)
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(
                    "Core Space total storage underflowed during storage-point conversion",
                )
            })?;
        globals.used_storage_points = globals
            .used_storage_points
            .checked_add(conversion.from_storage_collateral)
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(
                    "Core Space used storage points overflowed during conversion",
                )
            })?;
        globals.converted_storage_points = globals
            .converted_storage_points
            .checked_add(converted_amount)
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(
                    "Core Space converted storage points overflowed during conversion",
                )
            })?;

        positioned_changes.push(PositionedCoreSpaceChange::new(
            conversion.position,
            PendingCoreSpaceChange::StoragePointConversion {
                contract_address: conversion.contract_address,
                from_sponsor_pool_amount: conversion.from_sponsor_pool,
                from_storage_collateral_amount: conversion.from_storage_collateral,
            },
        ));
        Ok(())
    }

    fn apply_storage_collateral_release(
        &mut self,
        release: &StorageCollateralReleaseOperation,
    ) -> Result<(), CoreSpaceChangesError> {
        let collateral_location = CfxBalanceLocation::StorageCollateral {
            contract_address: release.contract_address,
        };
        let token_collateral = self
            .balances
            .get(&collateral_location)
            .copied()
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(format!(
                    "before Core Space token storage collateral is missing for contract {}",
                    release.contract_address
                ))
            })?;
        let refundable_amount = token_collateral.min(release.total_released_amount);
        let burnt_amount = release
            .total_released_amount
            .checked_sub(refundable_amount)
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(
                    "Core Space storage release refundable amount exceeded its total",
                )
            })?;

        let storage_point_refund = match self.storage_points.get_mut(&release.contract_address) {
            Some(Some(points)) => {
                let refund = points.used.min(refundable_amount);
                points.used = points.used.checked_sub(refund).ok_or_else(|| {
                    CoreSpaceChangesError::inconsistent_execution(format!(
                        "Core Space used storage points underflowed for contract {}",
                        release.contract_address
                    ))
                })?;
                points.unused = points.unused.checked_add(refund).ok_or_else(|| {
                    CoreSpaceChangesError::inconsistent_execution(format!(
                        "Core Space unused storage points overflowed for contract {}",
                        release.contract_address
                    ))
                })?;
                refund
            }
            Some(None) => U256::ZERO,
            None => {
                return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                    "before Core Space storage points are missing for contract {}",
                    release.contract_address
                )));
            }
        };

        let expected_non_point_amount = release
            .total_released_amount
            .checked_sub(storage_point_refund)
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(
                    "Core Space storage-point refund exceeded its total release",
                )
            })?;
        if release.observed_non_point_amount != expected_non_point_amount {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space storage release movement mismatch for contract {}: observed {}, expected {}",
                release.contract_address,
                release.observed_non_point_amount,
                expected_non_point_amount
            )));
        }

        let token_refund = refundable_amount
            .checked_sub(storage_point_refund)
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(
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
                CoreSpaceChangesError::inconsistent_execution(
                    "Core Space storage-point refund exceeded total storage release",
                )
            })?;
        let globals = self.storage_point_globals.as_mut().ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(
                "before Core Space storage-point globals are missing for a storage release",
            )
        })?;
        globals.total_storage = globals
            .total_storage
            .checked_sub(total_storage_debit)
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(
                    "Core Space total storage underflowed during a storage release",
                )
            })?;
        globals.used_storage_points = globals
            .used_storage_points
            .checked_sub(storage_point_refund)
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(
                    "Core Space used storage points underflowed during a storage release",
                )
            })?;

        Ok(())
    }

    fn sponsor_identity(
        &self,
        location: SponsorResourceLocation,
    ) -> Result<Option<Address>, CoreSpaceChangesError> {
        self.sponsor_identities
            .get(&location)
            .copied()
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(format!(
                    "before Core Space {:?} sponsor identity is missing for contract {}",
                    location.resource, location.contract_address
                ))
            })
    }

    fn set_sponsor_identity(
        &mut self,
        location: SponsorResourceLocation,
        sponsor: Option<Address>,
    ) -> Result<(), CoreSpaceChangesError> {
        let current = self.sponsor_identities.get_mut(&location).ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(format!(
                "before Core Space {:?} sponsor identity is missing for contract {}",
                location.resource, location.contract_address
            ))
        })?;
        *current = sponsor;
        Ok(())
    }

    fn gas_fee_upper_bound(
        &self,
        contract_address: Address,
    ) -> Result<U256, CoreSpaceChangesError> {
        self.gas_fee_upper_bounds
            .get(&contract_address)
            .copied()
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(format!(
                    "before Core Space gas sponsor bound is missing for contract {contract_address}"
                ))
            })
    }

    fn set_gas_fee_upper_bound(
        &mut self,
        contract_address: Address,
        gas_fee_upper_bound: U256,
    ) -> Result<(), CoreSpaceChangesError> {
        let current = self
            .gas_fee_upper_bounds
            .get_mut(&contract_address)
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(format!(
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
    ) -> Result<(), CoreSpaceChangesError> {
        let actual = self.balances.get(&location).ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(format!(
                "before Core Space CFX balance is missing for {location}"
            ))
        })?;
        if *actual != expected {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space sponsorship {operation} did not consume the complete {location}: balance {actual}, refund {expected}"
            )));
        }
        Ok(())
    }

    fn debit_balance(
        &mut self,
        location: CfxBalanceLocation,
        amount: U256,
    ) -> Result<(), CoreSpaceChangesError> {
        let balance = self.balances.get_mut(&location).ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(format!(
                "before Core simulation CFX balance is missing for {location}"
            ))
        })?;
        *balance = balance.checked_sub(amount).ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(format!(
                "Core simulation CFX balance underflow for {location}: balance {balance}, debit {amount}"
            ))
        })?;
        Ok(())
    }

    fn credit_balance(
        &mut self,
        location: CfxBalanceLocation,
        amount: U256,
    ) -> Result<(), CoreSpaceChangesError> {
        let balance = self.balances.get_mut(&location).ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(format!(
                "before Core simulation CFX balance is missing for {location}"
            ))
        })?;
        *balance = balance.checked_add(amount).ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(format!(
                "Core simulation CFX balance overflow for {location}: balance {balance}, credit {amount}"
            ))
        })?;
        Ok(())
    }

    fn debit_total_issued(
        &mut self,
        amount: U256,
        debit_reason: &str,
    ) -> Result<(), CoreSpaceChangesError> {
        self.total_issued = self.total_issued.checked_sub(amount).ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space total issued underflowed while replaying {debit_reason}"
            ))
        })?;
        Ok(())
    }

    fn verify_matches(&self, after_state: &Self) -> Result<(), CoreSpaceChangesError> {
        for (location, replayed_balance) in &self.balances {
            let after_balance = after_state.balances.get(location).ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(format!(
                    "after Core simulation CFX balance is missing for {location}"
                ))
            })?;
            if replayed_balance != after_balance {
                return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                    "Core simulation CFX balance mismatch for {location}: replayed {replayed_balance}, after {after_balance}"
                )));
            }
        }

        for (location, replayed_sponsor) in &self.sponsor_identities {
            let after_sponsor = after_state
                .sponsor_identities
                .get(location)
                .ok_or_else(|| {
                    CoreSpaceChangesError::inconsistent_execution(format!(
                        "after Core Space {:?} sponsor identity is missing for contract {}",
                        location.resource, location.contract_address
                    ))
                })?;
            if replayed_sponsor != after_sponsor {
                return Err(CoreSpaceChangesError::inconsistent_execution(format!(
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
                    CoreSpaceChangesError::inconsistent_execution(format!(
                        "after Core Space gas sponsor bound is missing for contract {contract_address}"
                    ))
                })?;
            if replayed_gas_fee_upper_bound != after_gas_fee_upper_bound {
                return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                    "Core Space gas sponsor bound mismatch for contract {contract_address}: replayed {replayed_gas_fee_upper_bound}, after {after_gas_fee_upper_bound}"
                )));
            }
        }

        for (rule_key, replayed_rule) in &self.sponsorship_access_rules {
            let after_rule = after_state
                .sponsorship_access_rules
                .get(rule_key)
                .ok_or_else(|| {
                    CoreSpaceChangesError::inconsistent_execution(format!(
                        "after Core Space sponsorship access rule is missing for contract {}",
                        rule_key.contract_address
                    ))
                })?;
            if replayed_rule != after_rule {
                return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                    "Core Space sponsorship access rule mismatch for contract {}: replayed {replayed_rule}, after {after_rule}",
                    rule_key.contract_address
                )));
            }
        }

        for (contract_address, replayed_admin) in &self.contract_admins {
            let after_admin = after_state
                .contract_admins
                .get(contract_address)
                .ok_or_else(|| {
                    CoreSpaceChangesError::inconsistent_execution(format!(
                        "after Core Space admin is missing for contract {contract_address}"
                    ))
                })?;
            if replayed_admin != after_admin {
                return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                    "Core Space admin mismatch for contract {contract_address}: replayed {replayed_admin}, after {after_admin}"
                )));
            }
        }

        for (contract_address, replayed_exists) in &self.contract_exists {
            let after_exists = after_state
                .contract_exists
                .get(contract_address)
                .ok_or_else(|| {
                    CoreSpaceChangesError::inconsistent_execution(format!(
                        "after Core Space contract existence is missing for {contract_address}"
                    ))
                })?;
            if replayed_exists != after_exists {
                return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                    "Core Space contract existence mismatch for {contract_address}: replayed {replayed_exists}, after {after_exists}"
                )));
            }
        }

        for (account, replayed_points) in &self.storage_points {
            let after_points = after_state.storage_points.get(account).ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(format!(
                    "after Core Space storage points are missing for contract {account}"
                ))
            })?;
            if replayed_points != after_points {
                return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                    "Core Space storage-point pocket mismatch for contract {account}: replayed {replayed_points:?}, after {after_points:?}"
                )));
            }
        }

        if self.total_issued != after_state.total_issued {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space total issued mismatch: replayed {}, after {}",
                self.total_issued, after_state.total_issued
            )));
        }
        if self.total_staking != after_state.total_staking {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space total staking mismatch: replayed {}, after {}",
                self.total_staking, after_state.total_staking
            )));
        }
        if self.total_espace_tokens != after_state.total_espace_tokens {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                "Core total eSpace tokens mismatch: replayed {:?}, after {:?}",
                self.total_espace_tokens, after_state.total_espace_tokens
            )));
        }
        if self.storage_point_globals != after_state.storage_point_globals {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
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
) -> Result<(), CoreSpaceChangesError> {
    if observed_location == expected_location {
        return Ok(());
    }

    Err(CoreSpaceChangesError::inconsistent_execution(format!(
        "Core Space gas {location_role} mismatch: observed {observed_location}, expected {expected_location}"
    )))
}
