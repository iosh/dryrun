mod analysis;
mod basic;
mod collection;
mod cross_space;
mod sponsorship;
mod verification;

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use crate::core_space::changes::ChangePosition;
use alloy_primitives::{Address, U256};
use primitives::{Action, SignedTransaction};

pub(crate) use analysis::CfxAnalysisInput;
pub(crate) use collection::collect_cfx_operations;
pub(crate) use verification::{CfxStateValues, read_cfx_state_values, verify_cfx_changes};

use crate::{
    core_space::CoreSpaceChangesError,
    core_space::changes::PendingSponsorshipAccessRuleScope,
    primitive::{address_from_cfx, address_to_cfx},
    state::{MaskedWhitelistKeys, SponsorWhitelistStorageKey},
};

#[derive(Debug)]
pub(crate) struct CfxOperations {
    balance_locations: Vec<CfxBalanceLocation>,
    sponsor_resources: Vec<SponsorResourceLocation>,
    contracts_requiring_gas_fee_upper_bound: Vec<Address>,
    sponsorship_access_rule_keys: Vec<SponsorshipAccessRuleKey>,
    contract_admins: Vec<Address>,
    storage_point_accounts: Vec<Address>,
    requires_storage_point_globals: bool,
    requires_total_espace_tokens: bool,
    operations: Vec<CfxOperation>,
}

