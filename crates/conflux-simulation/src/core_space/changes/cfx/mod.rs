mod collection;
mod cross_space;
mod sponsorship;
mod verification;

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use alloy_primitives::{Address, U256};
use contract_standards::Position;
use primitives::{Action, SignedTransaction};

pub(crate) use collection::collect_cfx_operations;
pub(crate) use verification::{read_cfx_state_values, verify_cfx_changes};

use crate::{
    ConfluxSimulationError,
    core_space::changes::{CrossSpaceAddress, SponsoredResource, SponsorshipEligibilityTarget},
    primitive::{address_from_cfx, address_to_cfx},
    state::{MaskedSponsorWhitelistEntries, SponsorWhitelistStorageKey},
};

#[derive(Debug)]
pub(crate) struct CfxOperations {
    balance_locations: Vec<CfxBalanceLocation>,
    sponsor_resources: Vec<SponsorResourceLocation>,
    contracts_requiring_gas_fee_upper_bound: Vec<Address>,
    sponsorship_access_rule_keys: Vec<SponsorshipAccessRuleKey>,
    admin_managed_sponsorship_contracts: Vec<Address>,
    storage_point_accounts: Vec<Address>,
    requires_storage_point_globals: bool,
    requires_total_espace_tokens: bool,
    operations: Vec<CfxOperation>,
}

