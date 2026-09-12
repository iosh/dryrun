use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use alloy_primitives::U256;
use cfx_executor::executive_observer::AddressPocket;
use cfx_types::{Address, Space};
use cfx_vm_types::CallType;
use conflux_provider::CoreAddress;
use primitives::{DepositInfo, DepositList, VoteStakeList};

use super::{CoreSpaceChangeSet, CoreSpaceChangeSetBuilder, CoreSpaceNativeCurrency};
use crate::{
    core_space::{
        CoreSpaceChangesError, CoreSpaceExecutedTransaction, CoreSpaceExecutionPosition,
        CoreSpaceStateAccess, CoreSpaceStateAccessError, state_access::CoreSpaceStateReader,
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
    let locations = required_balance_locations(&operations, &staking_calls);
    let before = read_state(state.initial(), &locations, "initial")?;
    let after = read_state(state.finalized(), &locations, "finalized")?;

    let mut builder = CoreSpaceChangeSetBuilder::new();
    replay_native_operations(
        execution,
        &operations,
        before,
        &after,
        currency,
        &mut builder,
    )?;
    verify_staking_changes(execution, state, &staking_calls, &after, &mut builder)?;
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

    const fn account(self) -> Address {
        match self {
            Self::Deposit { account, .. }
            | Self::Withdrawal { account, .. }
            | Self::VoteLock { account, .. } => account,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum BalanceLocation {
    CoreAccount(Address),
    Staking(Address),
    GasSponsor(Address),
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
    GasPrecharge {
        payer: BalanceLocation,
        amount: U256,
    },
    GasRefund {
        recipient: BalanceLocation,
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
}

fn collect_native_operations(
    execution: &CoreSpaceExecutedTransaction,
    staking_calls: &[StakingCall],
) -> Result<Vec<NativeOperation>, CoreSpaceChangesError> {
    let trace = execution.trace();
    let mut claimed_positions = BTreeSet::new();
    let mut staking_by_position = BTreeMap::new();
    for call in staking_calls {
        for position in call.transfer_positions() {
            claimed_positions.insert(position);
        }
        staking_by_position.insert(call.position().index(), *call);
    }

    let mut operations = Vec::new();
    for event in trace.events() {
        if claimed_positions.contains(&event.position()) {
            continue;
        }
        match event {
            TraceEvent::FrameStart { position, frame_id } => {
                if let Some(call) = staking_by_position.remove(position) {
                    match call {
                        StakingCall::Deposit {
                            account, amount, ..
                        } => operations.push(NativeOperation::StakingDeposit { account, amount }),
                        StakingCall::Withdrawal {
                            account,
                            principal_amount,
                            reward_amount,
                            ..
                        } => operations.push(NativeOperation::StakingWithdrawal {
                            account,
                            principal_amount,
                            reward_amount,
                        }),
                        StakingCall::VoteLock { .. } => {}
                    }
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
                space,
                from,
                to,
                value,
                ..
            } => collect_internal_transfer(
                *position,
                *space,
                *from,
                *to,
                u256_from_cfx(*value),
                &mut operations,
            )?,
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
        return Err(CoreSpaceChangesError::unsupported_operation(
            "nested eSpace execution is outside the current Core Space change rules",
        ));
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
        return Err(CoreSpaceChangesError::unsupported_operation(
            "eSpace balance movement is outside the current Core Space change rules",
        ));
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
        (AddressPocket::Balance(payer), AddressPocket::GasPayment)
            if payer.space == Space::Native =>
        {
            NativeOperation::GasPrecharge {
                payer: BalanceLocation::CoreAccount(payer.address),
                amount,
            }
        }
        (AddressPocket::SponsorBalanceForGas(contract), AddressPocket::GasPayment) => {
            NativeOperation::GasPrecharge {
                payer: BalanceLocation::GasSponsor(contract),
                amount,
            }
        }
        (AddressPocket::GasPayment, AddressPocket::Balance(recipient))
            if recipient.space == Space::Native =>
        {
            NativeOperation::GasRefund {
                recipient: BalanceLocation::CoreAccount(recipient.address),
                amount,
            }
        }
        (AddressPocket::GasPayment, AddressPocket::SponsorBalanceForGas(contract)) => {
            NativeOperation::GasRefund {
                recipient: BalanceLocation::GasSponsor(contract),
                amount,
            }
        }
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

fn required_balance_locations(
    operations: &[NativeOperation],
    staking_calls: &[StakingCall],
) -> BTreeSet<BalanceLocation> {
    let mut locations = BTreeSet::new();
    for operation in operations {
        match operation {
            NativeOperation::Transfer { from, to, .. } => {
                locations.insert(BalanceLocation::CoreAccount(*from));
                locations.insert(BalanceLocation::CoreAccount(*to));
            }
            NativeOperation::Burn { account, .. } => {
                locations.insert(BalanceLocation::CoreAccount(*account));
            }
            NativeOperation::GasPrecharge { payer, .. } => {
                locations.insert(*payer);
            }
            NativeOperation::GasRefund { recipient, .. } => {
                locations.insert(*recipient);
            }
            NativeOperation::StakingDeposit { account, .. }
            | NativeOperation::StakingWithdrawal { account, .. } => {
                locations.insert(BalanceLocation::CoreAccount(*account));
                locations.insert(BalanceLocation::Staking(*account));
            }
        }
    }
    for call in staking_calls {
        locations.insert(BalanceLocation::Staking(call.account()));
    }
    locations
}

#[derive(Debug, Clone)]
struct NativeState {
    balances: BTreeMap<BalanceLocation, U256>,
    total_issued: U256,
    total_staking: U256,
}

fn read_state(
    reader: &CoreSpaceStateReader,
    locations: &BTreeSet<BalanceLocation>,
    phase: &'static str,
) -> Result<NativeState, CoreSpaceChangesError> {
    let mut balances = BTreeMap::new();
    for location in locations {
        let value = match *location {
            BalanceLocation::CoreAccount(account) => reader.core_balance_raw(account),
            BalanceLocation::Staking(account) => reader.staking_balance_raw(account),
            BalanceLocation::GasSponsor(contract) => reader.gas_sponsor_balance_raw(contract),
        }
        .map_err(|source| state_error(phase, source))?;
        balances.insert(*location, value);
    }
    Ok(NativeState {
        balances,
        total_issued: reader
            .total_issued()
            .map_err(|source| state_error(phase, source))?,
        total_staking: reader
            .total_staking()
            .map_err(|source| state_error(phase, source))?,
    })
}

fn state_error(phase: &'static str, source: CoreSpaceStateAccessError) -> CoreSpaceChangesError {
    CoreSpaceChangesError::state_access(phase, source)
}

fn replay_native_operations(
    execution: &CoreSpaceExecutedTransaction,
    operations: &[NativeOperation],
    mut replay: NativeState,
    after: &NativeState,
    currency: &CoreSpaceNativeCurrency,
    builder: &mut CoreSpaceChangeSetBuilder,
) -> Result<(), CoreSpaceChangesError> {
    if execution
        .burnt_gas_fee
        .is_some_and(|burnt| burnt > execution.gas_fee)
    {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "Core Space burnt gas fee exceeds the total gas fee",
        ));
    }
    let expected_fee_payer = if execution.gas_sponsor_paid {
        BalanceLocation::GasSponsor(execution.transaction_recipient.ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(
                "Core Space contract creation unexpectedly reported sponsored gas",
            )
        })?)
    } else {
        BalanceLocation::CoreAccount(execution.sender)
    };
    let mut precharged_fee = U256::ZERO;
    let mut refunded_fee = U256::ZERO;

    for operation in operations {
        match *operation {
            NativeOperation::Transfer {
                position,
                from,
                to,
                amount,
            } => {
                replay.debit(BalanceLocation::CoreAccount(from), amount)?;
                replay.credit(BalanceLocation::CoreAccount(to), amount)?;
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
                replay.debit(BalanceLocation::CoreAccount(account), amount)?;
                replay.total_issued = replay.total_issued.checked_sub(amount).ok_or_else(|| {
                    CoreSpaceChangesError::inconsistent_execution(
                        "Core Space total issued underflowed while replaying a native burn",
                    )
                })?;
                builder
                    .native_burn(
                        position,
                        core_address(account, execution),
                        amount,
                        currency.clone(),
                    )
                    .expect("built-in Core Space changes use unique execution positions");
            }
            NativeOperation::GasPrecharge { payer, amount } => {
                verify_fee_location("precharge payer", payer, expected_fee_payer)?;
                precharged_fee = precharged_fee.checked_add(amount).ok_or_else(|| {
                    CoreSpaceChangesError::inconsistent_execution(
                        "Core Space gas precharge overflowed while deriving changes",
                    )
                })?;
                replay.debit(payer, amount)?;
            }
            NativeOperation::GasRefund { recipient, amount } => {
                verify_fee_location("refund recipient", recipient, expected_fee_payer)?;
                refunded_fee = refunded_fee.checked_add(amount).ok_or_else(|| {
                    CoreSpaceChangesError::inconsistent_execution(
                        "Core Space gas refund overflowed while deriving changes",
                    )
                })?;
                replay.credit(recipient, amount)?;
            }
            NativeOperation::StakingDeposit { account, amount } => {
                replay.debit(BalanceLocation::CoreAccount(account), amount)?;
                replay.credit(BalanceLocation::Staking(account), amount)?;
                replay.total_staking =
                    replay.total_staking.checked_add(amount).ok_or_else(|| {
                        CoreSpaceChangesError::inconsistent_execution(
                            "Core Space total staking overflowed while replaying a deposit",
                        )
                    })?;
            }
            NativeOperation::StakingWithdrawal {
                account,
                principal_amount,
                reward_amount,
            } => {
                replay.debit(BalanceLocation::Staking(account), principal_amount)?;
                let credit = principal_amount.checked_add(reward_amount).ok_or_else(|| {
                    CoreSpaceChangesError::inconsistent_execution(
                        "Core Space staking withdrawal credit overflowed",
                    )
                })?;
                replay.credit(BalanceLocation::CoreAccount(account), credit)?;
                replay.total_staking = replay
                    .total_staking
                    .checked_sub(principal_amount)
                    .ok_or_else(|| {
                        CoreSpaceChangesError::inconsistent_execution(
                            "Core Space total staking underflowed while replaying a withdrawal",
                        )
                    })?;
                replay.total_issued = replay.total_issued.checked_add(reward_amount).ok_or_else(
                    || {
                        CoreSpaceChangesError::inconsistent_execution(
                            "Core Space total issued overflowed while replaying staking interest",
                        )
                    },
                )?;
            }
        }
    }
    let settled_fee = precharged_fee.checked_sub(refunded_fee).ok_or_else(|| {
        CoreSpaceChangesError::inconsistent_execution(
            "Core Space gas refund exceeds the traced precharge",
        )
    })?;
    if settled_fee != execution.gas_fee {
        return Err(CoreSpaceChangesError::inconsistent_execution(format!(
            "Core Space gas settlement mismatch: traced {settled_fee}, execution {}",
            execution.gas_fee
        )));
    }
    if let Some(burnt_fee) = execution.burnt_gas_fee {
        replay.total_issued = replay.total_issued.checked_sub(burnt_fee).ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(
                "Core Space total issued underflowed while replaying the gas burn",
            )
        })?;
    }
    replay.verify(after)
}

impl NativeState {
    fn debit(
        &mut self,
        location: BalanceLocation,
        amount: U256,
    ) -> Result<(), CoreSpaceChangesError> {
        let balance = self
            .balances
            .get_mut(&location)
            .expect("native replay collects every debited balance");
        *balance = balance.checked_sub(amount).ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space balance underflowed at {location:?}"
            ))
        })?;
        Ok(())
    }

    fn credit(
        &mut self,
        location: BalanceLocation,
        amount: U256,
    ) -> Result<(), CoreSpaceChangesError> {
        let balance = self
            .balances
            .get_mut(&location)
            .expect("native replay collects every credited balance");
        *balance = balance.checked_add(amount).ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space balance overflowed at {location:?}"
            ))
        })?;
        Ok(())
    }

    fn verify(self, after: &Self) -> Result<(), CoreSpaceChangesError> {
        if self.balances != after.balances {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "replayed Core Space native and staking balances do not match finalized state",
            ));
        }
        if self.total_issued != after.total_issued {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                "replayed Core Space total issued {}, finalized {}",
                self.total_issued, after.total_issued
            )));
        }
        if self.total_staking != after.total_staking {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                "replayed Core Space total staking {}, finalized {}",
                self.total_staking, after.total_staking
            )));
        }
        Ok(())
    }
}

