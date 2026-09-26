//! Canonical WETH9 (Solidity 0.4.19). Runtime and source were matched at
//! https://sourcify.dev/server/v2/contract/1/0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2
//!
//! This implementation has balance/allowance mappings at slots 3/4, no proxy,
//! no mint/burn Transfer convention, and no Approval on allowance consumption.
//! Every committed write is checked, including same-value and restored writes.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use alloy_primitives::{Address, B256, U256, b256, keccak256};

use super::{
    AnalysisError, VerifiedChange, WrappedOperation,
    error::validation_error_at,
    events::decode_wrapped_log,
    state_queries::{read_allowance, read_erc20_balance, read_erc20_total_supply},
    view::{CallKind, ContractState, Frame, FrameAction, LogCheckpoint, StorageWrite, TokenView},
};
use crate::{DecodedStandardEvent, StandardChange, decode_standard_log};

pub const CODE_HASH: B256 =
    b256!("d0a06b12ac47863b5c7be4185c2deaad1c61557033f56c7d4ea74429cbb25e23");

/// Verifies the complete scoped implementation. Deployment matching belongs to the caller.
pub fn analyze(
    view: &dyn TokenView,
    contract: Address,
) -> Result<Vec<VerifiedChange>, AnalysisError> {
    for state in [view.initial(), view.finalized()] {
        if keccak256(state.code(contract)?) != CODE_HASH {
            return Err(AnalysisError::unsupported(
                "WETH9 runtime code differs from the reviewed implementation",
            ));
        }
        if read_erc20_total_supply(state, contract)? != state.native_balance(contract)? {
            return Err(AnalysisError::Validation {
                position: None,
                details: "WETH9 totalSupply differs from its native balance".into(),
            });
        }
    }

    let mut evidence = Vec::new();
    for frame in view
        .committed_frames()
        .filter(|frame| view.is_in_scope(frame.position))
    {
        match frame.action {
            FrameAction::Call { target, .. } if target == contract => {
                evidence.push((frame.position, Evidence::Call(frame)))
            }
            FrameAction::Create { address, .. } if address == contract => {
                return Err(AnalysisError::unsupported(
                    "WETH9 creation is outside the deployed-runtime rule",
                ));
            }
            _ => {}
        }
    }
    for write in view
        .storage_writes()
        .filter(|write| write.address == contract)
    {
        evidence.push((write.position, Evidence::Write(write)));
    }
    let checkpoints: HashMap<_, _> = view
        .log_checkpoints()
        .filter(|log| log.log.address == contract)
        .map(|log| (log.position, log))
        .collect();
    for log in view
        .committed_logs()
        .filter(|log| log.log.address == contract)
    {
        let checkpoint =
            checkpoints
                .get(&log.position)
                .ok_or_else(|| AnalysisError::IncompleteEvidence {
                    details: "WETH9 log has no retained state point".into(),
                })?;
        evidence.push((log.position, Evidence::Log(checkpoint)));
    }
    evidence.sort_by_key(|(position, _)| *position);

    let mut ledger = Ledger {
        initial: view.initial(),
        contract,
        values: BTreeMap::new(),
    };
    let mut plans = HashMap::<usize, CallPlan>::new();
    let mut changes = Vec::new();
    for (position, evidence) in evidence {
        match evidence {
            Evidence::Call(frame) => {
                let FrameAction::Call {
                    kind,
                    caller,
                    target,
                    bytecode_address,
                    value,
                    input,
                } = frame.action
                else {
                    unreachable!()
                };
                if frame.code_hash != Some(CODE_HASH)
                    || target != bytecode_address
                    || !matches!(kind, CallKind::Call | CallKind::StaticCall)
                {
                    return Err(AnalysisError::unsupported(
                        "WETH9 requires direct execution of its reviewed runtime",
                    ));
                }
                let operation = Operation::from_call(caller, value, input);
                if let Operation::Withdraw { account, amount } = operation {
                    let transfers = view.committed_frames().filter(|child| child.parent == Some(frame.id)).filter(|child| {
                        matches!(child.action, FrameAction::Call { kind: CallKind::Call, caller, target, value, input, .. }
                            if caller == contract && target == account && value == amount && input.is_empty())
                    }).count();
                    if transfers != 1 {
                        return Err(validation_error_at(
                            position,
                            "WETH9 withdrawal lacks its committed native value call",
                        ));
                    }
                }
                plans.insert(frame.id, ledger.plan(operation, position)?);
            }
            Evidence::Write(write) => {
                let plan = plans.get_mut(&write.frame_id).ok_or_else(|| {
                    validation_error_at(position, "WETH9 write has no reviewed call")
                })?;
                let expected = plan.writes.pop_front().ok_or_else(|| {
                    AnalysisError::unsupported("WETH9 made an unexplained storage write")
                })?;
                if write.slot != Some(expected.key.slot()) || write.value != expected.after {
                    return Err(validation_error_at(
                        position,
                        "WETH9 storage write differs from the reviewed operation",
                    ));
                }
                ledger.values.insert(expected.key, expected.after);
            }
            Evidence::Log(log) => {
                let plan = plans.get_mut(&log.frame_id).ok_or_else(|| {
                    validation_error_at(position, "WETH9 log has no reviewed call")
                })?;
                if plan.event_seen || !plan.writes.is_empty() {
                    return Err(validation_error_at(
                        position,
                        "WETH9 event and writes do not form one operation",
                    ));
                }
                plan.verify_event(log, contract)?;
                for key in &plan.touched {
                    ledger.verify(log.states.current, *key, Some(position))?;
                }
                changes.extend(plan.changes(contract, position));
                plan.event_seen = true;
            }
        }
    }
    if plans.values().any(|plan| {
        !plan.writes.is_empty() || (!matches!(plan.operation, Operation::Read) && !plan.event_seen)
    }) {
        return Err(AnalysisError::IncompleteEvidence {
            details: "WETH9 call is missing its writes or event".into(),
        });
    }
    for key in ledger.values.keys() {
        ledger.verify(view.finalized(), *key, None)?;
    }
    Ok(changes)
}