impl CfxOperations {
    fn from_operations(operations: Vec<CfxOperation>) -> Self {
        let mut balance_locations = BTreeSet::new();
        let mut sponsor_resources = BTreeSet::new();
        let mut sponsorship_access_rule_keys = BTreeSet::new();
        let mut admin_managed_sponsorship_contracts = BTreeSet::new();
        let mut storage_point_accounts = BTreeSet::new();
        let mut requires_storage_point_globals = false;
        let mut requires_total_espace_tokens = false;

        for operation in &operations {
            match operation {
                CfxOperation::CoreSpaceBalanceTransfer { from, to, .. } => {
                    balance_locations
                        .insert(CfxBalanceLocation::CoreSpaceAccount { account: *from });
                    balance_locations.insert(CfxBalanceLocation::CoreSpaceAccount { account: *to });
                }
                CfxOperation::EspaceBalanceTransfer { from, to, .. } => {
                    balance_locations.insert(CfxBalanceLocation::EspaceAccount { account: *from });
                    balance_locations.insert(CfxBalanceLocation::EspaceAccount { account: *to });
                }
                CfxOperation::CrossSpaceTransfer(transfer) => {
                    balance_locations.insert(cross_space_balance_location(transfer.from));
                    balance_locations.insert(cross_space_balance_location(transfer.to));
                    requires_total_espace_tokens = true;
                }
                CfxOperation::GasPrecharge { payer, .. } => {
                    balance_locations.insert(*payer);
                }
                CfxOperation::GasRefund { recipient, .. } => {
                    balance_locations.insert(*recipient);
                }
                CfxOperation::StakingDeposit { account, .. }
                | CfxOperation::StakingWithdrawal { account, .. } => {
                    balance_locations
                        .insert(CfxBalanceLocation::CoreSpaceAccount { account: *account });
                    balance_locations.insert(CfxBalanceLocation::Staking { account: *account });
                }
                CfxOperation::NativeBurn { account, .. } => {
                    balance_locations
                        .insert(CfxBalanceLocation::CoreSpaceAccount { account: *account });
                }
                CfxOperation::StakingBurn { account, .. } => {
                    balance_locations.insert(CfxBalanceLocation::Staking { account: *account });
                }
                CfxOperation::SponsorshipFunding(funding) => {
                    balance_locations.insert(CfxBalanceLocation::CoreSpaceAccount {
                        account: funding.sponsor,
                    });
                    let sponsored_resource = funding.funding_terms.sponsored_resource();
                    balance_locations
                        .insert(sponsored_resource.pool_location(funding.contract_address));
                    sponsor_resources.insert(SponsorResourceLocation {
                        resource: sponsored_resource,
                        contract_address: funding.contract_address,
                    });
                    if let Some(refund) = funding.refund {
                        balance_locations.insert(CfxBalanceLocation::CoreSpaceAccount {
                            account: refund.sponsor,
                        });
                    }
                    if sponsored_resource == SponsoredResource::StorageCollateral {
                        add_storage_point_requirements(
                            funding.contract_address,
                            &mut balance_locations,
                            &mut storage_point_accounts,
                            &mut requires_storage_point_globals,
                        );
                    }
                }
                CfxOperation::SponsorshipStandaloneRefund(refund) => {
                    balance_locations.insert(CfxBalanceLocation::CoreSpaceAccount {
                        account: refund.sponsor,
                    });
                    balance_locations
                        .insert(refund.resource.pool_location(refund.contract_address));
                    sponsor_resources.insert(SponsorResourceLocation {
                        resource: refund.resource,
                        contract_address: refund.contract_address,
                    });
                    if refund.resource == SponsoredResource::StorageCollateral {
                        add_storage_point_requirements(
                            refund.contract_address,
                            &mut balance_locations,
                            &mut storage_point_accounts,
                            &mut requires_storage_point_globals,
                        );
                    }
                }
                CfxOperation::StoragePointConversion(conversion) => {
                    add_storage_point_requirements(
                        conversion.contract_address,
                        &mut balance_locations,
                        &mut storage_point_accounts,
                        &mut requires_storage_point_globals,
                    );
                    sponsor_resources.insert(SponsorResourceLocation {
                        resource: SponsoredResource::StorageCollateral,
                        contract_address: conversion.contract_address,
                    });
                }
                CfxOperation::StorageCollateralRelease(release) => {
                    add_storage_point_requirements(
                        release.contract_address,
                        &mut balance_locations,
                        &mut storage_point_accounts,
                        &mut requires_storage_point_globals,
                    );
                }
                CfxOperation::SponsorshipAccessRule(update) => {
                    sponsorship_access_rule_keys.insert(update.key());
                    if update.caller_role == SponsorshipAccessCallerRole::ContractAdmin {
                        admin_managed_sponsorship_contracts.insert(update.contract_address);
                    }
                }
            }
        }

        let contracts_requiring_gas_fee_upper_bound = sponsor_resources
            .iter()
            .filter_map(|location| {
                (location.resource == SponsoredResource::Gas).then_some(location.contract_address)
            })
            .collect();

        Self {
            balance_locations: balance_locations.into_iter().collect(),
            sponsor_resources: sponsor_resources.into_iter().collect(),
            contracts_requiring_gas_fee_upper_bound,
            sponsorship_access_rule_keys: sponsorship_access_rule_keys.into_iter().collect(),
            admin_managed_sponsorship_contracts: admin_managed_sponsorship_contracts
                .into_iter()
                .collect(),
            storage_point_accounts: storage_point_accounts.into_iter().collect(),
            requires_storage_point_globals,
            requires_total_espace_tokens,
            operations,
        }
    }

    /// Applies already-collected CFX operations that affect staking balances.
    pub(crate) fn apply_staking_balance_effects(
        &self,
        staking_balances: &mut BTreeMap<Address, U256>,
    ) -> Result<(), ConfluxSimulationError> {
        for operation in &self.operations {
            match operation {
                CfxOperation::StakingDeposit {
                    account, amount, ..
                } => credit_staking_balance_if_present(staking_balances, *account, *amount)?,
                CfxOperation::StakingWithdrawal {
                    account,
                    principal_amount,
                    ..
                } => debit_staking_balance_if_present(
                    staking_balances,
                    *account,
                    *principal_amount,
                    "withdrawal",
                )?,
                CfxOperation::StakingBurn {
                    account, amount, ..
                } => debit_staking_balance_if_present(staking_balances, *account, *amount, "burn")?,
                _ => {}
            }
        }

        Ok(())
    }

