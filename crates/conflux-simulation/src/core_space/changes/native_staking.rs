use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use alloy_primitives::U256;
use cfx_executor::executive_observer::AddressPocket;
use cfx_types::{Address, Space};
use cfx_vm_types::CallType;
use conflux_provider::CoreAddress;
use primitives::VoteStakeList;

use super::{
    CoreSpaceChangeSet, CoreSpaceChangeSetBuilder, CoreSpaceNativeCurrency, CrossSpaceAddress,
};
use crate::core_space::cross_space_scope::CommittedCrossSpaceTransfer;
use crate::{
    core_space::{
        CoreSpaceChangesError, CoreSpaceExecutedTransaction, CoreSpaceExecutionPosition,
        CoreSpaceStateAccess, CoreSpaceStateAccessError,
    },
    execution::{CommittedExecutionTrace, FrameAction, FrameId, TraceEvent},
    primitive::{u256_from_cfx, u256_to_cfx},
};

const VOTE_LOCK_SELECTOR: [u8; 4] = [0x44, 0xa5, 0x1d, 0x6d];
const STAKING_DEPOSIT_SELECTOR: [u8; 4] = [0xb6, 0xb5, 0x5f, 0x25];
const STAKING_WITHDRAW_SELECTOR: [u8; 4] = [0x2e, 0x1a, 0x7d, 0x4d];

pub(super) fn derive_changes(
    execution: &CoreSpaceExecutedTransaction,
    state: &CoreSpaceStateAccess,
    currency: &CoreSpaceNativeCurrency,
) -> Result<CoreSpaceChangeSet, CoreSpaceChangesError> {
    let staking_calls = collect_staking_calls(execution)?;
    let operations = collect_native_operations(execution, &staking_calls)?;
    let mut builder = CoreSpaceChangeSetBuilder::new();
    for operation in operations {
        match operation {
            NativeOperation::Transfer {
                position,
                from,
                to,
                amount,
            } => {
                builder
                    .native_transfer(
                        position,
                        core_address(from, execution),
                        core_address(to, execution),
                        amount,
                        currency.clone(),
                    )
                    .expect("built-in Core Space changes use unique execution positions");
            }
            NativeOperation::Burn {
                position,
                account,
                amount,
            } => {
                builder
                    .native_burn(
                        position,
                        core_address(account, execution),
                        amount,
                        currency.clone(),
                    )
                    .expect("built-in Core Space changes use unique execution positions");
            }
            NativeOperation::CrossSpaceToEspace {
                position,
                core_sender,
                receiver,
                amount,
            } => {
                builder
                    .cross_space_transfer(
                        position,
                        CrossSpaceAddress::CoreSpace(core_address(core_sender, execution)),
                        CrossSpaceAddress::Espace(receiver),
                        amount,
                    )
                    .expect("built-in Core Space changes use unique execution positions");
            }
            NativeOperation::CrossSpaceToCore {
                position,
                mapped_sender,
                core_receiver,
                amount,
            } => {
                builder
                    .cross_space_transfer(
                        position,
                        CrossSpaceAddress::Espace(crate::primitive::address_from_cfx(
                            mapped_sender,
                        )),
                        CrossSpaceAddress::CoreSpace(core_address(core_receiver, execution)),
                        amount,
                    )
                    .expect("built-in Core Space changes use unique execution positions");
            }
        }
    }
    derive_staking_changes(execution, state, &staking_calls, &mut builder)?;
    Ok(builder.finish())
}

#[derive(Debug, Clone, Copy)]
enum StakingCall {
    Deposit {
        position: CoreSpaceExecutionPosition,
        account: Address,
        amount: U256,
        transfer_position: usize,
    },
    Withdrawal {
        position: CoreSpaceExecutionPosition,
        account: Address,
        principal_amount: U256,
        reward_amount: U256,
        principal_transfer_position: usize,
        reward_transfer_position: usize,
    },
    VoteLock {
        position: CoreSpaceExecutionPosition,
        account: Address,
        required_locked_amount: U256,
        unlock_block_number: u64,
    },
}

impl StakingCall {
    const fn position(self) -> CoreSpaceExecutionPosition {
        match self {
            Self::Deposit { position, .. }
            | Self::Withdrawal { position, .. }
            | Self::VoteLock { position, .. } => position,
        }
    }