enum Evidence<'a> {
    Call(Frame<'a>),
    Write(StorageWrite),
    Log(&'a LogCheckpoint<'a>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum StorageKey {
    Balance(Address),
    Allowance(Address, Address),
}

impl StorageKey {
    fn slot(self) -> B256 {
        match self {
            Self::Balance(account) => {
                super::support::address_mapping_slot(account, B256::from(U256::from(3)))
            }
            Self::Allowance(owner, spender) => super::support::address_mapping_slot(
                spender,
                super::support::address_mapping_slot(owner, B256::from(U256::from(4))),
            ),
        }
    }
    fn read(self, state: &dyn ContractState, contract: Address) -> Result<U256, AnalysisError> {
        match self {
            Self::Balance(account) => read_erc20_balance(state, contract, account),
            Self::Allowance(owner, spender) => read_allowance(state, contract, owner, spender),
        }
    }
}

struct Ledger<'a> {
    initial: &'a dyn ContractState,
    contract: Address,
    values: BTreeMap<StorageKey, U256>,
}
struct ExpectedWrite {
    key: StorageKey,
    after: U256,
}
struct CallPlan {
    operation: Operation,
    writes: VecDeque<ExpectedWrite>,
    touched: BTreeSet<StorageKey>,
    allowance: Option<(Address, Address, U256, U256)>,
    event_seen: bool,
}

impl Ledger<'_> {
    fn value(&mut self, key: StorageKey, position: usize) -> Result<U256, AnalysisError> {
        if let Some(value) = self.values.get(&key) {
            return Ok(*value);
        }
        let value = key.read(self.initial, self.contract)?;
        if self.initial.storage(self.contract, key.slot())? != B256::from(value) {
            return Err(validation_error_at(
                position,
                "WETH9 getter does not agree with the reviewed storage layout",
            ));
        }
        self.values.insert(key, value);
        Ok(value)
    }

    fn plan(&mut self, operation: Operation, position: usize) -> Result<CallPlan, AnalysisError> {
        let mut plan = CallPlan {
            operation,
            writes: VecDeque::new(),
            touched: BTreeSet::new(),
            allowance: None,
            event_seen: false,
        };
        let mut local = BTreeMap::new();
        let mut write = |plan: &mut CallPlan,
                         key: StorageKey,
                         update: &dyn Fn(U256) -> Option<U256>|
         -> Result<(U256, U256), AnalysisError> {
            let before = match local.get(&key) {
                Some(value) => *value,
                None => self.value(key, position)?,
            };
            let after = update(before).ok_or_else(|| {
                validation_error_at(
                    position,
                    "WETH9 operation exceeds the verified balance or allowance",
                )
            })?;
            local.insert(key, after);
            plan.touched.insert(key);
            plan.writes.push_back(ExpectedWrite { key, after });
            Ok((before, after))
        };
        match operation {
            Operation::Read => {}
            Operation::Deposit { account, amount } => {
                write(&mut plan, StorageKey::Balance(account), &|value| {
                    value.checked_add(amount)
                })?;
            }
            Operation::Withdraw { account, amount } => {
                write(&mut plan, StorageKey::Balance(account), &|value| {
                    value.checked_sub(amount)
                })?;
            }
            Operation::Approve {
                owner,
                spender,
                amount,
            } => {
                let (before, after) =
                    write(&mut plan, StorageKey::Allowance(owner, spender), &|_| {
                        Some(amount)
                    })?;
                plan.allowance = Some((owner, spender, before, after));
            }
            Operation::Transfer {
                caller,
                from,
                to,
                amount,
            } => {
                // The source checks balance before considering allowance.
                write(&mut plan, StorageKey::Balance(from), &|value| {
                    value.checked_sub(amount)
                })?;
                write(&mut plan, StorageKey::Balance(to), &|value| {
                    value.checked_add(amount)
                })?;
                if caller != from {
                    let before = self.value(StorageKey::Allowance(from, caller), position)?;
                    plan.touched.insert(StorageKey::Allowance(from, caller));
                    if before != U256::MAX {
                        let after = before.checked_sub(amount).ok_or_else(|| {
                            validation_error_at(position, "WETH9 transfer exceeds allowance")
                        })?;
                        plan.writes.push_front(ExpectedWrite {
                            key: StorageKey::Allowance(from, caller),
                            after,
                        });
                        if before != after {
                            plan.allowance = Some((from, caller, before, after));
                        }
                    }
                }
            }
        }
        Ok(plan)
    }

    fn verify(
        &self,
        state: &dyn ContractState,
        key: StorageKey,
        position: Option<usize>,
    ) -> Result<(), AnalysisError> {
        let expected = self.values[&key];
        if key.read(state, self.contract)? != expected
            || state.storage(self.contract, key.slot())? != B256::from(expected)
        {
            return Err(AnalysisError::Validation {
                position,
                details: "WETH9 state differs from the committed write sequence".into(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
enum Operation {
    Read,
    Deposit {
        account: Address,
        amount: U256,
    },
    Withdraw {
        account: Address,
        amount: U256,
    },
    Approve {
        owner: Address,
        spender: Address,
        amount: U256,
    },
    Transfer {
        caller: Address,
        from: Address,
        to: Address,
        amount: U256,
    },
}

impl Operation {
    fn from_call(caller: Address, value: U256, input: &[u8]) -> Self {
        let selector = input.get(..4).unwrap_or_default();
        // Solidity 0.4.19 uses zero-padded CALLDATALOAD and truncates address words.
        let word = |index: usize| {
            let offset = 4 + index * 32;
            let mut word = [0; 32];
            if let Some(bytes) = input.get(offset..) {
                let len = bytes.len().min(32);
                word[..len].copy_from_slice(&bytes[..len]);
            }
            B256::new(word)
        };
        match selector {
            [0x09, 0x5e, 0xa7, 0xb3] => Self::Approve {
                owner: caller,
                spender: Address::from_word(word(0)),
                amount: word(1).into(),
            },
            [0xa9, 0x05, 0x9c, 0xbb] => Self::Transfer {
                caller,
                from: caller,
                to: Address::from_word(word(0)),
                amount: word(1).into(),
            },
            [0x23, 0xb8, 0x72, 0xdd] => Self::Transfer {
                caller,
                from: Address::from_word(word(0)),
                to: Address::from_word(word(1)),
                amount: word(2).into(),
            },
            [0x2e, 0x1a, 0x7d, 0x4d] => Self::Withdraw {
                account: caller,
                amount: word(0).into(),
            },
            [0x06, 0xfd, 0xde, 0x03]
            | [0x18, 0x16, 0x0d, 0xdd]
            | [0x31, 0x3c, 0xe5, 0x67]
            | [0x70, 0xa0, 0x82, 0x31]
            | [0x95, 0xd8, 0x9b, 0x41]
            | [0xdd, 0x62, 0xed, 0x3e] => Self::Read,
            _ => Self::Deposit {
                account: caller,
                amount: value,
            },
        }
    }
}

impl CallPlan {
    fn verify_event(
        &self,
        checkpoint: &LogCheckpoint<'_>,
        contract: Address,
    ) -> Result<(), AnalysisError> {
        let log = &checkpoint.log;
        let matches = match self.operation {
            Operation::Deposit { account, amount } | Operation::Withdraw { account, amount } => {
                let direction = if matches!(self.operation, Operation::Deposit { .. }) {
                    WrappedOperation::Deposit
                } else {
                    WrappedOperation::Withdrawal
                };
                decode_wrapped_log(log)
                    .map_err(|error| validation_error_at(checkpoint.position, error))?
                    == (account, amount, direction)
            }
            Operation::Approve {
                owner,
                spender,
                amount,
            }
            | Operation::Transfer {
                from: owner,
                to: spender,
                amount,
                ..
            } => {
                let decoded =
                    decode_standard_log(contract, &log.topics, log.data, |address| address)
                        .map_err(|error| {
                            validation_error_at(checkpoint.position, error.to_string())
                        })?;
                let expected = if matches!(self.operation, Operation::Approve { .. }) {
                    DecodedStandardEvent::Erc20Approval {
                        token: contract,
                        owner,
                        spender,
                        value: amount,
                    }
                } else {
                    DecodedStandardEvent::Erc20Transfer {
                        token: contract,
                        from: owner,
                        to: spender,
                        amount,
                    }
                };
                decoded.is_some_and(|decoded| decoded.event() == &expected)
            }
            Operation::Read => false,
        };
        if !matches {
            return Err(validation_error_at(
                checkpoint.position,
                "WETH9 event differs from its call",
            ));
        }
        Ok(())
    }

    fn changes(&self, contract: Address, position: usize) -> Vec<VerifiedChange> {
        let mut changes = Vec::new();
        if let Some((owner, spender, before, after)) = self.allowance {
            changes.push(VerifiedChange::Standard {
                position,
                change: StandardChange::Erc20Approval {
                    contract_address: contract,
                    owner,
                    spender,
                    before,
                    after,
                },
            });
        }
        match self.operation {
            Operation::Transfer {
                from, to, amount, ..
            } => changes.push(VerifiedChange::Standard {
                position,
                change: StandardChange::Erc20Transfer {
                    contract_address: contract,
                    from,
                    to,
                    raw_amount: amount,
                },
            }),
            Operation::Deposit { account, amount } | Operation::Withdraw { account, amount } => {
                changes.push(VerifiedChange::Wrapped {
                    position,
                    contract,
                    account,
                    amount,
                    direction: if matches!(self.operation, Operation::Deposit { .. }) {
                        WrappedOperation::Deposit
                    } else {
                        WrappedOperation::Withdrawal
                    },
                })
            }
            Operation::Read | Operation::Approve { .. } => {}
        }
        changes
    }
}