    pub(crate) fn reject_masked_sponsorship_access_dependencies(
        &self,
        masked_entries: &MaskedSponsorWhitelistEntries,
    ) -> Result<(), ConfluxSimulationError> {
        let masked_entries =
            masked_entries
                .snapshot()
                .map_err(|error| ConfluxSimulationError::StateAccess {
                    message: error.to_string(),
                })?;
        for key in &self.sponsorship_access_rule_keys {
            let SponsorshipEligibilityTarget::Account(account_address) = key.account_scope else {
                continue;
            };
            let storage_key = SponsorWhitelistStorageKey {
                contract_address: address_to_cfx(key.contract_address),
                account_address: address_to_cfx(account_address),
            };
            if masked_entries.contains(&storage_key) {
                return Err(ConfluxSimulationError::analysis_failed(format!(
                    "Core Space sponsorship access depends on a raw whitelist entry masked by the all-accounts rule for contract {} and account {account_address}",
                    key.contract_address
                )));
            }
        }
        Ok(())
    }
}

fn add_storage_point_requirements(
    contract_address: Address,
    balance_locations: &mut BTreeSet<CfxBalanceLocation>,
    storage_point_accounts: &mut BTreeSet<Address>,
    requires_storage_point_globals: &mut bool,
) {
    balance_locations.insert(CfxBalanceLocation::StorageSponsor { contract_address });
    balance_locations.insert(CfxBalanceLocation::StorageCollateral { contract_address });
    storage_point_accounts.insert(contract_address);
    *requires_storage_point_globals = true;
}

fn credit_staking_balance_if_present(
    staking_balances: &mut BTreeMap<Address, U256>,
    account: Address,
    amount: U256,
) -> Result<(), ConfluxSimulationError> {
    let Some(balance) = staking_balances.get_mut(&account) else {
        return Ok(());
    };
    *balance = balance.checked_add(amount).ok_or_else(|| {
        ConfluxSimulationError::analysis_failed(format!(
            "Core Space staking balance overflowed while replaying a deposit for {account}"
        ))
    })?;
    Ok(())
}