fn verify_fee_location(
    role: &str,
    actual: BalanceLocation,
    expected: BalanceLocation,
) -> Result<(), CoreSpaceChangesError> {
    if actual == expected {
        return Ok(());
    }
    Err(CoreSpaceChangesError::inconsistent_execution(format!(
        "Core Space gas {role} mismatch: observed {actual:?}, expected {expected:?}"
    )))
}

fn core_address(address: Address, execution: &CoreSpaceExecutedTransaction) -> CoreAddress {
    CoreAddress::from_bytes(address.0, execution.address_network)
        .expect("executed Core Space addresses retain a validated network")
}

fn verify_staking_changes(
    execution: &CoreSpaceExecutedTransaction,
    state: &CoreSpaceStateAccess,
    calls: &[StakingCall],
    finalized_native_state: &NativeState,
    builder: &mut CoreSpaceChangeSetBuilder,
) -> Result<(), CoreSpaceChangesError> {
    verify_deposit_changes(execution, state, calls, finalized_native_state, builder)?;
    verify_vote_lock_changes(execution, state, calls, finalized_native_state, builder)
}

fn verify_deposit_changes(
    execution: &CoreSpaceExecutedTransaction,
    state: &CoreSpaceStateAccess,
    calls: &[StakingCall],
    finalized_native_state: &NativeState,
    builder: &mut CoreSpaceChangeSetBuilder,
) -> Result<(), CoreSpaceChangesError> {
    let mut replays = BTreeMap::<Address, StakingAccountReplay>::new();
    for call in calls {
        match *call {
            StakingCall::Deposit {
                position,
                account,
                amount,
                ..
            } => {
                if amount.is_zero() {
                    continue;
                }
                let replay = deposit_replay(&mut replays, account, state, execution.cip97)?;
                replay.deposit(
                    amount,
                    state.raw_accumulated_interest_rate(),
                    execution.execution_block_number,
                    execution.cip97,
                )?;
                builder
                    .staking_deposit(position, core_address(account, execution), amount)
                    .expect("built-in Core Space changes use unique execution positions");
            }
            StakingCall::Withdrawal {
                position,
                account,
                principal_amount,
                reward_amount,
                ..
            } => {
                if principal_amount.is_zero() {
                    if !reward_amount.is_zero() {
                        return Err(CoreSpaceChangesError::inconsistent_execution(
                            "zero-principal Core Space staking withdrawal issued a reward",
                        ));
                    }
                    continue;
                }
                let replay = deposit_replay(&mut replays, account, state, execution.cip97)?;
                let replayed_reward = replay.withdraw(
                    principal_amount,
                    state.raw_accumulated_interest_rate(),
                    execution.cip97,
                )?;
                if replayed_reward != reward_amount {
                    return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                        "Core Space staking withdrawal reward mismatch: replayed {replayed_reward}, traced {reward_amount}"
                    )));
                }
                builder
                    .staking_withdrawal(
                        position,
                        core_address(account, execution),
                        principal_amount,
                        reward_amount,
                    )
                    .expect("built-in Core Space changes use unique execution positions");
            }
            StakingCall::VoteLock { .. } => {}
        }
    }

    for (account, replay) in replays {
        verify_deposit_list_consistency(
            &replay.deposit_list,
            replay.staking_balance,
            account,
            execution.cip97,
        )?;
        let finalized_staking = finalized_native_state
            .balances
            .get(&BalanceLocation::Staking(account))
            .copied()
            .expect("staking calls include their finalized staking balance");
        if replay.staking_balance != finalized_staking {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "replayed staking balance does not match finalized state",
            ));
        }
        let final_length = state
            .finalized()
            .deposit_list_length_raw(account)
            .map_err(|source| state_error("verify finalized Core Space deposit list", source))?;
        if final_length != replay.deposit_list.len() {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                "replayed Core Space deposit-list length {}, finalized {final_length}",
                replay.deposit_list.len()
            )));
        }
    }
    Ok(())
}