impl CfxOperations {
    fn from_operations(
        operations: Vec<CfxOperation>,
        staking_accounts: impl IntoIterator<Item = Address>,
    ) -> Self {
        let mut balance_locations = BTreeSet::new();
        let mut sponsor_resources = BTreeSet::new();
        let mut sponsorship_access_rule_keys = BTreeSet::new();
        let mut contract_admins = BTreeSet::new();
        let mut storage_point_accounts = BTreeSet::new();
        let mut requires_storage_point_globals = false;
        let mut requires_total_espace_tokens = false;

        balance_locations.extend(
            staking_accounts
                .into_iter()
                .map(|account| CfxBalanceLocation::Staking { account }),
        );

        for operation in &operations {
            match operation {
                CfxOperation::Basic(BasicCfxOperation::CoreSpaceBalanceTransfer {
                    from,
                    to,
                    ..
                }) => {
                    balance_locations
                        .insert(CfxBalanceLocation::CoreSpaceAccount { account: *from });
                    balance_locations.insert(CfxBalanceLocation::CoreSpaceAccount { account: *to });
                }
                CfxOperation::Basic(BasicCfxOperation::EspaceBalanceTransfer {
                    from, to, ..
                }) => {
                    balance_locations.insert(CfxBalanceLocation::EspaceAccount { account: *from });
                    balance_locations.insert(CfxBalanceLocation::EspaceAccount { account: *to });
                }
                CfxOperation::CrossSpace(transfer) => {
                    match transfer {
                        CrossSpaceTransferOperation::ToEspace {
                            core_sender,
                            mapped_sender,
                            receiver,
                            ..
                        } => {
                            balance_locations.insert(CfxBalanceLocation::CoreSpaceAccount {
                                account: *core_sender,
                            });
                            balance_locations.insert(CfxBalanceLocation::EspaceAccount {
                                account: *mapped_sender,
                            });
                            balance_locations
                                .insert(CfxBalanceLocation::EspaceAccount { account: *receiver });
                        }
                        CrossSpaceTransferOperation::ToCoreSpace {
                            mapped_sender,
                            core_receiver,
                            ..
                        } => {
                            balance_locations.insert(CfxBalanceLocation::EspaceAccount {
                                account: *mapped_sender,
                            });
                            balance_locations.insert(CfxBalanceLocation::CoreSpaceAccount {
                                account: *core_receiver,
                            });
                        }
                    }
                    requires_total_espace_tokens = true;
                }
                CfxOperation::Basic(BasicCfxOperation::GasPrecharge { payer, .. }) => {
                    balance_locations.insert(*payer);
                }
                CfxOperation::Basic(BasicCfxOperation::GasRefund { recipient, .. }) => {
                    balance_locations.insert(*recipient);
                }
                CfxOperation::Basic(BasicCfxOperation::StakingDeposit { account, .. })
                | CfxOperation::Basic(BasicCfxOperation::StakingWithdrawal { account, .. }) => {
                    balance_locations
                        .insert(CfxBalanceLocation::CoreSpaceAccount { account: *account });
                    balance_locations.insert(CfxBalanceLocation::Staking { account: *account });
                }
                CfxOperation::Basic(BasicCfxOperation::NativeBurn { account, .. }) => {
                    balance_locations
                        .insert(CfxBalanceLocation::CoreSpaceAccount { account: *account });
                }
                CfxOperation::Sponsorship(SponsorshipOperation::Funding(funding)) => {
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
                CfxOperation::Sponsorship(SponsorshipOperation::StandaloneRefund(refund)) => {
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
                CfxOperation::Sponsorship(SponsorshipOperation::StoragePointConversion(
                    conversion,
                )) => {
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
                CfxOperation::Basic(BasicCfxOperation::StorageCollateralRelease(release)) => {
                    add_storage_point_requirements(
                        release.contract_address,
                        &mut balance_locations,
                        &mut storage_point_accounts,
                        &mut requires_storage_point_globals,
                    );
                }
                CfxOperation::Sponsorship(SponsorshipOperation::AccessRule(update)) => {
                    sponsorship_access_rule_keys.insert(update.key());
                    if update.caller_role == SponsorshipAccessCallerRole::ContractAdmin {
                        contract_admins.insert(update.contract_address);
                    }
                }
                CfxOperation::Admin(AdminOperation::Set(update)) => {
                    contract_admins.insert(update.contract_address);
                }
                CfxOperation::Admin(AdminOperation::Initialize { .. }) => {}
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
            contract_admins: contract_admins.into_iter().collect(),
            storage_point_accounts: storage_point_accounts.into_iter().collect(),
            requires_storage_point_globals,
            requires_total_espace_tokens,
            operations,
        }
    }

    pub(crate) fn staking_balance_effects(&self) -> StakingBalanceEffects {
        let effects = self
            .operations
            .iter()
            .filter_map(|operation| match operation {
                CfxOperation::Basic(BasicCfxOperation::StakingDeposit {
                    account, amount, ..
                }) => Some(StakingBalanceEffect::Deposit {
                    account: *account,
                    amount: *amount,
                }),
                CfxOperation::Basic(BasicCfxOperation::StakingWithdrawal {
                    account,
                    principal_amount,
                    ..
                }) => Some(StakingBalanceEffect::Withdrawal {
                    account: *account,
                    amount: *principal_amount,
                }),
                _ => None,
            })
            .collect();
        StakingBalanceEffects { effects }
    }
    pub(crate) fn reject_masked_sponsorship_access_dependencies(
        &self,
        masked_entries: &MaskedWhitelistKeys,
    ) -> Result<(), CoreSpaceChangesError> {
        let masked_entries = masked_entries.snapshot().map_err(|error| {
            CoreSpaceChangesError::recorded_state_access(
                "snapshot request-local sponsor whitelist reads",
                error,
            )
        })?;
        for key in &self.sponsorship_access_rule_keys {
            let PendingSponsorshipAccessRuleScope::Account(account_address) = key.account_scope
            else {
                continue;
            };
            let storage_key = SponsorWhitelistStorageKey {
                contract_address: address_to_cfx(key.contract_address),
                account_address: address_to_cfx(account_address),
            };
            if masked_entries.contains(&storage_key) {
                return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                    "Core Space sponsorship access depends on a raw whitelist entry masked by the all-accounts rule for contract {} and account {account_address}",
                    key.contract_address
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct StakingBalanceEffects {
    effects: Vec<StakingBalanceEffect>,
}

#[derive(Debug, Clone, Copy)]
enum StakingBalanceEffect {
    Deposit { account: Address, amount: U256 },
    Withdrawal { account: Address, amount: U256 },
}

impl StakingBalanceEffects {
    /// Applies effects already collected from the single execution trace.
    pub(crate) fn apply_to(
        &self,
        staking_balances: &mut BTreeMap<Address, U256>,
    ) -> Result<(), CoreSpaceChangesError> {
        for effect in &self.effects {
            match effect {
                StakingBalanceEffect::Deposit { account, amount } => {
                    credit_staking_balance_if_present(staking_balances, *account, *amount)?
                }
                StakingBalanceEffect::Withdrawal { account, amount } => {
                    debit_staking_balance_if_present(
                        staking_balances,
                        *account,
                        *amount,
                        "withdrawal",
                    )?
                }
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
) -> Result<(), CoreSpaceChangesError> {
    let Some(balance) = staking_balances.get_mut(&account) else {
        return Ok(());
    };
    *balance = balance.checked_add(amount).ok_or_else(|| {
        CoreSpaceChangesError::inconsistent_execution(format!(
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
) -> Result<(), CoreSpaceChangesError> {
    let Some(balance) = staking_balances.get_mut(&account) else {
        return Ok(());
    };
    *balance = balance.checked_sub(amount).ok_or_else(|| {
        CoreSpaceChangesError::inconsistent_execution(format!(
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SponsoredResource {
    Gas,
    StorageCollateral,
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
    Basic(BasicCfxOperation),
    CrossSpace(CrossSpaceTransferOperation),
    Admin(AdminOperation),
    Sponsorship(SponsorshipOperation),
}

#[derive(Debug)]
enum AdminOperation {
    Initialize {
        contract_address: Address,
        admin: Address,
    },
    Set(ContractAdminSetOperation),
}

#[derive(Debug)]
struct ContractAdminSetOperation {
    position: ChangePosition,
    caller: Address,
    contract_address: Address,
    new_admin: Address,
    is_creation_frame: bool,
}

#[derive(Debug)]
enum BasicCfxOperation {
    CoreSpaceBalanceTransfer {
        position: ChangePosition,
        from: Address,
        to: Address,
        amount: U256,
    },
    EspaceBalanceTransfer {
        from: Address,
        to: Address,
        amount: U256,
    },
    GasPrecharge {
        payer: CfxBalanceLocation,
        amount: U256,
    },
    GasRefund {
        recipient: CfxBalanceLocation,
        amount: U256,
    },
    StakingDeposit {
        account: Address,
        amount: U256,
    },
    StakingWithdrawal {
        account: Address,
        principal_amount: U256,
        reward_amount: U256,
    },
    NativeBurn {
        position: ChangePosition,
        account: Address,
        amount: U256,
    },
    StorageCollateralRelease(StorageCollateralReleaseOperation),
}

#[derive(Debug)]
enum SponsorshipOperation {
    Funding(SponsorshipFundingOperation),
    StandaloneRefund(SponsorshipRefundOperation),
    AccessRule(SponsorshipAccessRuleUpdate),
    StoragePointConversion(StoragePointConversionOperation),
}

#[derive(Debug, Clone, Copy)]
enum CrossSpaceTransferOperation {
    ToEspace {
        position: ChangePosition,
        core_sender: Address,
        mapped_sender: Address,
        receiver: Address,
        amount: U256,
    },
    ToCoreSpace {
        position: ChangePosition,
        mapped_sender: Address,
        core_receiver: Address,
        amount: U256,
    },
}

#[derive(Debug)]
struct SponsorshipFundingOperation {
    position: ChangePosition,
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
    resource: SponsoredResource,
    sponsor: Address,
    contract_address: Address,
    gross_refund_amount: U256,
    pool_refund_amount: U256,
}

#[derive(Debug)]
struct StoragePointConversionOperation {
    position: ChangePosition,
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
    position: ChangePosition,
    caller_role: SponsorshipAccessCallerRole,
    caller_address: Address,
    contract_address: Address,
    account_scope: PendingSponsorshipAccessRuleScope,
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
    account_scope: PendingSponsorshipAccessRuleScope,
}

pub(crate) fn determine_gas_fee_payer(
    transaction: &SignedTransaction,
    gas_paid_by_sponsor: bool,
) -> Result<CfxBalanceLocation, CoreSpaceChangesError> {
    if !gas_paid_by_sponsor {
        return Ok(CfxBalanceLocation::CoreSpaceAccount {
            account: address_from_cfx(transaction.sender().address),
        });
    }

    match transaction.action() {
        Action::Call(contract_address) => Ok(CfxBalanceLocation::GasSponsor {
            contract_address: address_from_cfx(contract_address),
        }),
        Action::Create => Err(CoreSpaceChangesError::inconsistent_execution(
            "Core Space contract creation unexpectedly reported sponsored gas",
        )),
    }
}