fn debit_staking_balance_if_present(
    staking_balances: &mut BTreeMap<Address, U256>,
    account: Address,
    amount: U256,
    operation: &str,
) -> Result<(), ConfluxSimulationError> {
    let Some(balance) = staking_balances.get_mut(&account) else {
        return Ok(());
    };
    *balance = balance.checked_sub(amount).ok_or_else(|| {
        ConfluxSimulationError::analysis_failed(format!(
            "Core Space staking balance underflowed while replaying a {operation} for {account}"
        ))
    })?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CfxBalanceLocation {
    CoreSpaceAccount { account: Address },
    EspaceAccount { account: Address },
    Staking { account: Address },
    GasSponsor { contract_address: Address },
    StorageSponsor { contract_address: Address },
    StorageCollateral { contract_address: Address },
}

impl fmt::Display for CfxBalanceLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoreSpaceAccount { account } => {
                write!(formatter, "Core Space balance for account {account}")
            }
            Self::EspaceAccount { account } => {
                write!(formatter, "eSpace balance for account {account}")
            }
            Self::Staking { account } => {
                write!(formatter, "staking balance for account {account}")
            }
            Self::GasSponsor { contract_address } => {
                write!(
                    formatter,
                    "gas sponsor balance for contract {contract_address}"
                )
            }
            Self::StorageSponsor { contract_address } => {
                write!(
                    formatter,
                    "storage sponsor balance for contract {contract_address}"
                )
            }
            Self::StorageCollateral { contract_address } => {
                write!(
                    formatter,
                    "token storage collateral for contract {contract_address}"
                )
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SponsorResourceLocation {
    resource: SponsoredResource,
    contract_address: Address,
}

impl SponsoredResource {
    const fn pool_location(self, contract_address: Address) -> CfxBalanceLocation {
        match self {
            Self::Gas => CfxBalanceLocation::GasSponsor { contract_address },
            Self::StorageCollateral => CfxBalanceLocation::StorageSponsor { contract_address },
        }
    }
}

#[derive(Debug)]
enum CfxOperation {
    CoreSpaceBalanceTransfer {
        position: Position,
        from: Address,
        to: Address,
        amount: U256,
    },
    EspaceBalanceTransfer {
        from: Address,
        to: Address,
        amount: U256,
    },
    CrossSpaceTransfer(CrossSpaceTransferOperation),
    GasPrecharge {
        payer: CfxBalanceLocation,
        amount: U256,
    },
    GasRefund {
        recipient: CfxBalanceLocation,
        amount: U256,
    },
    StakingDeposit {
        position: Position,
        account: Address,
        amount: U256,
    },
    StakingWithdrawal {
        position: Position,
        account: Address,
        principal_amount: U256,
        reward_amount: U256,
    },
    NativeBurn {
        position: Position,
        account: Address,
        amount: U256,
    },
    StakingBurn {
        position: Position,
        account: Address,
        amount: U256,
    },
    SponsorshipFunding(SponsorshipFundingOperation),
    SponsorshipStandaloneRefund(SponsorshipRefundOperation),
    SponsorshipAccessRule(SponsorshipAccessRuleUpdate),
    StoragePointConversion(StoragePointConversionOperation),
    StorageCollateralRelease(StorageCollateralReleaseOperation),
}

#[derive(Debug, Clone, Copy)]
struct CrossSpaceTransferOperation {
    position: Position,
    from: CrossSpaceAddress,
    to: CrossSpaceAddress,
    amount: U256,
}

const fn cross_space_balance_location(address: CrossSpaceAddress) -> CfxBalanceLocation {
    match address {
        CrossSpaceAddress::CoreSpace(account) => CfxBalanceLocation::CoreSpaceAccount { account },
        CrossSpaceAddress::Espace(account) => CfxBalanceLocation::EspaceAccount { account },
    }
}

#[derive(Debug)]
struct SponsorshipFundingOperation {
    position: Position,
    funding_terms: SponsorshipFundingTerms,
    sponsor: Address,
    contract_address: Address,
    gross_deposit_amount: U256,
    pool_deposit_amount: U256,
    refund: Option<SponsorshipRefundOperation>,
}

#[derive(Debug, Clone, Copy)]
enum SponsorshipFundingTerms {
    Gas { gas_fee_upper_bound: U256 },
    StorageCollateral,
}

impl SponsorshipFundingTerms {
    const fn sponsored_resource(self) -> SponsoredResource {
        match self {
            Self::Gas { .. } => SponsoredResource::Gas,
            Self::StorageCollateral => SponsoredResource::StorageCollateral,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SponsorshipRefundOperation {
    position: Position,
    resource: SponsoredResource,
    sponsor: Address,
    contract_address: Address,
    gross_refund_amount: U256,
    pool_refund_amount: U256,
}

#[derive(Debug)]
struct StoragePointConversionOperation {
    position: Position,
    contract_address: Address,
    from_sponsor_pool: U256,
    from_storage_collateral: U256,
}

#[derive(Debug)]
struct StorageCollateralReleaseOperation {
    contract_address: Address,
    total_released_amount: U256,
    observed_non_point_amount: U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SponsorshipAccessCallerRole {
    SponsoredContract,
    ContractAdmin,
}

#[derive(Debug)]
struct SponsorshipAccessRuleUpdate {
    position: Position,
    caller_role: SponsorshipAccessCallerRole,
    caller_address: Address,
    contract_address: Address,
    account_scope: SponsorshipEligibilityTarget,
    enabled_after: bool,
}

impl SponsorshipAccessRuleUpdate {
    const fn key(&self) -> SponsorshipAccessRuleKey {
        SponsorshipAccessRuleKey {
            contract_address: self.contract_address,
            account_scope: self.account_scope,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SponsorshipAccessRuleKey {
    contract_address: Address,
    account_scope: SponsorshipEligibilityTarget,
}

pub(crate) fn determine_gas_fee_payer(
    transaction: &SignedTransaction,
    gas_paid_by_sponsor: bool,
) -> Result<CfxBalanceLocation, ConfluxSimulationError> {
    if !gas_paid_by_sponsor {
        return Ok(CfxBalanceLocation::CoreSpaceAccount {
            account: address_from_cfx(transaction.sender().address),
        });
    }

    match transaction.action() {
        Action::Call(contract_address) => Ok(CfxBalanceLocation::GasSponsor {
            contract_address: address_from_cfx(contract_address),
        }),
        Action::Create => Err(ConfluxSimulationError::analysis_failed(
            "Core Space contract creation unexpectedly reported sponsored gas",
        )),
    }
}