    fn transfer_positions(self) -> impl Iterator<Item = usize> {
        let positions = match self {
            Self::Deposit {
                transfer_position, ..
            } => [Some(transfer_position), None],
            Self::Withdrawal {
                principal_transfer_position,
                reward_transfer_position,
                ..
            } => [
                Some(principal_transfer_position),
                Some(reward_transfer_position),
            ],
            Self::VoteLock { .. } => [None, None],
        };
        positions.into_iter().flatten()
    }
}

#[derive(Debug, Clone, Copy)]
enum DecodedStakingCall {
    Deposit {
        amount: U256,
    },
    Withdrawal {
        principal_amount: U256,
    },
    VoteLock {
        required_locked_amount: U256,
        unlock_block_number: u64,
    },
}

fn collect_staking_calls(
    execution: &CoreSpaceExecutedTransaction,
) -> Result<Vec<StakingCall>, CoreSpaceChangesError> {
    let staking_contract =
        cfx_parameters::internal_contract_addresses::STORAGE_INTEREST_STAKING_CONTRACT_ADDRESS;
    if !execution.is_active_internal_contract(staking_contract) {
        return Ok(Vec::new());
    }

    let trace = execution.trace();
    let mut calls = Vec::new();
    for event in trace.events() {
        let TraceEvent::FrameStart { position, frame_id } = event else {
            continue;
        };
        let frame = trace.frame(*frame_id);
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
        if *target != staking_contract && *code_address != staking_contract {
            continue;
        }
        let Some(decoded) = decode_staking_call(calldata)? else {
            continue;
        };
        if frame.space != Space::Native
            || *call_type != CallType::Call
            || *target != staking_contract
            || *code_address != staking_contract
            || !transferred_value.is_zero()
        {
            return Err(CoreSpaceChangesError::unsupported_operation(
                "Core Space staking call did not use the canonical native plain-call form",
            ));
        }
        calls.push(collect_staking_call(
            trace,
            CoreSpaceExecutionPosition::from_index(*position),
            *frame_id,
            *caller,
            decoded,
        )?);
    }
    Ok(calls)
}

fn decode_staking_call(
    calldata: &[u8],
) -> Result<Option<DecodedStakingCall>, CoreSpaceChangesError> {
    let Some(selector) = calldata.get(..4) else {
        return Ok(None);
    };
    if selector == STAKING_DEPOSIT_SELECTOR {
        return Ok(Some(DecodedStakingCall::Deposit {
            amount: U256::from_be_bytes(read_word(calldata, 4, "staking deposit amount")?),
        }));
    }
    if selector == STAKING_WITHDRAW_SELECTOR {
        return Ok(Some(DecodedStakingCall::Withdrawal {
            principal_amount: U256::from_be_bytes(read_word(
                calldata,
                4,
                "staking withdrawal principal amount",
            )?),
        }));
    }
    if selector == VOTE_LOCK_SELECTOR {
        let required_locked_amount =
            U256::from_be_bytes(read_word(calldata, 4, "voteLock required locked amount")?);
        let unlock_word = read_word(calldata, 36, "voteLock unlock block number")?;
        return Ok(Some(DecodedStakingCall::VoteLock {
            required_locked_amount,
            // The locked upstream implementation deliberately applies
            // U256::low_u64 to this ABI word.
            unlock_block_number: u64::from_be_bytes(
                unlock_word[24..]
                    .try_into()
                    .expect("voteLock word tail has eight bytes"),
            ),
        }));
    }
    Ok(None)
}

fn read_word(
    calldata: &[u8],
    offset: usize,
    field: &str,
) -> Result<[u8; 32], CoreSpaceChangesError> {
    let end = offset + 32;
    let Some(word) = calldata.get(offset..end) else {
        return Err(CoreSpaceChangesError::inconsistent_execution(format!(
            "Core Space {field} was missing from committed call data"
        )));
    };
    Ok(word
        .try_into()
        .expect("a checked 32-byte calldata slice has the expected length"))
}

