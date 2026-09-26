//! Dai.sol, Solidity 0.5.12, as verified at
//! https://sourcify.dev/server/v2/contract/1/0x6B175474E89094C44Da98b954EedeAC495271d0F
//!
//! Direct transfer, transferFrom, approve and getters use ordinary ERC-20
//! balance/allowance semantics. There are no external calls or configurable
//! transfer hooks. Slots 2/3 hold balances/allowances; every write and log must
//! belong to one supported call. Permit, issuance, aliases and ward updates
//! need additional rules. Transfers involving zero addresses are excluded
//! because this implementation does not give them mint/burn semantics.

use std::collections::{BTreeMap, HashMap, VecDeque};

use alloy_primitives::{Address, B256, U256, b256, keccak256};

use crate::{DecodedStandardEvent, decode_standard_log};

use super::{
    AnalysisError,
    call_evidence::{decode_call, selector},
    error::validation_error_at,
    support::{ReviewedStandardImplementation, address_mapping_slot},
    view::{CallKind, CommittedLog, Frame, FrameAction, StorageWrite, TokenView},
};

pub const CODE_HASH: B256 =
    b256!("4e36f96ee1667a663dfaac57c4d185a0e369a3a217e0079d49620f34f85d1ac7");

/// The reviewed runtime can be recognized in any supported network and Space.
pub struct Dai;

impl ReviewedStandardImplementation for Dai {
    fn code_hash(&self) -> B256 {
        CODE_HASH
    }

    fn verify_support(&self, view: &dyn TokenView, contract: Address) -> Result<(), AnalysisError> {
        if keccak256(view.finalized().code(contract)?) != CODE_HASH {
            return Err(AnalysisError::unsupported(
                "Dai runtime changed during execution",
            ));
        }
        let mut evidence = Vec::new();
        for frame in view
            .committed_frames()
            .filter(|frame| view.is_in_scope(frame.position))
        {
            match frame.action {
                FrameAction::Call { target, .. } if target == contract => {
                    evidence.push((frame.position, Evidence::Call(frame)));
                }
                FrameAction::Create { address, .. } if address == contract => {
                    return Err(AnalysisError::unsupported(
                        "Dai creation is outside the deployed-runtime rule",
                    ));
                }
                _ => {}
            }
        }
        for write in view.storage_writes() {
            evidence.push((write.position, Evidence::Write(write)));
        }
        for log in view.committed_logs() {
            evidence.push((log.position, Evidence::Log(log)));
        }
        evidence.sort_by_key(|(position, _)| *position);

        let mut plans = HashMap::<usize, CallPlan>::new();
        let mut values = BTreeMap::<B256, U256>::new();
        for (position, evidence) in evidence {
            match evidence {
                Evidence::Call(frame) => {
                    let FrameAction::Call {
                        kind,
                        caller,
                        target,
                        bytecode_address,
                        input,
                        ..
                    } = frame.action
                    else {
                        unreachable!()
                    };
                    if frame.code_hash != Some(CODE_HASH)
                        || target != bytecode_address
                        || !matches!(kind, CallKind::Call | CallKind::StaticCall)
                    {
                        return Err(AnalysisError::unsupported(
                            "Dai requires direct execution of the reviewed runtime",
                        ));
                    }
                    let operation = Operation::decode(caller, input)?;
                    let mut plan = CallPlan {
                        writes: VecDeque::new(),
                        event: None,
                    };
                    match operation {
                        Operation::Read => {}
                        Operation::Approve { spender, amount } => {
                            plan.writes.push_back(allowance_slot(caller, spender));
                            plan.event = Some(DecodedStandardEvent::Erc20Approval {
                                token: contract,
                                owner: caller,
                                spender,
                                value: amount,
                            });
                        }
                        Operation::Transfer { from, to, amount } => {
                            if from.is_zero() || to.is_zero() {
                                return Err(AnalysisError::unsupported(
                                    "Dai zero-address transfers are not mint or burn operations",
                                ));
                            }
                            if from != caller {
                                let slot = allowance_slot(from, caller);
                                let allowance = match values.get(&slot) {
                                    Some(value) => *value,
                                    None => U256::from_be_bytes(
                                        view.initial().storage(contract, slot)?.0,
                                    ),
                                };
                                if allowance != U256::MAX {
                                    plan.writes.push_back(slot);
                                }
                            }
                            plan.writes.push_back(balance_slot(from));
                            plan.writes.push_back(balance_slot(to));
                            plan.event = Some(DecodedStandardEvent::Erc20Transfer {
                                token: contract,
                                from,
                                to,
                                amount,
                            });
                        }
                    }
                    plans.insert(frame.id, plan);
                }
                Evidence::Write(write) => {
                    let plan = plans.get_mut(&write.frame_id).ok_or_else(|| {
                        validation_error_at(position, "Dai write has no supported call")
                    })?;
                    let slot = plan.writes.pop_front().ok_or_else(|| {
                        validation_error_at(position, "Dai made an unexplained storage write")
                    })?;
                    if write.slot != Some(slot) {
                        return Err(validation_error_at(
                            position,
                            "Dai write does not belong to the reviewed operation",
                        ));
                    }
                    values.insert(slot, write.value);
                }
                Evidence::Log(log) => {
                    let plan = plans.get_mut(&log.frame_id).ok_or_else(|| {
                        validation_error_at(position, "Dai log has no supported call")
                    })?;
                    let decoded =
                        decode_standard_log(contract, &log.log.topics, log.log.data, |address| {
                            address
                        })
                        .map_err(|error| AnalysisError::Validation {
                            position: Some(position),
                            details: error.to_string(),
                        })?;
                    let expected = plan.event.take().ok_or_else(|| {
                        validation_error_at(position, "Dai made an unexplained log")
                    })?;
                    if !plan.writes.is_empty()
                        || !decoded.is_some_and(|decoded| decoded.event() == &expected)
                    {
                        return Err(validation_error_at(
                            position,
                            "Dai call, writes and event do not describe one operation",
                        ));
                    }
                }
            }
        }
        if plans
            .values()
            .any(|plan| !plan.writes.is_empty() || plan.event.is_some())
        {
            return Err(AnalysisError::IncompleteEvidence {
                details: "Dai call is missing its writes or event".into(),
            });
        }
        for (slot, expected) in values {
            if view.finalized().storage(contract, slot)? != B256::from(expected) {
                return Err(AnalysisError::Validation {
                    position: None,
                    details: "Dai final storage differs from its committed write sequence".into(),
                });
            }
        }
        Ok(())
    }
}

