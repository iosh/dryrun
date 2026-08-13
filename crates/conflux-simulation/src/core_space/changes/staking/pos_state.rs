use std::collections::{BTreeMap, BTreeSet};

use alloy_primitives::{Address, B256, U256};
use cfx_executor::{
    internal_contract::pos_internal_entries::{address_entry, identifier_entry, index_entry},
    state::State,
};
use cfx_types::{Address as CfxAddress, AddressSpaceUtil, BigEndianHash, H256};

use super::CommittedPoSCall;
use crate::{
    core_space::CoreSpaceChangesError,
    core_space::changes::StatePhase,
    primitive::{address_from_cfx, address_to_cfx, b256_from_cfx, b256_to_cfx, u256_from_cfx},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PoSStatus {
    registered: u64,
    unlocked: u64,
}

impl PoSStatus {
    pub(super) const fn is_initialized(self) -> bool {
        self.registered != 0
    }

    pub(super) const fn is_fully_unlocked(self) -> bool {
        self.registered == self.unlocked
    }

    pub(super) const fn has_locked_votes(self) -> bool {
        self.registered > self.unlocked
    }

    pub(super) const fn locked_vote_count(self) -> u64 {
        self.registered - self.unlocked
    }

    pub(super) fn checked_add_registered_votes(&mut self, vote_count: u64) -> Option<()> {
        self.registered = self.registered.checked_add(vote_count)?;
        Some(())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PoSStateValues {
    pub(super) identifiers_by_account: BTreeMap<Address, B256>,
    pub(super) accounts_by_identifier: BTreeMap<B256, Address>,
    pub(super) statuses_by_identifier: BTreeMap<B256, PoSStatus>,
    pub(super) staking_balances_by_account: BTreeMap<Address, U256>,
    pub(super) total_pos_staking: U256,
}

#[derive(Debug, Clone)]
pub(crate) struct PoSStateRequirements {
    accounts: BTreeSet<Address>,
    identifiers: BTreeSet<B256>,
}

#[derive(Debug, Default)]
pub(crate) struct PoSStateReader {
    before_state: Option<PoSStateValues>,
}

impl PoSStateReader {
    pub(crate) fn read(
        &mut self,
        state: &State,
        committed_pos_calls: &[CommittedPoSCall],
        requirements: &PoSStateRequirements,
        phase: StatePhase,
    ) -> Result<Option<PoSStateValues>, CoreSpaceChangesError> {
        if committed_pos_calls.is_empty() {
            self.before_state = None;
            return Ok(None);
        }

        match phase {
            StatePhase::Before => {
                let before = read_pos_state_values(state, StatePhase::Before, requirements)?;
                self.before_state = Some(before.clone());
                Ok(Some(before))
            }
            StatePhase::After => {
                let Some(before) = self.before_state.as_ref() else {
                    return Err(CoreSpaceChangesError::inconsistent_execution(
                        "Core Space PoS before state was not collected",
                    ));
                };
                let requirements = requirements.including_identifiers_from(before);
                Ok(Some(read_pos_state_values(
                    state,
                    StatePhase::After,
                    &requirements,
                )?))
            }
        }
    }
}

impl PoSStateRequirements {
    pub(crate) fn from_pos_calls(committed_pos_calls: &[CommittedPoSCall]) -> Self {
        let mut accounts = BTreeSet::new();
        let mut identifiers = BTreeSet::new();
        for committed_call in committed_pos_calls {
            match committed_call {
                CommittedPoSCall::Registration {
                    account,
                    identifier,
                    ..
                } => {
                    accounts.insert(*account);
                    identifiers.insert(*identifier);
                }
                CommittedPoSCall::StakeIncrease { account, .. }
                | CommittedPoSCall::RetirementRequest { account, .. } => {
                    accounts.insert(*account);
                }
            }
        }
        Self {
            accounts,
            identifiers,
        }
    }

    pub(crate) fn including_identifiers_from(&self, state: &PoSStateValues) -> Self {
        let mut requirements = self.clone();
        requirements
            .identifiers
            .extend(state.accounts_by_identifier.keys().copied());
        requirements
    }
}

pub(crate) fn read_pos_state_values(
    state: &State,
    phase: StatePhase,
    requirements: &PoSStateRequirements,
) -> Result<PoSStateValues, CoreSpaceChangesError> {
    let mut identifiers_by_account = BTreeMap::new();
    for account in &requirements.accounts {
        let value = read_pos_storage(
            state,
            identifier_entry(&address_to_cfx(*account)),
            phase,
            "identifier",
        )?;
        identifiers_by_account.insert(*account, b256_from_cfx(H256::from_uint(&value)));
    }

    let mut identifiers = requirements.identifiers.clone();
    identifiers.extend(
        identifiers_by_account
            .values()
            .copied()
            .filter(|identifier| !identifier.is_zero()),
    );

    let mut accounts_by_identifier = BTreeMap::new();
    let mut statuses_by_identifier = BTreeMap::new();
    for identifier in identifiers {
        let storage_identifier = b256_to_cfx(identifier);
        let address_value = read_pos_storage(
            state,
            address_entry(&storage_identifier),
            phase,
            "identifier address",
        )?;
        accounts_by_identifier.insert(identifier, canonical_storage_address(address_value)?);
        let status_value =
            read_pos_storage(state, index_entry(&storage_identifier), phase, "status")?;
        statuses_by_identifier.insert(identifier, canonical_pos_status(status_value)?);
    }

    let mut staking_balances_by_account = BTreeMap::new();
    for account in &requirements.accounts {
        let balance = state
            .staking_balance(&address_to_cfx(*account))
            .map_err(|error| {
                CoreSpaceChangesError::state_read(
                    format!("read {phase} Core Space PoS staking balance for {account}"),
                    error,
                )
            })?;
        staking_balances_by_account.insert(*account, u256_from_cfx(balance));
    }

    Ok(PoSStateValues {
        identifiers_by_account,
        accounts_by_identifier,
        statuses_by_identifier,
        staking_balances_by_account,
        total_pos_staking: u256_from_cfx(state.total_pos_staking_tokens()),
    })
}

pub(super) fn identifier_for_account(
    state: &PoSStateValues,
    account: Address,
) -> Result<B256, CoreSpaceChangesError> {
    state
        .identifiers_by_account
        .get(&account)
        .copied()
        .ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space PoS state values did not include sender identifier for {account}"
            ))
        })
}