fn collect_staking_call(
    trace: &CommittedExecutionTrace,
    position: CoreSpaceExecutionPosition,
    frame_id: FrameId,
    caller: Address,
    call: DecodedStakingCall,
) -> Result<StakingCall, CoreSpaceChangesError> {
    let frame_transfers = trace
        .internal_transfers_in_scope(Some(frame_id))
        .collect::<Vec<_>>();
    match call {
        DecodedStakingCall::Deposit { amount } => {
            let [transfer] = frame_transfers.as_slice() else {
                return Err(transfer_count_mismatch("deposit", 1, frame_transfers.len()));
            };
            let TraceEvent::InternalTransfer {
                position: transfer_position,
                space: Space::Native,
                from: AddressPocket::Balance(from),
                to: AddressPocket::StakingBalance(to),
                value,
                ..
            } = transfer
            else {
                return Err(invalid_staking_transfer("deposit"));
            };
            if from.space != Space::Native
                || from.address != caller
                || *to != caller
                || u256_from_cfx(*value) != amount
            {
                return Err(invalid_staking_transfer("deposit"));
            }
            Ok(StakingCall::Deposit {
                position,
                account: caller,
                amount,
                transfer_position: *transfer_position,
            })
        }
        DecodedStakingCall::Withdrawal { principal_amount } => {
            let [principal_transfer, reward_transfer] = frame_transfers.as_slice() else {
                return Err(transfer_count_mismatch(
                    "withdrawal",
                    2,
                    frame_transfers.len(),
                ));
            };
            let TraceEvent::InternalTransfer {
                position: principal_transfer_position,
                space: Space::Native,
                from: AddressPocket::StakingBalance(from),
                to: AddressPocket::Balance(to),
                value: principal_value,
                ..
            } = principal_transfer
            else {
                return Err(invalid_staking_transfer("withdrawal principal"));
            };
            let TraceEvent::InternalTransfer {
                position: reward_transfer_position,
                space: Space::Native,
                from: AddressPocket::MintBurn,
                to: AddressPocket::Balance(reward_recipient),
                value: reward_value,
                ..
            } = reward_transfer
            else {
                return Err(invalid_staking_transfer("withdrawal reward"));
            };
            if *from != caller
                || to.space != Space::Native
                || to.address != caller
                || reward_recipient.space != Space::Native
                || reward_recipient.address != caller
                || u256_from_cfx(*principal_value) != principal_amount
            {
                return Err(invalid_staking_transfer("withdrawal"));
            }
            Ok(StakingCall::Withdrawal {
                position,
                account: caller,
                principal_amount,
                reward_amount: u256_from_cfx(*reward_value),
                principal_transfer_position: *principal_transfer_position,
                reward_transfer_position: *reward_transfer_position,
            })
        }
        DecodedStakingCall::VoteLock {
            required_locked_amount,
            unlock_block_number,
        } => {
            if !frame_transfers.is_empty() {
                return Err(transfer_count_mismatch(
                    "voteLock",
                    0,
                    frame_transfers.len(),
                ));
            }
            Ok(StakingCall::VoteLock {
                position,
                account: caller,
                required_locked_amount,
                unlock_block_number,
            })
        }
    }
}

fn transfer_count_mismatch(
    operation: &str,
    expected: usize,
    actual: usize,
) -> CoreSpaceChangesError {
    CoreSpaceChangesError::inconsistent_execution(format!(
        "Core Space staking {operation} expected {expected} internal transfers in its frame, got {actual}"
    ))
}

fn invalid_staking_transfer(operation: &str) -> CoreSpaceChangesError {
    CoreSpaceChangesError::inconsistent_execution(format!(
        "Core Space staking {operation} did not use the canonical caller, amount, and pocket movement"
    ))
}

#[derive(Debug, Clone, Copy)]
enum NativeOperation {
    Transfer {
        position: CoreSpaceExecutionPosition,
        from: Address,
        to: Address,
        amount: U256,
    },
    Burn {
        position: CoreSpaceExecutionPosition,
        account: Address,
        amount: U256,
    },
    CrossSpaceToEspace {
        position: CoreSpaceExecutionPosition,
        core_sender: Address,
        receiver: alloy_primitives::Address,
        amount: U256,
    },
    CrossSpaceToCore {
        position: CoreSpaceExecutionPosition,
        mapped_sender: Address,
        core_receiver: Address,
        amount: U256,
    },
}

impl NativeOperation {
    fn from_cross_space(transfer: CommittedCrossSpaceTransfer) -> Self {
        match transfer {
            CommittedCrossSpaceTransfer::ToEspace {
                position,
                core_sender,
                receiver,
                amount,
                ..
            } => Self::CrossSpaceToEspace {
                position: CoreSpaceExecutionPosition::from_index(position),
                core_sender,
                receiver,
                amount,
            },
            CommittedCrossSpaceTransfer::ToCoreSpace {
                position,
                mapped_sender,
                core_receiver,
                amount,
                ..
            } => Self::CrossSpaceToCore {
                position: CoreSpaceExecutionPosition::from_index(position),
                mapped_sender,
                core_receiver,
                amount,
            },
        }
    }
}

