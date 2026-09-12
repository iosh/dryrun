mod evidence;

use std::collections::{BTreeMap, BTreeSet};

use alloy_primitives::{Address, B256, U256};
use conflux_provider::CoreAddress;

use self::evidence::{CommittedPoSOperation, collect_operations};
use super::{CoreSpaceChangeSet, CoreSpaceChangeSetBuilder};
use crate::{
    core_space::{
        CoreSpaceChangesError, CoreSpaceExecutedTransaction, CoreSpacePoSRegistrationState,
        CoreSpaceStateAccess, CoreSpaceStateAccessError, CoreSpaceStateReader,
    },
    primitive::u256_from_cfx,
};

pub(super) fn derive_changes(
    execution: &CoreSpaceExecutedTransaction,
    state: &CoreSpaceStateAccess,
) -> Result<CoreSpaceChangeSet, CoreSpaceChangesError> {
    let operations = collect_operations(execution)?;
    if operations.is_empty() {
        return Ok(CoreSpaceChangeSet::default());
    }

    let affected_accounts = operations
        .iter()
        .map(CommittedPoSOperation::account)
        .collect::<BTreeSet<_>>();
    let initial_identifiers_by_account = read_identifiers_by_account(
        state.initial(),
        &affected_accounts,
        execution,
        StatePhase::Initial,
    )?;
    let finalized_identifiers_by_account = read_identifiers_by_account(
        state.finalized(),
        &affected_accounts,
        execution,
        StatePhase::Finalized,
    )?;

    let mut required_identifiers = operations
        .iter()
        .map(CommittedPoSOperation::identifier)
        .collect::<BTreeSet<_>>();
    required_identifiers.extend(initial_identifiers_by_account.values().flatten().copied());
    required_identifiers.extend(finalized_identifiers_by_account.values().flatten().copied());

    let initial_state = PoSStateSnapshot::read(
        state.initial(),
        initial_identifiers_by_account,
        &required_identifiers,
        StatePhase::Initial,
    )?;
    let finalized_state = PoSStateSnapshot::read(
        state.finalized(),
        finalized_identifiers_by_account,
        &required_identifiers,
        StatePhase::Finalized,
    )?;
    initial_state.verify_account_mappings()?;
    finalized_state.verify_account_mappings()?;

    let mut replayed_state = initial_state;
    let mut changes = CoreSpaceChangeSetBuilder::new();
    for operation in operations {
        match operation {
            CommittedPoSOperation::Registration {
                position,
                account,
                identifier,
                initial_vote_count,
                bls_public_key,
                vrf_public_key,
            } => {
                replayed_state.apply_registration(account, identifier);
                let locked_amount =
                    replayed_state.add_registered_votes(identifier, initial_vote_count)?;
                changes
                    .pos_registration(
                        position,
                        core_address(account, execution),
                        identifier,
                        bls_public_key,
                        vrf_public_key,
                        initial_vote_count,
                        locked_amount,
                    )
                    .expect("built-in Core Space changes use unique execution positions");
            }
            CommittedPoSOperation::StakeIncrease {
                position,
                account,
                identifier: event_identifier,
                added_vote_count,
            } => {
                let registered_identifier = replayed_state.identifier_for_account(account)?;
                verify_event_identifier("increaseStake", event_identifier, registered_identifier)?;
                let locked_amount =
                    replayed_state.add_registered_votes(registered_identifier, added_vote_count)?;
                changes
                    .pos_stake_increase(
                        position,
                        core_address(account, execution),
                        registered_identifier,
                        added_vote_count,
                        locked_amount,
                    )
                    .expect("built-in Core Space changes use unique execution positions");
            }
            CommittedPoSOperation::RetirementRequest {
                position,
                account,
                identifier: event_identifier,
                requested_vote_count,
            } => {
                let registered_identifier = replayed_state.identifier_for_account(account)?;
                verify_event_identifier("retire", event_identifier, registered_identifier)?;
                changes
                    .pos_retirement_request(
                        position,
                        core_address(account, execution),
                        registered_identifier,
                        requested_vote_count,
                    )
                    .expect("built-in Core Space changes use unique execution positions");
            }
        }
    }

    if replayed_state != finalized_state {
        return Err(inconsistent(
            "replayed Core Space PoS state does not match finalized state",
        ));
    }
    Ok(changes.finish())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PoSStateSnapshot {
    identifiers_by_account: BTreeMap<Address, Option<B256>>,
    registrations_by_identifier: BTreeMap<B256, PoSRegistrationSnapshot>,
    total_pos_staking: U256,
}

impl PoSStateSnapshot {
    fn read(
        reader: &CoreSpaceStateReader,
        identifiers_by_account: BTreeMap<Address, Option<B256>>,
        required_identifiers: &BTreeSet<B256>,
        phase: StatePhase,
    ) -> Result<Self, CoreSpaceChangesError> {
        let registrations_by_identifier = required_identifiers
            .iter()
            .map(|identifier| {
                let registration = reader
                    .pos_registration_for_identifier(*identifier)
                    .map_err(|source| state_error(phase, source))?;
                Ok((
                    *identifier,
                    PoSRegistrationSnapshot::from_state(registration),
                ))
            })
            .collect::<Result<_, CoreSpaceChangesError>>()?;
        let total_pos_staking = reader
            .total_pos_staking()
            .map_err(|source| state_error(phase, source))?;
        Ok(Self {
            identifiers_by_account,
            registrations_by_identifier,
            total_pos_staking,
        })
    }

    fn verify_account_mappings(&self) -> Result<(), CoreSpaceChangesError> {
        for (account, identifier) in &self.identifiers_by_account {
            let Some(identifier) = identifier else {
                continue;
            };
            let registration = self
                .registrations_by_identifier
                .get(identifier)
                .expect("PoS state snapshot includes every account identifier");
            if registration.account != Some(*account) {
                return Err(inconsistent(
                    "Core Space PoS forward and reverse account mappings disagree",
                ));
            }
        }
        Ok(())
    }

    fn identifier_for_account(&self, account: Address) -> Result<B256, CoreSpaceChangesError> {
        let identifier = self
            .identifiers_by_account
            .get(&account)
            .expect("PoS replay includes every operation account");
        identifier.ok_or_else(|| {
            inconsistent("Core Space PoS operation has no registered account identifier")
        })
    }

    fn apply_registration(&mut self, account: Address, identifier: B256) {
        let registration = self
            .registrations_by_identifier
            .get_mut(&identifier)
            .expect("PoS replay includes every operation identifier");
        registration.account = Some(account);
        self.identifiers_by_account
            .insert(account, Some(identifier));
    }

    fn add_registered_votes(
        &mut self,
        identifier: B256,
        added_vote_count: u64,
    ) -> Result<U256, CoreSpaceChangesError> {
        let registration = self
            .registrations_by_identifier
            .get_mut(&identifier)
            .expect("PoS replay includes every operation identifier");
        registration.registered_vote_count = registration
            .registered_vote_count
            .checked_add(added_vote_count)
            .expect("a committed PoS increase cannot overflow its registered vote count");

        let locked_amount = locked_amount_for_votes(added_vote_count);
        self.total_pos_staking = self
            .total_pos_staking
            .checked_add(locked_amount)
            .ok_or_else(|| inconsistent("Core Space total PoS staking overflowed"))?;
        Ok(locked_amount)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PoSRegistrationSnapshot {
    account: Option<Address>,
    registered_vote_count: u64,
    unlocked_vote_count: u64,
}

impl PoSRegistrationSnapshot {
    fn from_state(state: CoreSpacePoSRegistrationState) -> Self {
        Self {
            account: state
                .account()
                .map(|account| Address::from(account.bytes())),
            registered_vote_count: state.registered_vote_count(),
            unlocked_vote_count: state.unlocked_vote_count(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum StatePhase {
    Initial,
    Finalized,
}

impl StatePhase {
    const fn read_operation(self) -> &'static str {
        match self {
            Self::Initial => "read initial Core Space PoS state",
            Self::Finalized => "read finalized Core Space PoS state",
        }
    }
}

fn read_identifiers_by_account(
    reader: &CoreSpaceStateReader,
    accounts: &BTreeSet<Address>,
    execution: &CoreSpaceExecutedTransaction,
    phase: StatePhase,
) -> Result<BTreeMap<Address, Option<B256>>, CoreSpaceChangesError> {
    accounts
        .iter()
        .map(|account| {
            let identifier = reader
                .pos_identifier_for_account(core_address(*account, execution))
                .map_err(|source| state_error(phase, source))?;
            Ok((*account, identifier))
        })
        .collect()
}

fn verify_event_identifier(
    operation: &'static str,
    event_identifier: B256,
    registered_identifier: B256,
) -> Result<(), CoreSpaceChangesError> {
    if event_identifier != registered_identifier {
        return Err(inconsistent(format!(
            "Core Space PoS {operation} event is not backed by the account registration"
        )));
    }
    Ok(())
}

fn locked_amount_for_votes(vote_count: u64) -> U256 {
    U256::from(vote_count) * u256_from_cfx(*cfx_parameters::staking::POS_VOTE_PRICE)
}

fn core_address(address: Address, execution: &CoreSpaceExecutedTransaction) -> CoreAddress {
    CoreAddress::from_bytes(*address.0, execution.address_network)
        .expect("executed Core Space addresses retain a validated network")
}

fn state_error(phase: StatePhase, source: CoreSpaceStateAccessError) -> CoreSpaceChangesError {
    CoreSpaceChangesError::state_access(phase.read_operation(), source)
}

fn inconsistent(details: impl Into<String>) -> CoreSpaceChangesError {
    CoreSpaceChangesError::inconsistent_execution(details)
}
