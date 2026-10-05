use super::{Address, PosStake, ProtocolChange, contract_callers};
use crate::{Error, address::VmAddress, primitive::*};
use alloy::primitives::B256;
use alloy_sol_types::{SolEvent, sol};
use cfx_executor::{
    internal_contract::{IndexStatus, pos_internal_entries},
    state::State,
};
use cfx_parameters::internal_contract_addresses::POS_REGISTER_CONTRACT_ADDRESS;
use cfx_types::{AddressSpaceUtil, BigEndianHash, H256};
use conflux_provider::Network;
use simulation_core::{ChainAddress, ExecutionTrace};
use std::collections::BTreeSet;

sol! {
    event Retire(bytes32 indexed identifier, uint64 votes);
}

pub(super) fn derive(
    trace: &ExecutionTrace<Address>,
    before: &State,
    after: &State,
    network: Network,
) -> Result<Vec<ProtocolChange>, Error> {
    let contract = Address::from_vm(POS_REGISTER_CONTRACT_ADDRESS.with_native_space(), network);
    // Retirement has a retained log but no local index update. Its later
    // consensus effect cannot be described by these state differences.
    if trace.logs.iter().any(|log| {
        log.address == contract && log.data.topics().first() == Some(&Retire::SIGNATURE_HASH)
    }) {
        return Err(Error::Unsupported(
            "PoS retirement requires consensus effects unavailable to local simulation".into(),
        ));
    }

    let mut changes = Vec::new();
    let mut identifiers = BTreeSet::new();
    for account in contract_callers(trace, contract)? {
        let old = read_identifier(before, contract, account)?;
        let new = read_identifier(after, contract, account)?;
        identifiers.extend([old, new].into_iter().filter(|id| !id.is_zero()));
        if old != new {
            changes.push(ProtocolChange::PosIdentifier {
                account,
                before: (!old.is_zero()).then_some(old),
                after: (!new.is_zero()).then_some(new),
            });
        }
    }
    for identifier in identifiers {
        let old = read_stake(before, contract, identifier)?;
        let new = read_stake(after, contract, identifier)?;
        if old != new {
            changes.push(ProtocolChange::PosStake {
                identifier,
                before: old,
                after: new,
            });
        }
    }
    Ok(changes)
}

fn read_identifier(state: &State, contract: Address, account: Address) -> Result<B256, Error> {
    let value = state.storage_at(
        &contract.to_vm(),
        &pos_internal_entries::identifier_entry(&account.to_vm().address),
    )?;
    Ok(b256_from_cfx(H256::from_uint(&value)))
}

fn read_stake(state: &State, contract: Address, identifier: B256) -> Result<PosStake, Error> {
    let identifier = b256_to_cfx(identifier);
    let address = state.storage_at(
        &contract.to_vm(),
        &pos_internal_entries::address_entry(&identifier),
    )?;
    let address = cfx_types::Address::from(H256::from_uint(&address));
    let status: IndexStatus = state
        .storage_at(
            &contract.to_vm(),
            &pos_internal_entries::index_entry(&identifier),
        )?
        .into();
    Ok(PosStake {
        account: (!address.is_zero()).then(|| contract.with_raw(address_from_cfx(address))),
        registered: status.registered,
        unlocked: status.unlocked,
    })
}