fn collect_native_operations(
    execution: &CoreSpaceExecutedTransaction,
    staking_calls: &[StakingCall],
) -> Result<Vec<NativeOperation>, CoreSpaceChangesError> {
    let trace = execution.trace();
    let claimed_positions: BTreeSet<_> = staking_calls
        .iter()
        .flat_map(|call| call.transfer_positions())
        .collect();
    let staking_positions: BTreeSet<_> = staking_calls
        .iter()
        .map(|call| call.position().index())
        .collect();

    let mut operations = Vec::new();
    for event in trace.events() {
        if claimed_positions.contains(&event.position()) {
            continue;
        }
        match event {
            TraceEvent::FrameStart { position, frame_id } => {
                if staking_positions.contains(position) {
                    continue;
                }
                let cross_space_transfer =
                    execution
                        .cross_space_transfers()
                        .iter()
                        .find(|transfer| match transfer {
                            CommittedCrossSpaceTransfer::ToEspace {
                                parent_frame_id, ..
                            }
                            | CommittedCrossSpaceTransfer::ToCoreSpace {
                                parent_frame_id, ..
                            } => *parent_frame_id == *frame_id,
                        });
                if let Some(transfer) = cross_space_transfer {
                    operations.push(NativeOperation::from_cross_space(*transfer));
                    continue;
                }
                if execution.is_cross_space_scope_parent(*frame_id) {
                    continue;
                }
                collect_frame_value_transfer(
                    execution,
                    trace,
                    *position,
                    *frame_id,
                    &mut operations,
                )?;
            }
            TraceEvent::InternalTransfer {
                position,
                frame_id,
                space,
                from,
                to,
                value,
            } => {
                if frame_id.is_none()
                    && *space == Space::Native
                    && matches!(
                        (from, to),
                        (AddressPocket::Balance(account), AddressPocket::MintBurn)
                            if account.space == Space::Ethereum
                    )
                {
                    if execution.nested_espace_scope_roots().is_empty() {
                        return Err(CoreSpaceChangesError::inconsistent_execution(
                            "committed eSpace selfdestruct burn had no verified Core cross-space scope",
                        ));
                    }
                    continue;
                }
                if frame_id.is_some_and(|id| execution.is_cross_space_parent(id)) {
                    continue;
                }
                if frame_id.is_some_and(|id| execution.is_cross_space_scope_parent(id)) {
                    continue;
                }
                collect_internal_transfer(
                    *position,
                    *space,
                    *from,
                    *to,
                    u256_from_cfx(*value),
                    &mut operations,
                )?
            }
            TraceEvent::Log { .. } | TraceEvent::StorageWrite { .. } => {}
        }
    }
    Ok(operations)
}

fn collect_frame_value_transfer(
    execution: &CoreSpaceExecutedTransaction,
    trace: &CommittedExecutionTrace,
    position: usize,
    frame_id: FrameId,
    operations: &mut Vec<NativeOperation>,
) -> Result<(), CoreSpaceChangesError> {
    let frame = trace.frame(frame_id);
    if frame.space != Space::Native {
        return Ok(());
    }
    let (from, to, amount) = match &frame.action {
        FrameAction::Call {
            call_type,
            caller,
            target,
            code_address,
            transferred_value,
            ..
        } => {
            let amount = u256_from_cfx(*transferred_value);
            if amount.is_zero() {
                return Ok(());
            }
            if *call_type != CallType::Call {
                return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                    "nonzero Core Space {call_type:?} value is not an ordinary CFX transfer"
                )));
            }
            if execution.is_active_internal_contract(*code_address) {
                return Err(CoreSpaceChangesError::unsupported_operation(format!(
                    "value movement through Core Space internal contract {code_address:?} is outside the current change rules"
                )));
            }
            (*caller, *target, amount)
        }
        FrameAction::Create {
            creator,
            actual_created_address,
            value,
            ..
        } => {
            let amount = u256_from_cfx(*value);
            if amount.is_zero() {
                return Ok(());
            }
            let created = actual_created_address
                .expect("finalized execution validates committed create addresses");
            (*creator, created, amount)
        }
    };
    operations.push(NativeOperation::Transfer {
        position: CoreSpaceExecutionPosition::from_index(position),
        from,
        to,
        amount,
    });
    Ok(())
}