fn deposit_replay<'a>(
    replays: &'a mut BTreeMap<Address, StakingAccountReplay>,
    account: Address,
    state: &CoreSpaceStateAccess,
    cip97: bool,
) -> Result<&'a mut StakingAccountReplay, CoreSpaceChangesError> {
    match replays.entry(account) {
        Entry::Occupied(entry) => Ok(entry.into_mut()),
        Entry::Vacant(entry) => {
            let deposit_list = state
                .initial_deposit_list(account)
                .map_err(|source| state_error("read initial Core Space deposit list", source))?;
            let staking_balance = state
                .initial()
                .staking_balance_raw(account)
                .map_err(|source| state_error("read initial Core Space staking balance", source))?;
            verify_deposit_list_consistency(&deposit_list, staking_balance, account, cip97)?;
            Ok(entry.insert(StakingAccountReplay {
                staking_balance,
                deposit_list: DepositList(deposit_list),
            }))
        }
    }
}

struct StakingAccountReplay {
    staking_balance: U256,
    deposit_list: DepositList,
}

impl StakingAccountReplay {
    fn deposit(
        &mut self,
        amount: U256,
        accumulated_interest_rate: cfx_types::U256,
        current_block_number: u64,
        cip97: bool,
    ) -> Result<(), CoreSpaceChangesError> {
        self.staking_balance = self.staking_balance.checked_add(amount).ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(
                "Core Space staking balance overflowed while replaying a deposit",
            )
        })?;
        if !(cip97 && self.deposit_list.is_empty()) {
            self.deposit_list.push(DepositInfo {
                amount: u256_to_cfx(amount),
                deposit_time: current_block_number.into(),
                accumulated_interest_rate,
            });
        }
        Ok(())
    }

    fn withdraw(
        &mut self,
        principal_amount: U256,
        accumulated_interest_rate: cfx_types::U256,
        cip97: bool,
    ) -> Result<U256, CoreSpaceChangesError> {
        let before_staking_balance = self.staking_balance;
        self.staking_balance = self
            .staking_balance
            .checked_sub(principal_amount)
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(
                    "Core Space staking balance underflowed while replaying a withdrawal",
                )
            })?;
        if self.deposit_list.is_empty() {
            return Ok(U256::ZERO);
        }
        let mut remaining_principal = if cip97 {
            before_staking_balance
        } else {
            principal_amount
        };
        let mut reward = U256::ZERO;
        let mut consumed_entries = 0;
        while !remaining_principal.is_zero() {
            let Some(deposit) = self.deposit_list.get_mut(consumed_entries) else {
                return Err(CoreSpaceChangesError::inconsistent_execution(
                    "Core Space deposit list did not cover a staking withdrawal",
                ));
            };
            let deposit_amount = u256_from_cfx(deposit.amount);
            let capital = deposit_amount.min(remaining_principal);
            let deposit_rate = u256_from_cfx(deposit.accumulated_interest_rate);
            if deposit_rate.is_zero()
                || accumulated_interest_rate < deposit.accumulated_interest_rate
            {
                return Err(CoreSpaceChangesError::inconsistent_execution(
                    "Core Space staking interest rates are inconsistent",
                ));
            }
            let entry_reward = capital
                .checked_mul(u256_from_cfx(accumulated_interest_rate))
                .and_then(|value| value.checked_div(deposit_rate))
                .and_then(|value| value.checked_sub(capital))
                .ok_or_else(|| {
                    CoreSpaceChangesError::inconsistent_execution(
                        "Core Space staking interest arithmetic failed",
                    )
                })?;
            reward = reward.checked_add(entry_reward).ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(
                    "Core Space staking interest overflowed",
                )
            })?;
            deposit.amount = u256_to_cfx(deposit_amount - capital);
            remaining_principal -= capital;
            if deposit.amount.is_zero() {
                consumed_entries += 1;
            }
        }
        if consumed_entries > 0 {
            self.deposit_list.0.drain(..consumed_entries);
        }
        Ok(reward)
    }
}