pub(super) fn account_for_identifier(
    state: &PoSStateValues,
    identifier: B256,
) -> Result<Address, CoreSpaceChangesError> {
    state
        .accounts_by_identifier
        .get(&identifier)
        .copied()
        .ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space PoS state values did not include identifier address for {identifier}"
            ))
        })
}

pub(super) fn pos_status(
    state: &PoSStateValues,
    identifier: B256,
) -> Result<PoSStatus, CoreSpaceChangesError> {
    state
        .statuses_by_identifier
        .get(&identifier)
        .copied()
        .ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space PoS state values did not include status for {identifier}"
            ))
        })
}

pub(super) fn verify_mapping(
    state: &PoSStateValues,
    identifier: B256,
    account: Address,
) -> Result<(), CoreSpaceChangesError> {
    if account_for_identifier(state, identifier)? != account {
        return Err(CoreSpaceChangesError::inconsistent_execution(format!(
            "Core Space PoS sender and identifier mappings disagree for {account}"
        )));
    }
    Ok(())
}

fn read_pos_storage(
    state: &State,
    key: Vec<u8>,
    phase: StatePhase,
    field: &str,
) -> Result<cfx_types::U256, CoreSpaceChangesError> {
    let pos_register_contract_address =
        cfx_parameters::internal_contract_addresses::POS_REGISTER_CONTRACT_ADDRESS
            .with_native_space();
    state
        .storage_at(&pos_register_contract_address, &key)
        .map_err(|error| {
            CoreSpaceChangesError::state_read(format!("read {phase} Core Space PoS {field}"), error)
        })
}

fn canonical_storage_address(value: cfx_types::U256) -> Result<Address, CoreSpaceChangesError> {
    let address_hash: H256 = BigEndianHash::from_uint(&value);
    if address_hash.as_bytes()[..12].iter().any(|byte| *byte != 0) {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "Core Space PoS identifier address storage has noncanonical high bytes",
        ));
    }
    Ok(address_from_cfx(CfxAddress::from(address_hash)))
}

fn canonical_pos_status(value: cfx_types::U256) -> Result<PoSStatus, CoreSpaceChangesError> {
    let limbs = value.0;
    if limbs[2] != 0 || limbs[3] != 0 {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "Core Space PoS status storage has noncanonical high limbs",
        ));
    }
    if limbs[1] > limbs[0] {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "Core Space PoS status has unlocked votes above registered votes",
        ));
    }
    Ok(PoSStatus {
        registered: limbs[0],
        unlocked: limbs[1],
    })
}