fn collect_internal_transfer(
    position: usize,
    space: Space,
    from: AddressPocket,
    to: AddressPocket,
    amount: U256,
    operations: &mut Vec<NativeOperation>,
) -> Result<(), CoreSpaceChangesError> {
    if amount.is_zero() {
        return Ok(());
    }
    if space != Space::Native {
        return Ok(());
    }
    let operation = match (from, to) {
        (AddressPocket::Balance(from), AddressPocket::Balance(to))
            if from.space == Space::Native && to.space == Space::Native =>
        {
            NativeOperation::Transfer {
                position: CoreSpaceExecutionPosition::from_index(position),
                from: from.address,
                to: to.address,
                amount,
            }
        }
        (AddressPocket::GasPayment, _) | (_, AddressPocket::GasPayment) => return Ok(()),
        (AddressPocket::Balance(account), AddressPocket::MintBurn)
            if account.space == Space::Native =>
        {
            NativeOperation::Burn {
                position: CoreSpaceExecutionPosition::from_index(position),
                account: account.address,
                amount,
            }
        }
        (AddressPocket::Balance(_), AddressPocket::StakingBalance(_))
        | (AddressPocket::StakingBalance(_), AddressPocket::Balance(_)) => {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "Core Space staking movement was not owned by a canonical staking call",
            ));
        }
        (AddressPocket::MintBurn, AddressPocket::Balance(_)) => {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "Core Space issuance was not owned by a canonical staking withdrawal",
            ));
        }
        (from, to) => {
            return Err(CoreSpaceChangesError::unsupported_operation(format!(
                "Core Space native rules do not support {} ({}) -> {} ({}) pocket movement",
                from.pocket(),
                from.space(),
                to.pocket(),
                to.space()
            )));
        }
    };
    operations.push(operation);
    Ok(())
}

fn state_error(phase: &'static str, source: CoreSpaceStateAccessError) -> CoreSpaceChangesError {
    CoreSpaceChangesError::state_access(phase, source)
}

fn core_address(address: Address, execution: &CoreSpaceExecutedTransaction) -> CoreAddress {
    CoreAddress::from_bytes(address.0, execution.address_network)
        .expect("executed Core Space addresses retain a validated network")
}

fn derive_staking_changes(
    execution: &CoreSpaceExecutedTransaction,
    state: &CoreSpaceStateAccess,
    calls: &[StakingCall],
    builder: &mut CoreSpaceChangeSetBuilder,
) -> Result<(), CoreSpaceChangesError> {
    let mut vote_lists = BTreeMap::<Address, VoteStakeList>::new();
    for call in calls {
        match *call {
            StakingCall::Deposit {
                position,
                account,
                amount,
                ..
            } => {
                if !amount.is_zero() {
                    builder
                        .staking_deposit(position, core_address(account, execution), amount)
                        .expect("built-in Core Space changes use unique execution positions");
                }
            }
            StakingCall::Withdrawal {
                position,
                account,
                principal_amount,
                reward_amount,
                ..
            } => {
                if !principal_amount.is_zero() || !reward_amount.is_zero() {
                    builder
                        .staking_withdrawal(
                            position,
                            core_address(account, execution),
                            principal_amount,
                            reward_amount,
                        )
                        .expect("built-in Core Space changes use unique execution positions");
                }
            }
            StakingCall::VoteLock {
                position,
                account,
                required_locked_amount,
                unlock_block_number,
            } => {
                if required_locked_amount.is_zero() {
                    continue;
                }
                let vote_list = match vote_lists.entry(account) {
                    Entry::Occupied(entry) => entry.into_mut(),
                    Entry::Vacant(entry) => {
                        entry.insert(VoteStakeList(state.initial_vote_list(account).map_err(
                            |source| state_error("read initial Core Space vote list", source),
                        )?))
                    }
                };
                vote_list.remove_expired_vote_stake_info(execution.execution_block_number);
                let before = vote_list.clone();
                vote_list.vote_lock(u256_to_cfx(required_locked_amount), unlock_block_number);
                if *vote_list != before {
                    builder
                        .staking_vote_lock(
                            position,
                            core_address(account, execution),
                            required_locked_amount,
                            unlock_block_number,
                        )
                        .expect("built-in Core Space changes use unique execution positions");
                }
            }
        }
    }
    Ok(())
}
