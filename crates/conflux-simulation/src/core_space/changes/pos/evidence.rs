use alloy_primitives::{Address, B256, Bytes};
use alloy_sol_types::SolEvent;
use cfx_types::Space;
use cfx_vm_types::CallType;

use crate::{
    core_space::{CoreSpaceChangesError, CoreSpaceExecutedTransaction, CoreSpaceExecutionPosition},
    execution::{CommittedExecutionTrace, FrameAction, FrameId, TraceEvent},
    primitive::{address_from_cfx, b256_from_cfx},
};

mod events {
    alloy_sol_types::sol! {
        event Register(bytes32 indexed identifier, bytes verified_bls_pubkey, bytes vrf_pubkey);
        event IncreaseStake(bytes32 indexed identifier, uint64 vote_count);
        event Retire(bytes32 indexed identifier, uint64 requested_vote_count);
    }
}

const POS_REGISTER_SELECTOR: [u8; 4] = [0xe3, 0x35, 0xb4, 0x51];
const POS_INCREASE_STAKE_SELECTOR: [u8; 4] = [0x09, 0xfe, 0xcf, 0x7f];
const POS_RETIRE_SELECTOR: [u8; 4] = [0xf4, 0x9d, 0x06, 0x38];

#[derive(Debug, Clone)]
pub(super) enum CommittedPoSOperation {
    Registration {
        position: CoreSpaceExecutionPosition,
        account: Address,
        identifier: B256,
        initial_vote_count: u64,
        bls_public_key: Bytes,
        vrf_public_key: Bytes,
    },
    StakeIncrease {
        position: CoreSpaceExecutionPosition,
        account: Address,
        identifier: B256,
        added_vote_count: u64,
    },
    RetirementRequest {
        position: CoreSpaceExecutionPosition,
        account: Address,
        identifier: B256,
        requested_vote_count: u64,
    },
}

impl CommittedPoSOperation {
    pub(super) const fn account(&self) -> Address {
        match self {
            Self::Registration { account, .. }
            | Self::StakeIncrease { account, .. }
            | Self::RetirementRequest { account, .. } => *account,
        }
    }

