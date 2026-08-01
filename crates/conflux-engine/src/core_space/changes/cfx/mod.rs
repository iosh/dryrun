mod collection;
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
    ConfluxEngineError, core_space::changes::SponsoredResource, primitive::address_from_cfx,
};

#[derive(Debug)]
pub(crate) struct CfxOperations {
    balance_locations: Vec<CfxBalanceLocation>,
    sponsor_resources: Vec<SponsorResourceLocation>,
    storage_point_accounts: Vec<Address>,
    requires_storage_point_globals: bool,
    operations: Vec<CfxOperation>,
}

impl CfxOperations {
    fn from_operations(operations: Vec<CfxOperation>) -> Self {
        let mut balance_locations = BTreeSet::new();
        let mut sponsor_resources = BTreeSet::new();
        let mut storage_point_accounts = BTreeSet::new();
        let mut requires_storage_point_globals = false;

        for operation in &operations {
            match operation {
                CfxOperation::AccountTransfer { from, to, .. } => {
                    balance_locations.insert(CfxBalanceLocation::Account { account: *from });
                    balance_locations.insert(CfxBalanceLocation::Account { account: *to });
                }
                CfxOperation::GasPrecharge { payer, .. } => {
                    balance_locations.insert(*payer);
                }
                CfxOperation::GasRefund { recipient, .. } => {
                    balance_locations.insert(*recipient);
                }
                CfxOperation::StakingDeposit { account, .. }
                | CfxOperation::StakingWithdrawal { account, .. } => {
                    balance_locations.insert(CfxBalanceLocation::Account { account: *account });
                    balance_locations.insert(CfxBalanceLocation::Staking { account: *account });
                }
                CfxOperation::NativeBurn { account, .. } => {
                    balance_locations.insert(CfxBalanceLocation::Account { account: *account });
                }
                CfxOperation::StakingBurn { account, .. } => {
                    balance_locations.insert(CfxBalanceLocation::Staking { account: *account });
                }
                CfxOperation::SponsorshipCall(call) => {
                    balance_locations.insert(CfxBalanceLocation::Account {
                        account: call.sponsor,
                    });
                    balance_locations.insert(call.resource.pool_location(call.contract_address));
                    sponsor_resources.insert(SponsorResourceLocation {
                        resource: call.resource,
                        contract_address: call.contract_address,
                    });
                    if let Some(refund) = call.refund {
                        balance_locations.insert(CfxBalanceLocation::Account {
                            account: refund.sponsor,
                        });
                    }
                    if call.resource == SponsoredResource::StorageCollateral {
                        add_storage_point_requirements(
                            call.contract_address,
                            &mut balance_locations,
                            &mut storage_point_accounts,
                            &mut requires_storage_point_globals,
                        );
                    }
                }
                CfxOperation::SponsorshipStandaloneRefund(refund) => {
                    balance_locations.insert(CfxBalanceLocation::Account {
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
            }
        }

        Self {
            balance_locations: balance_locations.into_iter().collect(),
            sponsor_resources: sponsor_resources.into_iter().collect(),
            storage_point_accounts: storage_point_accounts.into_iter().collect(),
            requires_storage_point_globals,
            operations,
        }
    }

    /// Applies already-collected CFX operations that affect staking balances.
    pub(crate) fn apply_staking_balance_effects(
        &self,
        staking_balances: &mut BTreeMap<Address, U256>,
    ) -> Result<(), ConfluxEngineError> {
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
) -> Result<(), ConfluxEngineError> {
    let Some(balance) = staking_balances.get_mut(&account) else {
        return Ok(());
    };
    *balance = balance.checked_add(amount).ok_or_else(|| {
        ConfluxEngineError::analysis_failed(format!(
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
) -> Result<(), ConfluxEngineError> {
    let Some(balance) = staking_balances.get_mut(&account) else {
        return Ok(());
    };
    *balance = balance.checked_sub(amount).ok_or_else(|| {
        ConfluxEngineError::analysis_failed(format!(
            "Core Space staking balance underflowed while replaying a {operation} for {account}"
        ))
    })?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CfxBalanceLocation {
    Account { account: Address },
    Staking { account: Address },
    GasSponsor { contract_address: Address },
    StorageSponsor { contract_address: Address },
    StorageCollateral { contract_address: Address },
}

impl fmt::Display for CfxBalanceLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Account { account } => {
                write!(formatter, "balance for account {account}")
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
    AccountTransfer {
        position: Position,
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
    SponsorshipCall(SponsorshipCallOperation),
    SponsorshipStandaloneRefund(SponsorshipRefundOperation),
    StoragePointConversion(StoragePointConversionOperation),
}

#[derive(Debug)]
struct SponsorshipCallOperation {
    position: Position,
    resource: SponsoredResource,
    sponsor: Address,
    contract_address: Address,
    gross_deposit_amount: U256,
    pool_deposit_amount: U256,
    refund: Option<SponsorshipRefundOperation>,
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

pub(crate) fn determine_gas_fee_payer(
    transaction: &SignedTransaction,
    gas_paid_by_sponsor: bool,
) -> Result<CfxBalanceLocation, ConfluxEngineError> {
    if !gas_paid_by_sponsor {
        return Ok(CfxBalanceLocation::Account {
            account: address_from_cfx(transaction.sender().address),
        });
    }

    match transaction.action() {
        Action::Call(contract_address) => Ok(CfxBalanceLocation::GasSponsor {
            contract_address: address_from_cfx(contract_address),
        }),
        Action::Create => Err(ConfluxEngineError::analysis_failed(
            "Core Space contract creation unexpectedly reported sponsored gas",
        )),
    }
}
