use super::{Address, GasSponsor, ProtocolChange, StoragePoints, StorageSponsor};
use crate::{Error, address::VmAddress, primitive::*, state::read_account};
use alloy::primitives::{B256, U256};
use cfx_executor::state::State;
use cfx_parameters::internal_contract_addresses::{
    SPONSOR_WHITELIST_CONTROL_CONTRACT_ADDRESS, STORAGE_INTEREST_STAKING_CONTRACT_ADDRESS,
};
use cfx_types::{AddressSpaceUtil, Space};
use conflux_provider::Network;
use simulation_core::ExecutionTrace;

pub(crate) fn derive(
    before: &State,
    after: &State,
    trace: &ExecutionTrace<Address>,
    network: Network,
) -> Result<Vec<ProtocolChange>, Error> {
    let native =
        |address: cfx_types::Address| Address::from_vm(address.with_native_space(), network);
    let optional = |address: cfx_types::Address| (!address.is_zero()).then(|| native(address));
    let mut changes = Vec::new();
    for (identity, diff) in &trace.accounts {
        let address = identity.to_vm();
        if address.space != Space::Native {
            continue;
        }
        let old = read_account(before, &address)?;
        let new = read_account(after, &address)?;
        let old_staking = old
            .as_ref()
            .map_or(U256::ZERO, |a| u256_from_cfx(a.staking_balance));
        let new_staking = new
            .as_ref()
            .map_or(U256::ZERO, |a| u256_from_cfx(a.staking_balance));
        if old_staking != new_staking {
            changes.push(ProtocolChange::StakingBalance {
                account: *identity,
                before: old_staking,
                after: new_staking,
            });
        }
        let old_interest = old
            .as_ref()
            .map_or(U256::ZERO, |a| u256_from_cfx(a.accumulated_interest_return));
        let new_interest = new
            .as_ref()
            .map_or(U256::ZERO, |a| u256_from_cfx(a.accumulated_interest_return));
        if old_interest != new_interest {
            changes.push(ProtocolChange::AccumulatedInterestReturn {
                account: *identity,
                before: old_interest,
                after: new_interest,
            });
        }
        let old_collateral = old
            .as_ref()
            .map_or(U256::ZERO, |a| u256_from_cfx(a.collateral_for_storage));
        let new_collateral = new
            .as_ref()
            .map_or(U256::ZERO, |a| u256_from_cfx(a.collateral_for_storage));
        if old_collateral != new_collateral {
            changes.push(ProtocolChange::CollateralForStorage {
                account: *identity,
                before: old_collateral,
                after: new_collateral,
            });
        }
        // Staking operates on the actual caller. Frames locate the account only;
        // list values always come from the two state views.
        if old_staking != new_staking
            || trace.calls.iter().any(|call| {
                call.from == *identity
                    && call.to == native(STORAGE_INTEREST_STAKING_CONTRACT_ADDRESS)
            })
        {
            let old = before.deposit_list(&address.address)?.0;
            let new = after.deposit_list(&address.address)?.0;
            if old != new {
                changes.push(ProtocolChange::DepositList {
                    account: *identity,
                    before: old,
                    after: new,
                });
            }
            let old = before.vote_stake_list(&address.address)?.0;
            let new = after.vote_stake_list(&address.address)?.0;
            if old != new {
                changes.push(ProtocolChange::VoteStakeList {
                    account: *identity,
                    before: old,
                    after: new,
                });
            }
        }
        let old_admin = old.as_ref().and_then(|a| optional(a.admin));
        let new_admin = new.as_ref().and_then(|a| optional(a.admin));
        if old_admin != new_admin {
            changes.push(ProtocolChange::Admin {
                contract: *identity,
                before: old_admin,
                after: new_admin,
            });
        }
        let old_sponsor = old
            .as_ref()
            .map(|a| a.sponsor_info.clone())
            .unwrap_or_default();
        let new_sponsor = new
            .as_ref()
            .map(|a| a.sponsor_info.clone())
            .unwrap_or_default();
        let gas = |s: &primitives::SponsorInfo| GasSponsor {
            sponsor: optional(s.sponsor_for_gas),
            balance: u256_from_cfx(s.sponsor_balance_for_gas),
            gas_bound: u256_from_cfx(s.sponsor_gas_bound),
        };
        let old_gas = gas(&old_sponsor);
        let new_gas = gas(&new_sponsor);
        if old_gas != new_gas {
            changes.push(ProtocolChange::GasSponsor {
                contract: *identity,
                before: old_gas,
                after: new_gas,
            });
        }
        let storage = |s: &primitives::SponsorInfo| StorageSponsor {
            sponsor: optional(s.sponsor_for_collateral),
            balance: u256_from_cfx(s.sponsor_balance_for_collateral),
            storage_points: s.storage_points.as_ref().map(|points| StoragePoints {
                unused: u256_from_cfx(points.unused),
                used: u256_from_cfx(points.used),
            }),
        };
        let old_storage = storage(&old_sponsor);
        let new_storage = storage(&new_sponsor);
        if old_storage != new_storage {
            changes.push(ProtocolChange::StorageSponsor {
                contract: *identity,
                before: old_storage,
                after: new_storage,
            });
        }
        if address.address == SPONSOR_WHITELIST_CONTROL_CONTRACT_ADDRESS {
            for (key, value) in &diff.storage {
                let Some(crate::state::CoreSpaceInternalStateItem::SponsorWhitelist(key)) =
                    crate::state::parse_core_space_internal_storage(address.address, key)
                else {
                    return Err(Error::Unsupported(
                        "unrecognized sponsor whitelist storage key".into(),
                    ));
                };
                changes.push(ProtocolChange::SponsorWhitelist {
                    contract: native(key.contract_address),
                    user: optional(key.account_address),
                    before: value.before != B256::ZERO,
                    after: value.after != B256::ZERO,
                });
            }
        }
    }
    Ok(changes)
}