fn verify_deposit_list_consistency(
    deposit_list: &[DepositInfo],
    staking_balance: U256,
    account: Address,
    cip97: bool,
) -> Result<(), CoreSpaceChangesError> {
    if deposit_list
        .iter()
        .any(|deposit| deposit.accumulated_interest_rate.is_zero())
    {
        return Err(CoreSpaceChangesError::inconsistent_execution(format!(
            "Core Space deposit list contains a zero interest rate for {account:?}"
        )));
    }
    if !deposit_list.is_empty() || !cip97 {
        let listed = deposit_list
            .iter()
            .try_fold(U256::ZERO, |total, deposit| {
                total.checked_add(u256_from_cfx(deposit.amount))
            })
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(
                    "Core Space deposit-list principal overflowed",
                )
            })?;
        if listed != staking_balance {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space deposit-list principal {listed} does not match staking balance {staking_balance}"
            )));
        }
    }
    Ok(())
}

fn verify_vote_lock_changes(
    execution: &CoreSpaceExecutedTransaction,
    state: &CoreSpaceStateAccess,
    calls: &[StakingCall],
    finalized_native_state: &NativeState,
    builder: &mut CoreSpaceChangeSetBuilder,
) -> Result<(), CoreSpaceChangesError> {
    let mut vote_lists = BTreeMap::<Address, VoteStakeList>::new();
    let mut staking_balances = BTreeMap::<Address, U256>::new();

    for call in calls {
        let account = call.account();
        let staking_balance = match staking_balances.entry(account) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(
                state
                    .initial()
                    .staking_balance_raw(account)
                    .map_err(|source| {
                        state_error("read initial Core Space vote-lock staking balance", source)
                    })?,
            ),
        };
        match *call {
            StakingCall::Deposit { amount, .. } => {
                *staking_balance = staking_balance.checked_add(amount).ok_or_else(|| {
                    CoreSpaceChangesError::inconsistent_execution(
                        "Core Space staking balance overflowed before vote-lock verification",
                    )
                })?;
                continue;
            }
            StakingCall::Withdrawal { .. } | StakingCall::VoteLock { .. } => {}
        }

        let vote_list = match vote_lists.entry(account) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let list =
                    VoteStakeList(state.initial_vote_list(account).map_err(|source| {
                        state_error("read initial Core Space vote list", source)
                    })?);
                verify_vote_list_consistency(&list, account)?;
                entry.insert(list)
            }
        };
        vote_list.remove_expired_vote_stake_info(execution.execution_block_number);
        verify_vote_list_consistency(vote_list, account)?;
        let locked = vote_list
            .first()
            .map_or(U256::ZERO, |entry| u256_from_cfx(entry.amount));
        let withdrawable = staking_balance.checked_sub(locked).ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(
                "Core Space vote-lock amount exceeds staking balance",
            )
        })?;

        match *call {
            StakingCall::Withdrawal {
                principal_amount, ..
            } => {
                if withdrawable < principal_amount {
                    return Err(CoreSpaceChangesError::inconsistent_execution(
                        "Core Space staking withdrawal exceeds vote-lock withdrawable balance",
                    ));
                }
                *staking_balance = staking_balance.checked_sub(principal_amount).ok_or_else(
                    || {
                        CoreSpaceChangesError::inconsistent_execution(
                            "Core Space staking balance underflowed during vote-lock verification",
                        )
                    },
                )?;
            }
            StakingCall::VoteLock {
                position,
                required_locked_amount,
                unlock_block_number,
                ..
            } => {
                if required_locked_amount > *staking_balance {
                    return Err(CoreSpaceChangesError::inconsistent_execution(
                        "Core Space voteLock requirement exceeds staking balance",
                    ));
                }
                let before = vote_list.clone();
                if !required_locked_amount.is_zero() {
                    vote_list.vote_lock(u256_to_cfx(required_locked_amount), unlock_block_number);
                }
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
            StakingCall::Deposit { .. } => continue,
        }
    }

    for (account, vote_list) in vote_lists {
        verify_final_vote_list(state, account, &vote_list)?;
        let expected_staking = finalized_native_state
            .balances
            .get(&BalanceLocation::Staking(account))
            .copied()
            .expect("vote-lock calls include their finalized staking balance");
        let replayed_staking = staking_balances
            .get(&account)
            .copied()
            .expect("vote-list replay initializes its staking balance");
        if expected_staking != replayed_staking {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "vote-lock replay staking balance does not match finalized state",
            ));
        }
    }
    Ok(())
}

