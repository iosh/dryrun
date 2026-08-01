use std::collections::{BTreeMap, BTreeSet};

use alloy_primitives::{Address, B256, U256};
use cfx_executor::{
    internal_contract::pos_internal_entries::{address_entry, identifier_entry, index_entry},
    state::State,
};
use cfx_types::{Address as CfxAddress, AddressSpaceUtil, BigEndianHash, H256};
use contract_standards::StatePhase;

use super::{CommittedStakingCalls, StakingCall};
use crate::{
    ConfluxEngineError,
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
    pub(super) sender_pos_identifiers: BTreeMap<Address, B256>,
    pub(super) pos_identifier_accounts: BTreeMap<B256, Address>,
    pub(super) pos_statuses: BTreeMap<B256, PoSStatus>,
    pub(super) staking_balances: BTreeMap<Address, U256>,
    pub(super) total_pos_staking: U256,
}

#[derive(Debug, Clone)]
pub(crate) struct PoSStateRequirements {
    accounts: BTreeSet<Address>,
    pos_identifiers: BTreeSet<B256>,
}

impl PoSStateRequirements {
    pub(crate) fn from_committed_calls(committed_staking_calls: &CommittedStakingCalls) -> Self {
        let mut accounts = BTreeSet::new();
        let mut pos_identifiers = BTreeSet::new();
        for committed_call in committed_staking_calls.iter() {
            match committed_call {
                StakingCall::PoSRegistration {
                    account,
                    pos_identifier,
                    ..
                } => {
                    accounts.insert(*account);
                    pos_identifiers.insert(*pos_identifier);
                }
                StakingCall::PoSStakeIncrease { account, .. }
                | StakingCall::PoSRetirementRequest { account, .. } => {
                    accounts.insert(*account);
                }
                StakingCall::VoteLock { .. } => {}
            }
        }
        Self {
            accounts,
            pos_identifiers,
        }
    }

    pub(crate) fn including_identifiers_from(&self, state: &PoSStateValues) -> Self {
        let mut requirements = self.clone();
        requirements
            .pos_identifiers
            .extend(state.pos_identifier_accounts.keys().copied());
        requirements
    }
}

pub(crate) fn read_pos_state_values(
    state: &State,
    phase: StatePhase,
    requirements: &PoSStateRequirements,
) -> Result<PoSStateValues, ConfluxEngineError> {
    let mut sender_pos_identifiers = BTreeMap::new();
    for account in &requirements.accounts {
        let value = read_pos_storage(
            state,
            identifier_entry(&address_to_cfx(*account)),
            phase,
            "identifier",
        )?;
        sender_pos_identifiers.insert(*account, b256_from_cfx(H256::from_uint(&value)));
    }

    let mut pos_identifiers = requirements.pos_identifiers.clone();
    pos_identifiers.extend(
        sender_pos_identifiers
            .values()
            .copied()
            .filter(|pos_identifier| !pos_identifier.is_zero()),
    );

    let mut pos_identifier_accounts = BTreeMap::new();
    let mut pos_statuses = BTreeMap::new();
    for pos_identifier in pos_identifiers {
        let identifier = b256_to_cfx(pos_identifier);
        let address_value = read_pos_storage(
            state,
            address_entry(&identifier),
            phase,
            "identifier address",
        )?;
        pos_identifier_accounts.insert(pos_identifier, canonical_storage_address(address_value)?);
        let status_value = read_pos_storage(state, index_entry(&identifier), phase, "status")?;
        pos_statuses.insert(pos_identifier, canonical_pos_status(status_value)?);
    }

    let mut staking_balances = BTreeMap::new();
    for account in &requirements.accounts {
        let balance = state
            .staking_balance(&address_to_cfx(*account))
            .map_err(|error| ConfluxEngineError::StateAccess {
                message: format!(
                    "failed to read {phase} Core Space PoS staking balance for {account}: {error}"
                ),
            })?;
        staking_balances.insert(*account, u256_from_cfx(balance));
    }

    Ok(PoSStateValues {
        sender_pos_identifiers,
        pos_identifier_accounts,
        pos_statuses,
        staking_balances,
        total_pos_staking: u256_from_cfx(state.total_pos_staking_tokens()),
    })
}

pub(super) fn sender_pos_identifier(
    state: &PoSStateValues,
    account: Address,
) -> Result<B256, ConfluxEngineError> {
    state
        .sender_pos_identifiers
        .get(&account)
        .copied()
        .ok_or_else(|| {
            ConfluxEngineError::analysis_failed(format!(
                "Core Space PoS state values did not include sender identifier for {account}"
            ))
        })
}

pub(super) fn pos_identifier_account(
    state: &PoSStateValues,
    pos_identifier: B256,
) -> Result<Address, ConfluxEngineError> {
    state
        .pos_identifier_accounts
        .get(&pos_identifier)
        .copied()
        .ok_or_else(|| {
            ConfluxEngineError::analysis_failed(format!(
                "Core Space PoS state values did not include identifier address for {pos_identifier}"
            ))
        })
}

pub(super) fn pos_status(
    state: &PoSStateValues,
    pos_identifier: B256,
) -> Result<PoSStatus, ConfluxEngineError> {
    state
        .pos_statuses
        .get(&pos_identifier)
        .copied()
        .ok_or_else(|| {
            ConfluxEngineError::analysis_failed(format!(
                "Core Space PoS state values did not include status for {pos_identifier}"
            ))
        })
}

pub(super) fn verify_pos_identifier_account_pair(
    state: &PoSStateValues,
    pos_identifier: B256,
    account: Address,
) -> Result<(), ConfluxEngineError> {
    if pos_identifier_account(state, pos_identifier)? != account {
        return Err(ConfluxEngineError::analysis_failed(format!(
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
) -> Result<cfx_types::U256, ConfluxEngineError> {
    let pos_register_contract_address =
        cfx_parameters::internal_contract_addresses::POS_REGISTER_CONTRACT_ADDRESS
            .with_native_space();
    state
        .storage_at(&pos_register_contract_address, &key)
        .map_err(|error| ConfluxEngineError::StateAccess {
            message: format!("failed to read {phase} Core Space PoS {field}: {error}"),
        })
}

fn canonical_storage_address(value: cfx_types::U256) -> Result<Address, ConfluxEngineError> {
    let address_hash: H256 = BigEndianHash::from_uint(&value);
    if address_hash.as_bytes()[..12].iter().any(|byte| *byte != 0) {
        return Err(ConfluxEngineError::analysis_failed(
            "Core Space PoS identifier address storage has noncanonical high bytes",
        ));
    }
    Ok(address_from_cfx(CfxAddress::from(address_hash)))
}

fn canonical_pos_status(value: cfx_types::U256) -> Result<PoSStatus, ConfluxEngineError> {
    let limbs = value.0;
    if limbs[2] != 0 || limbs[3] != 0 {
        return Err(ConfluxEngineError::analysis_failed(
            "Core Space PoS status storage has noncanonical high limbs",
        ));
    }
    if limbs[1] > limbs[0] {
        return Err(ConfluxEngineError::analysis_failed(
            "Core Space PoS status has unlocked votes above registered votes",
        ));
    }
    Ok(PoSStatus {
        registered: limbs[0],
        unlocked: limbs[1],
    })
}
