use alloy_primitives::{B256, U256};
use alloy_sol_types::{SolEvent, sol};
use cfx_types::Space;
use primitives::LogEntry;

use crate::{ConfluxSimulationError, primitive::b256_from_cfx};

sol! {
    event Register(bytes32 indexed pos_identifier, bytes verified_bls_pubkey, bytes vrf_pubkey);
    event IncreaseStake(bytes32 indexed pos_identifier, uint64 vote_count);
    event Retire(bytes32 indexed pos_identifier, uint64 requested_vote_count);
}

const VOTE_LOCK_SELECTOR: [u8; 4] = [0x44, 0xa5, 0x1d, 0x6d];
const POS_REGISTER_SELECTOR: [u8; 4] = [0xe3, 0x35, 0xb4, 0x51];
const POS_INCREASE_STAKE_SELECTOR: [u8; 4] = [0x09, 0xfe, 0xcf, 0x7f];
const POS_RETIRE_SELECTOR: [u8; 4] = [0xf4, 0x9d, 0x06, 0x38];

pub(super) struct VoteLockCall {
    pub(super) amount: U256,
    pub(super) unlock_block_number: u64,
}

pub(super) enum PoSCall {
    Registration {
        pos_identifier: B256,
        vote_count: u64,
    },
    StakeIncrease {
        vote_count: u64,
    },
    RetirementRequest {
        requested_vote_count: u64,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PoSEvent {
    Register {
        pos_identifier: B256,
    },
    IncreaseStake {
        pos_identifier: B256,
        vote_count: u64,
    },
    Retire {
        pos_identifier: B256,
        requested_vote_count: u64,
    },
}

pub(super) fn decode_vote_lock_call(
    calldata_len: usize,
    calldata_prefix: &[u8],
) -> Result<Option<VoteLockCall>, ConfluxSimulationError> {
    if call_selector(calldata_len, calldata_prefix) != Some(VOTE_LOCK_SELECTOR) {
        return Ok(None);
    }
    Ok(Some(VoteLockCall {
        amount: U256::from_be_bytes(read_call_word(
            calldata_len,
            calldata_prefix,
            4,
            "voteLock amount",
        )?),
        unlock_block_number: low_u64(read_call_word(
            calldata_len,
            calldata_prefix,
            36,
            "voteLock unlock block number",
        )?),
    }))
}

pub(super) fn decode_pos_call(
    calldata_len: usize,
    calldata_prefix: &[u8],
) -> Result<Option<PoSCall>, ConfluxSimulationError> {
    let Some(selector) = call_selector(calldata_len, calldata_prefix) else {
        return Ok(None);
    };
    match selector {
        POS_REGISTER_SELECTOR => Ok(Some(PoSCall::Registration {
            pos_identifier: B256::from(read_call_word(
                calldata_len,
                calldata_prefix,
                4,
                "PoS register identifier",
            )?),
            vote_count: low_u64(read_call_word(
                calldata_len,
                calldata_prefix,
                36,
                "PoS register vote count",
            )?),
        })),
        POS_INCREASE_STAKE_SELECTOR => Ok(Some(PoSCall::StakeIncrease {
            vote_count: low_u64(read_call_word(
                calldata_len,
                calldata_prefix,
                4,
                "PoS increase vote count",
            )?),
        })),
        POS_RETIRE_SELECTOR => Ok(Some(PoSCall::RetirementRequest {
            requested_vote_count: low_u64(read_call_word(
                calldata_len,
                calldata_prefix,
                4,
                "PoS retire vote count",
            )?),
        })),
        _ => Ok(None),
    }
}

pub(crate) fn decode_pos_staking_events(
    final_logs: &[LogEntry],
    pos_register_contract_active: bool,
) -> Result<Vec<PoSEvent>, ConfluxSimulationError> {
    if !pos_register_contract_active {
        return Ok(Vec::new());
    }
    let pos_register_contract_address =
        cfx_parameters::internal_contract_addresses::POS_REGISTER_CONTRACT_ADDRESS;
    final_logs
        .iter()
        .filter(|log| log.space == Space::Native && log.address == pos_register_contract_address)
        .map(decode_pos_event)
        .collect()
}

fn call_selector(calldata_len: usize, calldata_prefix: &[u8]) -> Option<[u8; 4]> {
    if calldata_len < 4 || calldata_prefix.len() < 4 {
        return None;
    }
    let mut selector = [0_u8; 4];
    selector.copy_from_slice(&calldata_prefix[..4]);
    Some(selector)
}

fn read_call_word(
    calldata_len: usize,
    calldata_prefix: &[u8],
    offset: usize,
    field: &str,
) -> Result<[u8; 32], ConfluxSimulationError> {
    let end = offset + 32;
    if calldata_len < end || calldata_prefix.len() < end {
        return Err(ConfluxSimulationError::analysis_failed(format!(
            "Core Space {field} was not fully captured in the calldata prefix"
        )));
    }
    let mut word = [0_u8; 32];
    word.copy_from_slice(&calldata_prefix[offset..end]);
    Ok(word)
}

fn decode_pos_event(log: &LogEntry) -> Result<PoSEvent, ConfluxSimulationError> {
    if log.topics.len() != 2 {
        return Err(ConfluxSimulationError::analysis_failed(
            "Core Space PoS event did not have exactly two topics",
        ));
    }
    let event_signature = b256_from_cfx(log.topics[0]);
    let data = log.data.as_ref();

    if event_signature == Register::SIGNATURE_HASH {
        let event =
            Register::decode_raw_log_validate(log.topics.iter().copied().map(b256_from_cfx), data)
                .map_err(|error| {
                    ConfluxSimulationError::analysis_failed(format!(
                        "Core Space PoS Register event is not canonical ABI data: {error}"
                    ))
                })?;
        verify_encoded_event_data(data, event.encode_data(), "Register")?;
        Ok(PoSEvent::Register {
            pos_identifier: event.pos_identifier,
        })
    } else if event_signature == IncreaseStake::SIGNATURE_HASH {
        let event = IncreaseStake::decode_raw_log_validate(
            log.topics.iter().copied().map(b256_from_cfx),
            data,
        )
        .map_err(|error| {
            ConfluxSimulationError::analysis_failed(format!(
                "Core Space PoS IncreaseStake event is not canonical ABI data: {error}"
            ))
        })?;
        verify_encoded_event_data(data, event.encode_data(), "IncreaseStake")?;
        Ok(PoSEvent::IncreaseStake {
            pos_identifier: event.pos_identifier,
            vote_count: event.vote_count,
        })
    } else if event_signature == Retire::SIGNATURE_HASH {
        let event =
            Retire::decode_raw_log_validate(log.topics.iter().copied().map(b256_from_cfx), data)
                .map_err(|error| {
                    ConfluxSimulationError::analysis_failed(format!(
                        "Core Space PoS Retire event is not canonical ABI data: {error}"
                    ))
                })?;
        verify_encoded_event_data(data, event.encode_data(), "Retire")?;
        Ok(PoSEvent::Retire {
            pos_identifier: event.pos_identifier,
            requested_vote_count: event.requested_vote_count,
        })
    } else {
        Err(ConfluxSimulationError::analysis_failed(
            "Core Space PoS log had an unknown event signature",
        ))
    }
}

fn verify_encoded_event_data(
    data: &[u8],
    encoded_data: Vec<u8>,
    event_name: &str,
) -> Result<(), ConfluxSimulationError> {
    if encoded_data != data {
        return Err(ConfluxSimulationError::analysis_failed(format!(
            "Core Space PoS {event_name} event has noncanonical or trailing ABI data"
        )));
    }
    Ok(())
}

fn low_u64(word: [u8; 32]) -> u64 {
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&word[24..]);
    u64::from_be_bytes(bytes)
}