fn verify_final_vote_list(
    state: &CoreSpaceStateAccess,
    account: Address,
    vote_list: &VoteStakeList,
) -> Result<(), CoreSpaceChangesError> {
    verify_vote_list_consistency(vote_list, account)?;
    let final_length = state
        .finalized()
        .vote_list_length_raw(account)
        .map_err(|source| state_error("verify finalized Core Space vote list", source))?;
    if final_length != vote_list.len() {
        return Err(CoreSpaceChangesError::inconsistent_execution(format!(
            "replayed Core Space vote-list length {}, finalized {final_length}",
            vote_list.len()
        )));
    }
    for (index, vote) in vote_list.iter().enumerate() {
        let unlock_block = u64::try_from(vote.unlock_block_number).map_err(|_| {
            CoreSpaceChangesError::inconsistent_execution(
                "Core Space vote-list unlock block exceeds u64",
            )
        })?;
        let previous_block = unlock_block.checked_sub(1).ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(
                "Core Space vote-list unlock block is zero",
            )
        })?;
        let locked_before = state
            .finalized()
            .locked_staking_balance_at_raw(account, previous_block)
            .map_err(|source| {
                state_error("verify Core Space balance before vote unlock", source)
            })?;
        if locked_before != u256_from_cfx(vote.amount) {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "Core Space finalized vote-lock balance before unlock does not match replay",
            ));
        }
        let locked_at = state
            .finalized()
            .locked_staking_balance_at_raw(account, unlock_block)
            .map_err(|source| state_error("verify Core Space balance at vote unlock", source))?;
        let expected_at = vote_list
            .get(index + 1)
            .map_or(U256::ZERO, |next| u256_from_cfx(next.amount));
        if locked_at != expected_at {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "Core Space finalized vote-lock balance at unlock does not match replay",
            ));
        }
    }
    Ok(())
}

fn verify_vote_list_consistency(
    vote_list: &VoteStakeList,
    account: Address,
) -> Result<(), CoreSpaceChangesError> {
    for (earlier, later) in vote_list.iter().zip(vote_list.iter().skip(1)) {
        if earlier.unlock_block_number >= later.unlock_block_number
            || earlier.amount <= later.amount
        {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space vote list is not canonical for {account:?}"
            )));
        }
    }
    if vote_list.iter().any(|entry| entry.amount.is_zero()) {
        return Err(CoreSpaceChangesError::inconsistent_execution(format!(
            "Core Space vote list contains a zero amount for {account:?}"
        )));
    }
    Ok(())
}