enum Evidence<'a> {
    Call(Frame<'a>),
    Write(StorageWrite),
    Log(CommittedLog<'a>),
}
struct CallPlan {
    writes: VecDeque<B256>,
    event: Option<DecodedStandardEvent<Address>>,
}
enum Operation {
    Read,
    Approve {
        spender: Address,
        amount: U256,
    },
    Transfer {
        from: Address,
        to: Address,
        amount: U256,
    },
}

impl Operation {
    fn decode(caller: Address, input: &[u8]) -> Result<Self, AnalysisError> {
        if let Some((to, amount)) = decode_call(input, selector("transfer(address,uint256)")) {
            return Ok(Self::Transfer {
                from: caller,
                to,
                amount,
            });
        }
        if let Some((from, to, amount)) =
            decode_call(input, selector("transferFrom(address,address,uint256)"))
        {
            return Ok(Self::Transfer { from, to, amount });
        }
        if let Some((spender, amount)) = decode_call(input, selector("approve(address,uint256)")) {
            return Ok(Self::Approve { spender, amount });
        }
        if [
            "name()",
            "symbol()",
            "version()",
            "decimals()",
            "totalSupply()",
            "balanceOf(address)",
            "allowance(address,address)",
            "nonces(address)",
            "wards(address)",
            "DOMAIN_SEPARATOR()",
            "PERMIT_TYPEHASH()",
        ]
        .iter()
        .any(|signature| input.starts_with(&selector(signature)))
        {
            return Ok(Self::Read);
        }
        Err(AnalysisError::unsupported(
            "Dai call is outside the reviewed ERC-20 transfer and approval rules",
        ))
    }
}

fn balance_slot(account: Address) -> B256 {
    address_mapping_slot(account, B256::from(U256::from(2)))
}
fn allowance_slot(owner: Address, spender: Address) -> B256 {
    address_mapping_slot(
        spender,
        address_mapping_slot(owner, B256::from(U256::from(3))),
    )
}