    pub(super) const fn identifier(&self) -> B256 {
        match self {
            Self::Registration { identifier, .. }
            | Self::StakeIncrease { identifier, .. }
            | Self::RetirementRequest { identifier, .. } => *identifier,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum PoSCallData {
    Registration {
        identifier: B256,
        initial_vote_count: u64,
    },
    StakeIncrease {
        added_vote_count: u64,
    },
    RetirementRequest {
        requested_vote_count: u64,
    },
}

pub(super) fn collect_operations(
    execution: &CoreSpaceExecutedTransaction,
) -> Result<Vec<CommittedPoSOperation>, CoreSpaceChangesError> {
    let pos_contract = cfx_parameters::internal_contract_addresses::POS_REGISTER_CONTRACT_ADDRESS;
    if !execution.is_active_internal_contract(pos_contract) {
        return Ok(Vec::new());
    }

    let committed_trace = execution.trace();
    let mut operations = Vec::new();
    for trace_event in committed_trace.events() {
        let TraceEvent::FrameStart { position, frame_id } = trace_event else {
            continue;
        };
        let frame = committed_trace.frame(*frame_id);
        let FrameAction::Call {
            caller,
            target,
            code_address,
            transferred_value,
            call_type,
            calldata,
            ..
        } = &frame.action
        else {
            continue;
        };
        if *target != pos_contract && *code_address != pos_contract {
            continue;
        }
        let Some(call_data) = decode_call_data(calldata)? else {
            continue;
        };
        if frame.space != Space::Native
            || *call_type != CallType::Call
            || *target != pos_contract
            || *code_address != pos_contract
            || !transferred_value.is_zero()
        {
            return Err(CoreSpaceChangesError::unsupported_operation(
                "Core Space PoS call did not use the canonical native plain-call form",
            ));
        }

        let protocol_logs = pos_logs_for_frame(committed_trace, *frame_id, pos_contract);
        let position = CoreSpaceExecutionPosition::from_index(*position);
        let account = address_from_cfx(*caller);
        operations.push(match_operation_evidence(
            position,
            account,
            call_data,
            &protocol_logs,
        )?);
    }

    Ok(operations)
}

fn decode_call_data(calldata: &[u8]) -> Result<Option<PoSCallData>, CoreSpaceChangesError> {
    let Some(selector) = calldata.get(..4) else {
        return Ok(None);
    };
    if selector == POS_REGISTER_SELECTOR {
        return Ok(Some(PoSCallData::Registration {
            identifier: B256::from(read_abi_word(calldata, 4, "PoS register identifier")?),
            initial_vote_count: low_u64(read_abi_word(calldata, 36, "PoS register vote count")?),
        }));
    }
    if selector == POS_INCREASE_STAKE_SELECTOR {
        return Ok(Some(PoSCallData::StakeIncrease {
            added_vote_count: low_u64(read_abi_word(calldata, 4, "PoS increase vote count")?),
        }));
    }
    if selector == POS_RETIRE_SELECTOR {
        return Ok(Some(PoSCallData::RetirementRequest {
            requested_vote_count: low_u64(read_abi_word(calldata, 4, "PoS retire vote count")?),
        }));
    }
    Ok(None)
}

fn match_operation_evidence(
    position: CoreSpaceExecutionPosition,
    account: Address,
    call_data: PoSCallData,
    protocol_logs: &[PoSProtocolLog<'_>],
) -> Result<CommittedPoSOperation, CoreSpaceChangesError> {
    match call_data {
        PoSCallData::Registration {
            identifier,
            initial_vote_count,
        } => {
            let [register_log, increase_log] = protocol_logs else {
                return Err(log_count_error("register", 2, protocol_logs.len()));
            };
            let register_event = decode_register_event(*register_log)?;
            let increase_event = decode_increase_event(*increase_log)?;
            if register_event.identifier != identifier
                || increase_event.identifier != identifier
                || increase_event.vote_count != initial_vote_count
            {
                return Err(inconsistent(
                    "Core Space PoS register call and committed events disagree",
                ));
            }
            Ok(CommittedPoSOperation::Registration {
                position,
                account,
                identifier,
                initial_vote_count,
                bls_public_key: register_event.verified_bls_pubkey,
                vrf_public_key: register_event.vrf_pubkey,
            })
        }
        PoSCallData::StakeIncrease { added_vote_count } => {
            let [increase_log] = protocol_logs else {
                return Err(log_count_error("increaseStake", 1, protocol_logs.len()));
            };
            let increase_event = decode_increase_event(*increase_log)?;
            if increase_event.vote_count != added_vote_count {
                return Err(inconsistent(
                    "Core Space PoS increaseStake call and committed event disagree",
                ));
            }
            Ok(CommittedPoSOperation::StakeIncrease {
                position,
                account,
                identifier: increase_event.identifier,
                added_vote_count,
            })
        }
        PoSCallData::RetirementRequest {
            requested_vote_count,
        } => {
            let [retire_log] = protocol_logs else {
                return Err(log_count_error("retire", 1, protocol_logs.len()));
            };
            let retire_event = decode_retire_event(*retire_log)?;
            if retire_event.requested_vote_count != requested_vote_count {
                return Err(inconsistent(
                    "Core Space PoS retire call and committed event disagree",
                ));
            }
            Ok(CommittedPoSOperation::RetirementRequest {
                position,
                account,
                identifier: retire_event.identifier,
                requested_vote_count,
            })
        }
    }
}

fn read_abi_word(
    calldata: &[u8],
    offset: usize,
    field: &str,
) -> Result<[u8; 32], CoreSpaceChangesError> {
    let Some(word) = calldata.get(offset..offset + 32) else {
        return Err(inconsistent(format!(
            "Core Space {field} was missing from committed call data"
        )));
    };
    let mut result = [0_u8; 32];
    result.copy_from_slice(word);
    Ok(result)
}

fn low_u64(word: [u8; 32]) -> u64 {
    let mut value = [0_u8; 8];
    value.copy_from_slice(&word[24..]);
    u64::from_be_bytes(value)
}

#[derive(Clone, Copy)]
struct PoSProtocolLog<'a> {
    topics: &'a [cfx_types::H256],
    data: &'a [u8],
}

fn pos_logs_for_frame<'a>(
    committed_trace: &'a CommittedExecutionTrace,
    expected_frame: FrameId,
    pos_contract: cfx_types::Address,
) -> Vec<PoSProtocolLog<'a>> {
    committed_trace
        .events()
        .iter()
        .filter_map(|trace_event| match trace_event {
            TraceEvent::Log {
                frame_id,
                address,
                topics,
                data,
                ..
            } if *frame_id == expected_frame && *address == pos_contract => {
                Some(PoSProtocolLog { topics, data })
            }
            _ => None,
        })
        .collect()
}

fn decode_register_event(
    log: PoSProtocolLog<'_>,
) -> Result<events::Register, CoreSpaceChangesError> {
    events::Register::decode_raw_log_validate(
        log.topics.iter().copied().map(b256_from_cfx),
        log.data,
    )
    .map_err(|_| invalid_event("Register"))
}

fn decode_increase_event(
    log: PoSProtocolLog<'_>,
) -> Result<events::IncreaseStake, CoreSpaceChangesError> {
    events::IncreaseStake::decode_raw_log_validate(
        log.topics.iter().copied().map(b256_from_cfx),
        log.data,
    )
    .map_err(|_| invalid_event("IncreaseStake"))
}

fn decode_retire_event(log: PoSProtocolLog<'_>) -> Result<events::Retire, CoreSpaceChangesError> {
    events::Retire::decode_raw_log_validate(log.topics.iter().copied().map(b256_from_cfx), log.data)
        .map_err(|_| invalid_event("Retire"))
}

fn invalid_event(event: &'static str) -> CoreSpaceChangesError {
    inconsistent(format!(
        "Core Space PoS {event} event is not canonical ABI data"
    ))
}

fn log_count_error(operation: &str, expected: usize, actual: usize) -> CoreSpaceChangesError {
    inconsistent(format!(
        "Core Space PoS {operation} expected {expected} committed events, got {actual}"
    ))
}

fn inconsistent(details: impl Into<String>) -> CoreSpaceChangesError {
    CoreSpaceChangesError::inconsistent_execution(details)
}
