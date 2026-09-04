use std::collections::{HashMap, HashSet};

use alloy::{
    primitives::{Address, B256, Bytes, Log, U256, keccak256},
    sol_types::{SolCall, SolType, SolValue, abi::TokenSeq, sol},
};
use contract_standards::{
    DecodedStandardLog, Erc1155TransferItem, MetadataCall, MetadataValues, decode_standard_log,
    is_supported_event_topic, metadata_calls,
};
use thiserror::Error;

use crate::{
    EvmChangeResolutionError, EvmChangeResolver, EvmChangeSet, EvmChangeSetBuilder,
    EvmObservationRequirements, EvmStandardChange,
    changeset::{EvmWrappedNativeDepositChange, EvmWrappedNativeWithdrawalChange},
    execution::{
        EvmCommittedFrame, EvmExecutionPosition, EvmFrameAction, EvmFrameId,
        EvmSemanticLogOccurrence, EvmTransactionExecution,
    },
    state::{EvmReadCallOutcome, EvmStateAccess, EvmStateReader},
};

sol! {
    interface IERC20State {
        function balanceOf(address account) external view returns (uint256);
        function allowance(address owner, address spender) external view returns (uint256);
        function totalSupply() external view returns (uint256);
    }

    interface IERC721State {
        function ownerOf(uint256 tokenId) external view returns (address);
        function getApproved(uint256 tokenId) external view returns (address);
        function isApprovedForAll(address owner, address operator) external view returns (bool);
    }

    interface IERC1155State {
        function balanceOf(address account, uint256 id) external view returns (uint256);
    }
}

use std::sync::LazyLock;

static DEPOSIT_TOPIC0: LazyLock<B256> = LazyLock::new(|| keccak256("Deposit(address,uint256)"));
static WITHDRAWAL_TOPIC0: LazyLock<B256> =
    LazyLock::new(|| keccak256("Withdrawal(address,uint256)"));

#[derive(Debug, Clone, Copy)]
pub(crate) struct EvmStandardChangeResolver {
    wrapped_native_token: Option<Address>,
}

impl EvmStandardChangeResolver {
    pub(crate) const fn new(wrapped_native_token: Option<Address>) -> Self {
        Self {
            wrapped_native_token,
        }
    }

    pub(crate) fn resolve(
        &self,
        execution: &EvmTransactionExecution,
        state: &EvmStateAccess,
    ) -> Result<EvmChangeSet, EvmChangeResolutionError> {
        let collection = collect_candidates(execution, self.wrapped_native_token)?;
        let candidates = collection.candidates;
        if candidates.is_empty() {
            return Ok(EvmChangeSet::default());
        }

        let mut final_checks = HashMap::new();
        let mut wrapped_pair_evidence = HashMap::new();
        let mut verified = Vec::with_capacity(candidates.len());
        for (candidate_index, candidate) in candidates.iter().enumerate() {
            verified.push(verify_candidate(
                candidate_index,
                candidate,
                execution,
                state,
                &collection.pairs,
                &mut wrapped_pair_evidence,
                &mut final_checks,
            )?);
        }
        for (pair_index, pair) in collection.pairs.iter().enumerate() {
            if !wrapped_pair_evidence
                .get(&pair_index)
                .is_some_and(|evidence| evidence.proves_single_transition(pair.amount))
            {
                return Err(mismatch(
                    pair.position,
                    "wrapped-native Transfer and Deposit/Withdrawal did not prove one operation",
                ));
            }
        }
        verify_final_checks(&final_checks, state)?;

        let metadata = load_metadata(&candidates, state);
        let mut builder = EvmChangeSetBuilder::new();
        for (candidate, verified) in candidates.into_iter().zip(verified) {
            match (candidate, verified) {
                (Candidate::Standard { occurrence, .. }, VerifiedCandidate::Standard(change)) => {
                    let change = change.into_change(&metadata)?;
                    builder.standard(occurrence.position(), change)?;
                }
                (
                    Candidate::Wrapped {
                        occurrence,
                        contract,
                        account,
                        amount,
                        direction,
                    },
                    VerifiedCandidate::Wrapped,
                ) => {
                    let change_metadata = metadata
                        .erc20_metadata(&contract)
                        .map_err(|error| standard_error(error.to_string()))?;
                    match direction {
                        WrappedDirection::Deposit => builder.wrapped_native_deposit(
                            occurrence.position(),
                            EvmWrappedNativeDepositChange {
                                contract_address: contract,
                                account,
                                raw_amount: amount,
                                metadata: change_metadata,
                            },
                        )?,
                        WrappedDirection::Withdrawal => builder.wrapped_native_withdrawal(
                            occurrence.position(),
                            EvmWrappedNativeWithdrawalChange {
                                contract_address: contract,
                                account,
                                raw_amount: amount,
                                metadata: change_metadata,
                            },
                        )?,
                    }
                }
                _ => return Err(standard_error("candidate verification result mismatch")),
            }
        }

        Ok(builder.finish())
    }
}

impl EvmChangeResolver for EvmStandardChangeResolver {
    fn observation_requirements(&self) -> EvmObservationRequirements {
        let mut requirements = EvmObservationRequirements::new();
        for topic0 in contract_standards::supported_event_topics() {
            requirements.checkpoint_any_address(*topic0);
        }
        if let Some(address) = self.wrapped_native_token {
            requirements.checkpoint_at(address, *DEPOSIT_TOPIC0);
            requirements.checkpoint_at(address, *WITHDRAWAL_TOPIC0);
        }
        requirements
    }

    fn resolve(
        &self,
        execution: &EvmTransactionExecution,
        state: &EvmStateAccess,
    ) -> Result<EvmChangeSet, EvmChangeResolutionError> {
        Self::resolve(self, execution, state)
    }
}

#[derive(Debug)]
enum Candidate {
    Standard {
        occurrence: EvmSemanticLogOccurrence,
        decoded: DecodedStandardLog<Address>,
        kind: Box<StandardCandidate>,
    },
    Wrapped {
        occurrence: EvmSemanticLogOccurrence,
        contract: Address,
        account: Address,
        amount: U256,
        direction: WrappedDirection,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WrappedDirection {
    Deposit,
    Withdrawal,
}

#[derive(Debug)]
struct CandidateCollection {
    candidates: Vec<Candidate>,
    pairs: Vec<WrappedTransferPair>,
}

#[derive(Debug, Clone, Copy)]
struct WrappedTransferPair {
    standard_index: usize,
    wrapped_index: usize,
    position: EvmExecutionPosition,
    direction: WrappedDirection,
    amount: U256,
}

#[derive(Debug, Clone, Copy)]
enum WrappedTransitionEvidence {
    Exact,
    Unchanged,
}

#[derive(Debug, Default)]
struct WrappedPairEvidence {
    exact: u8,
    unchanged: u8,
}

impl WrappedPairEvidence {
    fn record(&mut self, evidence: WrappedTransitionEvidence) {
        match evidence {
            WrappedTransitionEvidence::Exact => self.exact += 1,
            WrappedTransitionEvidence::Unchanged => self.unchanged += 1,
        }
    }

    fn proves_single_transition(&self, amount: U256) -> bool {
        if amount == U256::ZERO {
            self.exact == 2 && self.unchanged == 0
        } else {
            self.exact == 1 && self.unchanged == 1
        }
    }
}

#[derive(Debug, Clone)]
enum StandardCandidate {
    Erc20Transfer {
        contract: Address,
        from: Address,
        to: Address,
        amount: U256,
    },
    Erc20Approval {
        contract: Address,
        owner: Address,
        spender: Address,
        amount: U256,
    },
    Erc721Transfer {
        contract: Address,
        from: Address,
        to: Address,
        token_id: U256,
    },
    Erc721Approval {
        contract: Address,
        owner: Address,
        approved: Option<Address>,
        token_id: U256,
    },
    OperatorApproval {
        contract: Address,
        owner: Address,
        operator: Address,
        approved: bool,
    },
    Erc1155TransferSingle {
        contract: Address,
        operator: Address,
        from: Address,
        to: Address,
        token_id: U256,
        amount: U256,
    },
    Erc1155TransferBatch {
        contract: Address,
        operator: Address,
        from: Address,
        to: Address,
        items: Vec<(U256, U256)>,
    },
}

#[derive(Debug)]
enum VerifiedCandidate {
    Standard(VerifiedStandardChange),
    Wrapped,
}

#[derive(Debug)]
enum VerifiedStandardChange {
    Erc20Transfer {
        contract: Address,
        from: Address,
        to: Address,
        amount: U256,
    },
    Erc20Approval {
        contract: Address,
        owner: Address,
        spender: Address,
        before: U256,
        after: U256,
    },
    Erc721Transfer {
        contract: Address,
        from: Address,
        to: Address,
        token_id: U256,
    },
    Erc721Approval {
        contract: Address,
        owner: Address,
        before: Option<Address>,
        after: Option<Address>,
        token_id: U256,
    },
    OperatorApproval {
        contract: Address,
        owner: Address,
        operator: Address,
        before: bool,
        after: bool,
    },
    Erc1155TransferSingle {
        contract: Address,
        operator: Address,
        from: Address,
        to: Address,
        token_id: U256,
        amount: U256,
    },
    Erc1155TransferBatch {
        contract: Address,
        operator: Address,
        from: Address,
        to: Address,
        items: Vec<(U256, U256)>,
    },
}

impl VerifiedStandardChange {
    fn into_change(
        self,
        metadata: &MetadataValues<Address>,
    ) -> Result<EvmStandardChange, EvmChangeResolutionError> {
        Ok(match self {
            Self::Erc20Transfer {
                contract,
                from,
                to,
                amount,
            } => {
                let metadata = metadata
                    .erc20_metadata(&contract)
                    .map_err(|error| standard_error(error.to_string()))?;
                if from == Address::ZERO {
                    EvmStandardChange::Erc20Mint {
                        contract_address: contract,
                        to,
                        raw_amount: amount,
                        metadata,
                    }
                } else if to == Address::ZERO {
                    EvmStandardChange::Erc20Burn {
                        contract_address: contract,
                        from,
                        raw_amount: amount,
                        metadata,
                    }
                } else {
                    EvmStandardChange::Erc20Transfer {
                        contract_address: contract,
                        from,
                        to,
                        raw_amount: amount,
                        metadata,
                    }
                }
            }
            Self::Erc20Approval {
                contract,
                owner,
                spender,
                before,
                after,
            } => EvmStandardChange::Erc20Approval {
                contract_address: contract,
                owner,
                spender,
                before,
                after,
                metadata: metadata
                    .erc20_metadata(&contract)
                    .map_err(|error| standard_error(error.to_string()))?,
            },
            Self::Erc721Transfer {
                contract,
                from,
                to,
                token_id,
            } => {
                let metadata = metadata
                    .erc721_collection_metadata(&contract)
                    .map_err(|error| standard_error(error.to_string()))?;
                if from == Address::ZERO {
                    EvmStandardChange::Erc721Mint {
                        contract_address: contract,
                        to,
                        token_id,
                        metadata,
                    }
                } else if to == Address::ZERO {
                    EvmStandardChange::Erc721Burn {
                        contract_address: contract,
                        from,
                        token_id,
                        metadata,
                    }
                } else {
                    EvmStandardChange::Erc721Transfer {
                        contract_address: contract,
                        from,
                        to,
                        token_id,
                        metadata,
                    }
                }
            }
            Self::Erc721Approval {
                contract,
                owner,
                before,
                after,
                token_id,
            } => EvmStandardChange::Erc721Approval {
                contract_address: contract,
                owner,
                before,
                after,
                token_id,
                metadata: metadata
                    .erc721_collection_metadata(&contract)
                    .map_err(|error| standard_error(error.to_string()))?,
            },
            Self::OperatorApproval {
                contract,
                owner,
                operator,
                before,
                after,
            } => EvmStandardChange::OperatorApproval {
                contract_address: contract,
                owner,
                operator,
                before,
                after,
            },
            Self::Erc1155TransferSingle {
                contract,
                operator,
                from,
                to,
                token_id,
                amount,
            } => {
                if from == Address::ZERO {
                    EvmStandardChange::Erc1155MintSingle {
                        contract_address: contract,
                        operator,
                        to,
                        token_id,
                        raw_amount: amount,
                    }
                } else if to == Address::ZERO {
                    EvmStandardChange::Erc1155BurnSingle {
                        contract_address: contract,
                        operator,
                        from,
                        token_id,
                        raw_amount: amount,
                    }
                } else {
                    EvmStandardChange::Erc1155TransferSingle {
                        contract_address: contract,
                        operator,
                        from,
                        to,
                        token_id,
                        raw_amount: amount,
                    }
                }
            }
            Self::Erc1155TransferBatch {
                contract,
                operator,
                from,
                to,
                items,
            } => {
                let items = items
                    .into_iter()
                    .map(|(token_id, raw_amount)| Erc1155TransferItem {
                        token_id,
                        raw_amount,
                    })
                    .collect();
                if from == Address::ZERO {
                    EvmStandardChange::Erc1155MintBatch {
                        contract_address: contract,
                        operator,
                        to,
                        items,
                    }
                } else if to == Address::ZERO {
                    EvmStandardChange::Erc1155BurnBatch {
                        contract_address: contract,
                        operator,
                        from,
                        items,
                    }
                } else {
                    EvmStandardChange::Erc1155TransferBatch {
                        contract_address: contract,
                        operator,
                        from,
                        to,
                        items,
                    }
                }
            }
        })
    }
}

fn collect_candidates(
    execution: &EvmTransactionExecution,
    wrapped_native_token: Option<Address>,
) -> Result<CandidateCollection, EvmChangeResolutionError> {
    let occurrences = execution.semantic_log_occurrences()?;
    let mut candidates = Vec::new();

    for occurrence in occurrences {
        let log = occurrence.log();
        let Some(topic0) = log.data.topics().first() else {
            continue;
        };

        if wrapped_native_token.is_some_and(|address| address == log.address)
            && (*topic0 == *DEPOSIT_TOPIC0 || *topic0 == *WITHDRAWAL_TOPIC0)
        {
            let (account, amount, direction) = decode_wrapped_log(log)
                .map_err(|error| standard_error_at(occurrence.position(), error))?;
            candidates.push(Candidate::Wrapped {
                occurrence: occurrence.clone(),
                contract: log.address,
                account,
                amount,
                direction,
            });
            continue;
        }

        if !is_supported_event_topic(topic0) {
            continue;
        }

        let kind = decode_standard_candidate(log)
            .map_err(|error| standard_error_at(occurrence.position(), error))?;
        let decoded =
            decode_standard_log(log.address, log.data.topics(), &log.data.data, |address| {
                address
            })
            .ok_or_else(|| {
                standard_error_at(
                    occurrence.position(),
                    "supported standard log could not be decoded",
                )
            })?;
        candidates.push(Candidate::Standard {
            occurrence: occurrence.clone(),
            decoded,
            kind: Box::new(kind),
        });
    }

    Ok(CandidateCollection {
        pairs: pair_wrapped_transfers(&candidates),
        candidates,
    })
}

fn pair_wrapped_transfers(candidates: &[Candidate]) -> Vec<WrappedTransferPair> {
    let mut used_wrapped = HashSet::new();
    let mut pairs = Vec::new();

    for (standard_index, candidate) in candidates.iter().enumerate() {
        let Candidate::Standard {
            occurrence, kind, ..
        } = candidate
        else {
            continue;
        };
        let StandardCandidate::Erc20Transfer {
            contract,
            from,
            to,
            amount,
        } = kind.as_ref()
        else {
            continue;
        };

        let (direction, account) = if *from == Address::ZERO && *to != Address::ZERO {
            (WrappedDirection::Deposit, *to)
        } else if *to == Address::ZERO && *from != Address::ZERO {
            (WrappedDirection::Withdrawal, *from)
        } else {
            continue;
        };

        let Some((wrapped_index, wrapped)) = candidates.iter().enumerate().find(|(index, item)| {
            if used_wrapped.contains(index) {
                return false;
            }
            let Candidate::Wrapped {
                occurrence: wrapped_occurrence,
                contract: wrapped_contract,
                account: wrapped_account,
                amount: wrapped_amount,
                direction: wrapped_direction,
            } = item
            else {
                return false;
            };
            occurrence.frame_id() == wrapped_occurrence.frame_id()
                && *wrapped_contract == *contract
                && *wrapped_account == account
                && *wrapped_amount == *amount
                && *wrapped_direction == direction
        }) else {
            continue;
        };

        used_wrapped.insert(wrapped_index);
        let Candidate::Wrapped { occurrence, .. } = wrapped else {
            unreachable!("pair search only returns wrapped candidates");
        };
        pairs.push(WrappedTransferPair {
            standard_index,
            wrapped_index,
            position: occurrence.position(),
            direction,
            amount: *amount,
        });
    }

    pairs
}

fn decode_standard_candidate(log: &Log) -> Result<StandardCandidate, &'static str> {
    let topics = log.data.topics();
    let data = log.data.data.as_ref();
    let Some(topic0) = topics.first() else {
        return Err("supported event has no signature topic");
    };

    if *topic0 == transfer_topic() {
        return match (topics.len(), data.len()) {
            (3, 32) => Ok(StandardCandidate::Erc20Transfer {
                contract: log.address,
                from: indexed_address(&topics[1])?,
                to: indexed_address(&topics[2])?,
                amount: U256::from_be_slice(data),
            }),
            (4, 0) => Ok(StandardCandidate::Erc721Transfer {
                contract: log.address,
                from: indexed_address(&topics[1])?,
                to: indexed_address(&topics[2])?,
                token_id: U256::from_be_slice(topics[3].as_slice()),
            }),
            _ => Err("malformed Transfer event"),
        };
    }

    if *topic0 == approval_topic() {
        return match (topics.len(), data.len()) {
            (3, 32) => Ok(StandardCandidate::Erc20Approval {
                contract: log.address,
                owner: indexed_address(&topics[1])?,
                spender: indexed_address(&topics[2])?,
                amount: U256::from_be_slice(data),
            }),
            (4, 0) => Ok(StandardCandidate::Erc721Approval {
                contract: log.address,
                owner: indexed_address(&topics[1])?,
                approved: nonzero_address(indexed_address(&topics[2])?),
                token_id: U256::from_be_slice(topics[3].as_slice()),
            }),
            _ => Err("malformed Approval event"),
        };
    }

    if *topic0 == approval_for_all_topic() {
        if topics.len() != 3 || data.len() != 32 {
            return Err("malformed ApprovalForAll event");
        }
        let approved = canonical_bool(data).ok_or("ApprovalForAll value is not canonical")?;
        return Ok(StandardCandidate::OperatorApproval {
            contract: log.address,
            owner: indexed_address(&topics[1])?,
            operator: indexed_address(&topics[2])?,
            approved,
        });
    }

    if *topic0 == transfer_single_topic() {
        if topics.len() != 4 || data.len() != 64 {
            return Err("malformed TransferSingle event");
        }
        let (token_id, amount) = <(U256, U256)>::abi_decode_sequence_validate(data)
            .map_err(|_| "TransferSingle data is not canonical")?;
        return Ok(StandardCandidate::Erc1155TransferSingle {
            contract: log.address,
            operator: indexed_address(&topics[1])?,
            from: indexed_address(&topics[2])?,
            to: indexed_address(&topics[3])?,
            token_id,
            amount,
        });
    }

    if *topic0 == transfer_batch_topic() {
        if topics.len() != 4 {
            return Err("malformed TransferBatch event");
        }
        let (token_ids, amounts) = <(Vec<U256>, Vec<U256>)>::abi_decode_sequence_validate(data)
            .map_err(|_| "TransferBatch data is not canonical")?;
        if (token_ids.as_slice(), amounts.as_slice()).abi_encode_sequence() != data {
            return Err("TransferBatch data is not canonical");
        }
        if token_ids.len() != amounts.len() {
            return Err("TransferBatch arrays have different lengths");
        }
        return Ok(StandardCandidate::Erc1155TransferBatch {
            contract: log.address,
            operator: indexed_address(&topics[1])?,
            from: indexed_address(&topics[2])?,
            to: indexed_address(&topics[3])?,
            items: token_ids.into_iter().zip(amounts).collect(),
        });
    }

    Err("unsupported standard event topic")
}

fn decode_wrapped_log(log: &Log) -> Result<(Address, U256, WrappedDirection), &'static str> {
    let topics = log.data.topics();
    if topics.len() != 2 || log.data.data.len() != 32 {
        return Err("malformed wrapped-native event");
    }
    let account = indexed_address(&topics[1])?;
    let amount = U256::from_be_slice(&log.data.data);
    let direction = if topics[0] == *DEPOSIT_TOPIC0 {
        WrappedDirection::Deposit
    } else if topics[0] == *WITHDRAWAL_TOPIC0 {
        WrappedDirection::Withdrawal
    } else {
        return Err("unsupported wrapped-native event topic");
    };
    Ok((account, amount, direction))
}

fn indexed_address(topic: &B256) -> Result<Address, &'static str> {
    if topic.as_slice()[..12].iter().any(|byte| *byte != 0) {
        return Err("indexed address is not zero padded");
    }
    Ok(Address::from_word(*topic))
}

fn nonzero_address(address: Address) -> Option<Address> {
    (address != Address::ZERO).then_some(address)
}

fn canonical_bool(data: &[u8]) -> Option<bool> {
    (data.len() == 32).then(|| {
        let value = U256::from_be_slice(data);
        if value.is_zero() {
            Some(false)
        } else if value == U256::from(1_u8) {
            Some(true)
        } else {
            None
        }
    })?
}

fn transfer_topic() -> B256 {
    keccak256("Transfer(address,address,uint256)")
}

fn approval_topic() -> B256 {
    keccak256("Approval(address,address,uint256)")
}

fn approval_for_all_topic() -> B256 {
    keccak256("ApprovalForAll(address,address,bool)")
}

fn transfer_single_topic() -> B256 {
    keccak256("TransferSingle(address,address,address,uint256,uint256)")
}

fn transfer_batch_topic() -> B256 {
    keccak256("TransferBatch(address,address,address,uint256[],uint256[])")
}

fn verify_candidate(
    candidate_index: usize,
    candidate: &Candidate,
    execution: &EvmTransactionExecution,
    state: &EvmStateAccess,
    pairs: &[WrappedTransferPair],
    wrapped_pair_evidence: &mut HashMap<usize, WrappedPairEvidence>,
    final_checks: &mut HashMap<CheckKey, CheckValue>,
) -> Result<VerifiedCandidate, EvmChangeResolutionError> {
    let pair = pairs.iter().enumerate().find_map(|(index, pair)| {
        (pair.standard_index == candidate_index || pair.wrapped_index == candidate_index)
            .then_some((index, *pair))
    });
    match candidate {
        Candidate::Standard {
            occurrence, kind, ..
        } => {
            let around = state.around(occurrence.handle())?;
            let mut context = VerificationContext {
                position: occurrence.position(),
                execution,
                previous: around.previous(),
                current: around.current(),
                frame_id: occurrence.frame_id(),
                pair,
                wrapped_pair_evidence,
                final_checks,
            };
            Ok(VerifiedCandidate::Standard(context.verify_standard(kind)?))
        }
        Candidate::Wrapped {
            occurrence,
            contract,
            account,
            amount,
            direction,
        } => {
            let around = state.around(occurrence.handle())?;
            let (after, total_supply_after) = verify_wrapped_transition(
                execution,
                occurrence.frame_id(),
                occurrence.position(),
                around.previous(),
                around.current(),
                *contract,
                *account,
                *amount,
                *direction,
                pair,
                wrapped_pair_evidence,
            )?;
            final_checks.insert(
                CheckKey::Erc20Balance {
                    contract: *contract,
                    account: *account,
                },
                CheckValue::Amount(after),
            );
            final_checks.insert(
                CheckKey::Erc20TotalSupply {
                    contract: *contract,
                },
                CheckValue::Amount(total_supply_after),
            );
            Ok(VerifiedCandidate::Wrapped)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn verify_wrapped_transition(
    execution: &EvmTransactionExecution,
    frame_id: EvmFrameId,
    position: EvmExecutionPosition,
    previous: &EvmStateReader,
    current: &EvmStateReader,
    contract: Address,
    account: Address,
    amount: U256,
    direction: WrappedDirection,
    pair: Option<(usize, WrappedTransferPair)>,
    wrapped_pair_evidence: &mut HashMap<usize, WrappedPairEvidence>,
) -> Result<(U256, U256), EvmChangeResolutionError> {
    require_wrapped_operation_evidence(
        execution, frame_id, contract, account, amount, direction, position,
    )?;
    let before = read_erc20_balance(previous, contract, account)?;
    let after = read_erc20_balance(current, contract, account)?;
    let total_supply_before = read_erc20_total_supply(previous, contract)?;
    let total_supply_after = read_erc20_total_supply(current, contract)?;
    let expected = match direction {
        WrappedDirection::Deposit => before.checked_add(amount),
        WrappedDirection::Withdrawal => before.checked_sub(amount),
    };
    let expected_total_supply = match direction {
        WrappedDirection::Deposit => total_supply_before.checked_add(amount),
        WrappedDirection::Withdrawal => total_supply_before.checked_sub(amount),
    };

    if expected == Some(after) && expected_total_supply == Some(total_supply_after) {
        record_wrapped_pair(
            wrapped_pair_evidence,
            pair,
            WrappedTransitionEvidence::Exact,
        );
        return Ok((after, total_supply_after));
    }

    if before != after || total_supply_before != total_supply_after {
        return Err(mismatch(
            position,
            "wrapped-native balance or total supply does not match Deposit/Withdrawal",
        ));
    }

    let Some(pair) = pair else {
        return Err(mismatch(
            position,
            "wrapped-native event did not change the expected balance",
        ));
    };
    record_wrapped_pair(
        wrapped_pair_evidence,
        Some(pair),
        WrappedTransitionEvidence::Unchanged,
    );
    Ok((after, total_supply_after))
}

fn require_wrapped_operation_evidence(
    execution: &EvmTransactionExecution,
    frame_id: EvmFrameId,
    contract: Address,
    account: Address,
    amount: U256,
    direction: WrappedDirection,
    position: EvmExecutionPosition,
) -> Result<(), EvmChangeResolutionError> {
    let operation = match direction {
        WrappedDirection::Deposit => has_call_in_scope(
            execution,
            frame_id,
            contract,
            position,
            |caller, value, input| {
                caller == account
                    && value == amount
                    && (input.is_empty() || input == selector("deposit()").as_slice())
            },
        ),
        WrappedDirection::Withdrawal => {
            let input = encode_call("withdraw(uint256)", (amount,).abi_encode_sequence());
            has_call_in_scope(
                execution,
                frame_id,
                contract,
                position,
                |caller, _, actual| caller == account && actual == input,
            ) && has_value_call(execution, frame_id, contract, account, amount, position)
        }
    };
    operation
        .then_some(())
        .ok_or_else(|| mismatch(position, "wrapped-native event has no matching value flow"))
}

fn record_wrapped_pair(
    evidence: &mut HashMap<usize, WrappedPairEvidence>,
    pair: Option<(usize, WrappedTransferPair)>,
    transition: WrappedTransitionEvidence,
) {
    if let Some((pair_index, _)) = pair {
        evidence.entry(pair_index).or_default().record(transition);
    }
}

struct VerificationContext<'a> {
    position: EvmExecutionPosition,
    execution: &'a EvmTransactionExecution,
    frame_id: EvmFrameId,
    previous: &'a EvmStateReader,
    current: &'a EvmStateReader,
    pair: Option<(usize, WrappedTransferPair)>,
    wrapped_pair_evidence: &'a mut HashMap<usize, WrappedPairEvidence>,
    final_checks: &'a mut HashMap<CheckKey, CheckValue>,
}

impl VerificationContext<'_> {
    fn verify_standard(
        &mut self,
        kind: &StandardCandidate,
    ) -> Result<VerifiedStandardChange, EvmChangeResolutionError> {
        let change = match kind {
            StandardCandidate::Erc20Transfer {
                contract,
                from,
                to,
                amount,
            } => {
                self.verify_erc20_transfer(*contract, *from, *to, *amount)?;
                VerifiedStandardChange::Erc20Transfer {
                    contract: *contract,
                    from: *from,
                    to: *to,
                    amount: *amount,
                }
            }
            StandardCandidate::Erc20Approval {
                contract,
                owner,
                spender,
                amount,
            } => {
                let before = read_allowance(self.previous, *contract, *owner, *spender)?;
                let after = read_allowance(self.current, *contract, *owner, *spender)?;
                if after != *amount {
                    return Err(mismatch(
                        self.position,
                        "ERC-20 allowance does not match Approval",
                    ));
                }
                if before == after {
                    require_erc20_approval_evidence(
                        self.execution,
                        self.frame_id,
                        *contract,
                        *owner,
                        *spender,
                        *amount,
                        self.position,
                    )?;
                }
                self.final_checks.insert(
                    CheckKey::Allowance {
                        contract: *contract,
                        owner: *owner,
                        spender: *spender,
                    },
                    CheckValue::Amount(after),
                );
                VerifiedStandardChange::Erc20Approval {
                    contract: *contract,
                    owner: *owner,
                    spender: *spender,
                    before,
                    after,
                }
            }
            StandardCandidate::Erc721Transfer {
                contract,
                from,
                to,
                token_id,
            } => {
                if *from == Address::ZERO && *to == Address::ZERO {
                    return Err(mismatch(
                        self.position,
                        "ERC-721 Transfer cannot mint to or burn from the zero address",
                    ));
                }
                let before = read_owner(self.previous, *contract, *token_id)?;
                let after = read_owner(self.current, *contract, *token_id)?;
                if *from == Address::ZERO {
                    if before.is_some() || after != Some(*to) {
                        return Err(mismatch(
                            self.position,
                            "ERC-721 mint owner does not match Transfer",
                        ));
                    }
                } else if *to == Address::ZERO {
                    if before != Some(*from) || after.is_some() {
                        return Err(mismatch(
                            self.position,
                            "ERC-721 burn owner does not match Transfer",
                        ));
                    }
                } else if before != Some(*from) || after != Some(*to) {
                    return Err(mismatch(
                        self.position,
                        "ERC-721 owner does not match Transfer",
                    ));
                }
                if before == after {
                    require_erc721_transfer_evidence(
                        self.execution,
                        self.frame_id,
                        *contract,
                        *from,
                        *to,
                        *token_id,
                        self.position,
                    )?;
                }
                let approval_before = if before.is_some() {
                    read_approved(self.previous, *contract, *token_id)?
                } else {
                    read_approved_optional(self.previous, *contract, *token_id)?
                };
                if *from == Address::ZERO && approval_before.is_some() {
                    return Err(mismatch(
                        self.position,
                        "ERC-721 mint started with an unexpected token approval",
                    ));
                }
                let approval_after = if after.is_some() {
                    read_approved(self.current, *contract, *token_id)?
                } else {
                    read_approved_optional(self.current, *contract, *token_id)?
                };
                if approval_after.is_some() {
                    return Err(mismatch(
                        self.position,
                        "ERC-721 Transfer did not clear the token approval",
                    ));
                }
                self.final_checks.insert(
                    CheckKey::Owner {
                        contract: *contract,
                        token_id: *token_id,
                    },
                    CheckValue::Owner(after),
                );
                self.final_checks.insert(
                    CheckKey::Approved {
                        contract: *contract,
                        token_id: *token_id,
                    },
                    CheckValue::Owner(None),
                );
                VerifiedStandardChange::Erc721Transfer {
                    contract: *contract,
                    from: *from,
                    to: *to,
                    token_id: *token_id,
                }
            }
            StandardCandidate::Erc721Approval {
                contract,
                owner,
                approved,
                token_id,
            } => {
                let owner_state = read_owner(self.current, *contract, *token_id)?;
                if owner_state != Some(*owner) {
                    return Err(mismatch(
                        self.position,
                        "ERC-721 Approval owner does not own the token",
                    ));
                }
                let before = read_approved(self.previous, *contract, *token_id)?;
                let after = read_approved(self.current, *contract, *token_id)?;
                if after != *approved {
                    return Err(mismatch(
                        self.position,
                        "ERC-721 approval does not match Approval",
                    ));
                }
                if before == after {
                    require_erc721_approval_evidence(
                        self.execution,
                        self.frame_id,
                        *contract,
                        *owner,
                        *approved,
                        *token_id,
                        self.position,
                    )?;
                }
                self.final_checks.insert(
                    CheckKey::Approved {
                        contract: *contract,
                        token_id: *token_id,
                    },
                    CheckValue::Owner(after),
                );
                VerifiedStandardChange::Erc721Approval {
                    contract: *contract,
                    owner: *owner,
                    before,
                    after,
                    token_id: *token_id,
                }
            }
            StandardCandidate::OperatorApproval {
                contract,
                owner,
                operator,
                approved,
            } => {
                let before = read_operator(self.previous, *contract, *owner, *operator)?;
                let after = read_operator(self.current, *contract, *owner, *operator)?;
                if after != *approved {
                    return Err(mismatch(
                        self.position,
                        "operator approval does not match event",
                    ));
                }
                if before == after {
                    require_operator_approval_evidence(
                        self.execution,
                        self.frame_id,
                        *contract,
                        *owner,
                        *operator,
                        *approved,
                        self.position,
                    )?;
                }
                self.final_checks.insert(
                    CheckKey::Operator {
                        contract: *contract,
                        owner: *owner,
                        operator: *operator,
                    },
                    CheckValue::Bool(after),
                );
                VerifiedStandardChange::OperatorApproval {
                    contract: *contract,
                    owner: *owner,
                    operator: *operator,
                    before,
                    after,
                }
            }
            StandardCandidate::Erc1155TransferSingle {
                contract,
                operator,
                from,
                to,
                token_id,
                amount,
            } => {
                self.verify_erc1155_transfer(
                    *contract,
                    *from,
                    *to,
                    &[(*token_id, *amount)],
                    false,
                )?;
                VerifiedStandardChange::Erc1155TransferSingle {
                    contract: *contract,
                    operator: *operator,
                    from: *from,
                    to: *to,
                    token_id: *token_id,
                    amount: *amount,
                }
            }
            StandardCandidate::Erc1155TransferBatch {
                contract,
                operator,
                from,
                to,
                items,
            } => {
                self.verify_erc1155_transfer(*contract, *from, *to, items, true)?;
                VerifiedStandardChange::Erc1155TransferBatch {
                    contract: *contract,
                    operator: *operator,
                    from: *from,
                    to: *to,
                    items: items.clone(),
                }
            }
        };
        Ok(change)
    }

    fn verify_erc20_transfer(
        &mut self,
        contract: Address,
        from: Address,
        to: Address,
        amount: U256,
    ) -> Result<(), EvmChangeResolutionError> {
        if from == Address::ZERO && to == Address::ZERO {
            return Err(mismatch(
                self.position,
                "ERC-20 Transfer cannot mint to or burn from the zero address",
            ));
        }
        let from_before = (from != Address::ZERO)
            .then(|| read_erc20_balance(self.previous, contract, from))
            .transpose()?;
        let from_after = (from != Address::ZERO)
            .then(|| read_erc20_balance(self.current, contract, from))
            .transpose()?;
        let to_before = (to != Address::ZERO)
            .then(|| read_erc20_balance(self.previous, contract, to))
            .transpose()?;
        let to_after = (to != Address::ZERO)
            .then(|| read_erc20_balance(self.current, contract, to))
            .transpose()?;
        let total_supply_before = (from == Address::ZERO || to == Address::ZERO)
            .then(|| read_erc20_total_supply(self.previous, contract))
            .transpose()?;
        let total_supply_after = (from == Address::ZERO || to == Address::ZERO)
            .then(|| read_erc20_total_supply(self.current, contract))
            .transpose()?;

        let source_exact = match (from_before, from_after) {
            (Some(before), Some(after)) => before.checked_sub(amount) == Some(after),
            (None, None) => true,
            _ => false,
        };
        let target_exact = match (to_before, to_after) {
            (Some(before), Some(after)) => before.checked_add(amount) == Some(after),
            (None, None) => true,
            _ => false,
        };
        let source_unchanged = match (from_before, from_after) {
            (Some(before), Some(after)) => before == after,
            (None, None) => true,
            _ => false,
        };
        let target_unchanged = match (to_before, to_after) {
            (Some(before), Some(after)) => before == after,
            (None, None) => true,
            _ => false,
        };
        let supply_exact = match (total_supply_before, total_supply_after) {
            (Some(before), Some(after)) if from == Address::ZERO => {
                before.checked_add(amount) == Some(after)
            }
            (Some(before), Some(after)) => before.checked_sub(amount) == Some(after),
            (None, None) => true,
            _ => false,
        };
        let supply_unchanged = match (total_supply_before, total_supply_after) {
            (Some(before), Some(after)) => before == after,
            (None, None) => true,
            _ => false,
        };

        if from == to {
            if !source_unchanged {
                return Err(mismatch(
                    self.position,
                    "self ERC-20 Transfer changed an unexpected balance",
                ));
            }
            self.require_erc20_transfer_evidence(contract, from, to, amount)?;
        } else if source_exact && target_exact && supply_exact {
            if amount == U256::ZERO {
                self.require_erc20_transfer_evidence(contract, from, to, amount)?;
            }
            self.record_wrapped_pair(WrappedTransitionEvidence::Exact);
        } else if source_unchanged && target_unchanged && supply_unchanged {
            let Some((_, pair)) = self.pair else {
                return Err(mismatch(
                    self.position,
                    "ERC-20 Transfer did not change the expected balances",
                ));
            };
            let (account, direction) = match pair.direction {
                WrappedDirection::Deposit => (to, WrappedDirection::Deposit),
                WrappedDirection::Withdrawal => (from, WrappedDirection::Withdrawal),
            };
            require_wrapped_operation_evidence(
                self.execution,
                self.frame_id,
                contract,
                account,
                amount,
                direction,
                self.position,
            )?;
            self.record_wrapped_pair(WrappedTransitionEvidence::Unchanged);
        } else {
            return Err(mismatch(
                self.position,
                "ERC-20 Transfer did not change the expected balances",
            ));
        }

        if let Some(value) = from_after {
            self.final_checks.insert(
                CheckKey::Erc20Balance {
                    contract,
                    account: from,
                },
                CheckValue::Amount(value),
            );
        }
        if let Some(value) = to_after {
            self.final_checks.insert(
                CheckKey::Erc20Balance {
                    contract,
                    account: to,
                },
                CheckValue::Amount(value),
            );
        }
        if let Some(value) = total_supply_after {
            self.final_checks.insert(
                CheckKey::Erc20TotalSupply { contract },
                CheckValue::Amount(value),
            );
        }
        Ok(())
    }

    fn require_erc20_transfer_evidence(
        &self,
        contract: Address,
        from: Address,
        to: Address,
        amount: U256,
    ) -> Result<(), EvmChangeResolutionError> {
        if let Some((_, pair)) = self.pair {
            let account = match pair.direction {
                WrappedDirection::Deposit => to,
                WrappedDirection::Withdrawal => from,
            };
            require_wrapped_operation_evidence(
                self.execution,
                self.frame_id,
                contract,
                account,
                amount,
                pair.direction,
                self.position,
            )
        } else {
            require_erc20_transfer_evidence(
                self.execution,
                self.frame_id,
                contract,
                from,
                to,
                amount,
                self.position,
            )
        }
    }

    fn record_wrapped_pair(&mut self, transition: WrappedTransitionEvidence) {
        record_wrapped_pair(self.wrapped_pair_evidence, self.pair, transition);
    }

    fn verify_erc1155_transfer(
        &mut self,
        contract: Address,
        from: Address,
        to: Address,
        items: &[(U256, U256)],
        batch: bool,
    ) -> Result<(), EvmChangeResolutionError> {
        if from == Address::ZERO && to == Address::ZERO {
            return Err(mismatch(
                self.position,
                "ERC-1155 transfer cannot mint to or burn from the zero address",
            ));
        }
        if from == to || items.iter().any(|(_, amount)| *amount == U256::ZERO) {
            require_erc1155_transfer_evidence(
                self.execution,
                self.frame_id,
                contract,
                from,
                to,
                items,
                batch,
                self.position,
            )?;
        }

        let mut totals = HashMap::<U256, U256>::new();
        for &(token_id, amount) in items {
            let entry = totals.entry(token_id).or_insert(U256::ZERO);
            *entry = entry
                .checked_add(amount)
                .ok_or_else(|| mismatch(self.position, "ERC-1155 batch amount overflow"))?;
        }

        for (token_id, amount) in totals {
            let from_before = (from != Address::ZERO)
                .then(|| read_erc1155_balance(self.previous, contract, from, token_id))
                .transpose()?;
            let from_after = (from != Address::ZERO)
                .then(|| read_erc1155_balance(self.current, contract, from, token_id))
                .transpose()?;
            let to_before = (to != Address::ZERO)
                .then(|| read_erc1155_balance(self.previous, contract, to, token_id))
                .transpose()?;
            let to_after = (to != Address::ZERO)
                .then(|| read_erc1155_balance(self.current, contract, to, token_id))
                .transpose()?;

            if from == to {
                if from_before != from_after {
                    return Err(mismatch(
                        self.position,
                        "self ERC-1155 transfer changed an unexpected balance",
                    ));
                }
            } else {
                if let (Some(before), Some(after)) = (from_before, from_after) {
                    expect_decrease(
                        before,
                        after,
                        amount,
                        self.position,
                        "ERC-1155 transfer source",
                    )?;
                }
                if let (Some(before), Some(after)) = (to_before, to_after) {
                    expect_increase(
                        before,
                        after,
                        amount,
                        self.position,
                        "ERC-1155 transfer target",
                    )?;
                }
            }

            if let Some(value) = from_after {
                self.final_checks.insert(
                    CheckKey::Erc1155Balance {
                        contract,
                        account: from,
                        token_id,
                    },
                    CheckValue::Amount(value),
                );
            }
            if let Some(value) = to_after {
                self.final_checks.insert(
                    CheckKey::Erc1155Balance {
                        contract,
                        account: to,
                        token_id,
                    },
                    CheckValue::Amount(value),
                );
            }
        }
        Ok(())
    }
}

fn expect_increase(
    before: U256,
    after: U256,
    amount: U256,
    position: EvmExecutionPosition,
    label: &'static str,
) -> Result<(), EvmChangeResolutionError> {
    let expected = before
        .checked_add(amount)
        .ok_or_else(|| mismatch(position, "balance increase overflow"))?;
    (after == expected)
        .then_some(())
        .ok_or_else(|| mismatch(position, label))
}

fn expect_decrease(
    before: U256,
    after: U256,
    amount: U256,
    position: EvmExecutionPosition,
    label: &'static str,
) -> Result<(), EvmChangeResolutionError> {
    let expected = before
        .checked_sub(amount)
        .ok_or_else(|| mismatch(position, "balance decrease underflow"))?;
    (after == expected)
        .then_some(())
        .ok_or_else(|| mismatch(position, label))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum CheckKey {
    Erc20Balance {
        contract: Address,
        account: Address,
    },
    Erc20TotalSupply {
        contract: Address,
    },
    Erc1155Balance {
        contract: Address,
        account: Address,
        token_id: U256,
    },
    Allowance {
        contract: Address,
        owner: Address,
        spender: Address,
    },
    Owner {
        contract: Address,
        token_id: U256,
    },
    Approved {
        contract: Address,
        token_id: U256,
    },
    Operator {
        contract: Address,
        owner: Address,
        operator: Address,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CheckValue {
    Amount(U256),
    Owner(Option<Address>),
    Bool(bool),
}

fn verify_final_checks(
    checks: &HashMap<CheckKey, CheckValue>,
    state: &EvmStateAccess,
) -> Result<(), EvmChangeResolutionError> {
    for (key, expected) in checks {
        let actual = match key {
            CheckKey::Erc20Balance { contract, account } => {
                CheckValue::Amount(read_erc20_balance(state.finalized(), *contract, *account)?)
            }
            CheckKey::Erc20TotalSupply { contract } => {
                CheckValue::Amount(read_erc20_total_supply(state.finalized(), *contract)?)
            }
            CheckKey::Erc1155Balance {
                contract,
                account,
                token_id,
            } => CheckValue::Amount(read_erc1155_balance(
                state.finalized(),
                *contract,
                *account,
                *token_id,
            )?),
            CheckKey::Allowance {
                contract,
                owner,
                spender,
            } => CheckValue::Amount(read_allowance(
                state.finalized(),
                *contract,
                *owner,
                *spender,
            )?),
            CheckKey::Owner { contract, token_id } => {
                CheckValue::Owner(read_owner(state.finalized(), *contract, *token_id)?)
            }
            CheckKey::Approved { contract, token_id } => {
                let owner_key = CheckKey::Owner {
                    contract: *contract,
                    token_id: *token_id,
                };
                let token_is_missing = checks.get(&owner_key) == Some(&CheckValue::Owner(None));
                let approved = if token_is_missing {
                    read_approved_optional(state.finalized(), *contract, *token_id)?
                } else {
                    read_approved(state.finalized(), *contract, *token_id)?
                };
                CheckValue::Owner(approved)
            }
            CheckKey::Operator {
                contract,
                owner,
                operator,
            } => CheckValue::Bool(read_operator(
                state.finalized(),
                *contract,
                *owner,
                *operator,
            )?),
        };
        if &actual != expected {
            return Err(standard_error(
                "finalized state differs from the last verified occurrence",
            ));
        }
    }
    Ok(())
}

fn read_erc20_balance(
    view: &EvmStateReader,
    contract: Address,
    account: Address,
) -> Result<U256, EvmChangeResolutionError> {
    let output = read_call_output(
        view,
        contract,
        IERC20State::balanceOfCall { account }.abi_encode().into(),
        "balanceOf(address)",
    )?;
    IERC20State::balanceOfCall::abi_decode_returns_validate(&output)
        .map_err(|error| standard_error(format!("invalid balanceOf return data: {error}")))
}

fn read_erc20_total_supply(
    view: &EvmStateReader,
    contract: Address,
) -> Result<U256, EvmChangeResolutionError> {
    let output = read_call_output(
        view,
        contract,
        IERC20State::totalSupplyCall {}.abi_encode().into(),
        "totalSupply()",
    )?;
    IERC20State::totalSupplyCall::abi_decode_returns_validate(&output)
        .map_err(|error| standard_error(format!("invalid totalSupply return data: {error}")))
}

fn read_erc1155_balance(
    view: &EvmStateReader,
    contract: Address,
    account: Address,
    token_id: U256,
) -> Result<U256, EvmChangeResolutionError> {
    let output = read_call_output(
        view,
        contract,
        IERC1155State::balanceOfCall {
            account,
            id: token_id,
        }
        .abi_encode()
        .into(),
        "balanceOf(address,uint256)",
    )?;
    IERC1155State::balanceOfCall::abi_decode_returns_validate(&output)
        .map_err(|error| standard_error(format!("invalid ERC-1155 balanceOf return data: {error}")))
}

fn read_allowance(
    view: &EvmStateReader,
    contract: Address,
    owner: Address,
    spender: Address,
) -> Result<U256, EvmChangeResolutionError> {
    let output = read_call_output(
        view,
        contract,
        IERC20State::allowanceCall { owner, spender }
            .abi_encode()
            .into(),
        "allowance(address,address)",
    )?;
    IERC20State::allowanceCall::abi_decode_returns_validate(&output)
        .map_err(|error| standard_error(format!("invalid allowance return data: {error}")))
}

fn read_owner(
    view: &EvmStateReader,
    contract: Address,
    token_id: U256,
) -> Result<Option<Address>, EvmChangeResolutionError> {
    match view.read_call(
        contract,
        IERC721State::ownerOfCall { tokenId: token_id }
            .abi_encode()
            .into(),
    )? {
        EvmReadCallOutcome::Success(output) => {
            IERC721State::ownerOfCall::abi_decode_returns_validate(&output)
                .map(Some)
                .map_err(|error| standard_error(format!("invalid ownerOf return data: {error}")))
        }
        EvmReadCallOutcome::Reverted(_) => Ok(None),
        EvmReadCallOutcome::Halted { reason } => {
            Err(standard_error(format!("ownerOf halted: {reason}")))
        }
    }
}

fn read_approved(
    view: &EvmStateReader,
    contract: Address,
    token_id: U256,
) -> Result<Option<Address>, EvmChangeResolutionError> {
    let output = read_call_output(
        view,
        contract,
        IERC721State::getApprovedCall { tokenId: token_id }
            .abi_encode()
            .into(),
        "getApproved(uint256)",
    )?;
    let address = IERC721State::getApprovedCall::abi_decode_returns_validate(&output)
        .map_err(|error| standard_error(format!("invalid getApproved return data: {error}")))?;
    Ok(nonzero_address(address))
}

fn read_approved_optional(
    view: &EvmStateReader,
    contract: Address,
    token_id: U256,
) -> Result<Option<Address>, EvmChangeResolutionError> {
    match view.read_call(
        contract,
        IERC721State::getApprovedCall { tokenId: token_id }
            .abi_encode()
            .into(),
    )? {
        EvmReadCallOutcome::Success(output) => {
            let address = IERC721State::getApprovedCall::abi_decode_returns_validate(&output)
                .map_err(|error| {
                    standard_error(format!("invalid getApproved return data: {error}"))
                })?;
            Ok(nonzero_address(address))
        }
        EvmReadCallOutcome::Reverted(_) => Ok(None),
        EvmReadCallOutcome::Halted { reason } => {
            Err(standard_error(format!("getApproved halted: {reason}")))
        }
    }
}

fn read_operator(
    view: &EvmStateReader,
    contract: Address,
    owner: Address,
    operator: Address,
) -> Result<bool, EvmChangeResolutionError> {
    let output = read_call_output(
        view,
        contract,
        IERC721State::isApprovedForAllCall { owner, operator }
            .abi_encode()
            .into(),
        "isApprovedForAll(address,address)",
    )?;
    IERC721State::isApprovedForAllCall::abi_decode_returns_validate(&output)
        .map_err(|error| standard_error(format!("invalid isApprovedForAll return data: {error}")))
}

fn read_call_output(
    view: &EvmStateReader,
    target: Address,
    calldata: Bytes,
    operation: &'static str,
) -> Result<Bytes, EvmChangeResolutionError> {
    match view.read_call(target, calldata)? {
        EvmReadCallOutcome::Success(output) => Ok(output),
        EvmReadCallOutcome::Reverted(_) => Err(standard_error(format!("{operation} reverted"))),
        EvmReadCallOutcome::Halted { reason } => {
            Err(standard_error(format!("{operation} halted: {reason}")))
        }
    }
}

fn require_erc20_transfer_evidence(
    execution: &EvmTransactionExecution,
    frame_id: EvmFrameId,
    contract: Address,
    from: Address,
    to: Address,
    amount: U256,
    position: EvmExecutionPosition,
) -> Result<(), EvmChangeResolutionError> {
    let transfer = encode_call(
        "transfer(address,uint256)",
        (to, amount).abi_encode_sequence(),
    );
    let transfer_from = encode_call(
        "transferFrom(address,address,uint256)",
        (from, to, amount).abi_encode_sequence(),
    );
    has_call_in_scope(execution, frame_id, contract, position, |_, _, input| {
        input == transfer || input == transfer_from
    })
    .then_some(())
    .ok_or_else(|| mismatch(position, "ERC-20 no-op has no matching committed call"))
}

fn require_erc20_approval_evidence(
    execution: &EvmTransactionExecution,
    frame_id: EvmFrameId,
    contract: Address,
    owner: Address,
    spender: Address,
    amount: U256,
    position: EvmExecutionPosition,
) -> Result<(), EvmChangeResolutionError> {
    let expected = encode_call(
        "approve(address,uint256)",
        (spender, amount).abi_encode_sequence(),
    );
    has_call_in_scope(
        execution,
        frame_id,
        contract,
        position,
        |caller, _, input| caller == owner && input == expected,
    )
    .then_some(())
    .ok_or_else(|| mismatch(position, "ERC-20 Approval no-op has no matching call"))
}

fn require_erc721_transfer_evidence(
    execution: &EvmTransactionExecution,
    frame_id: EvmFrameId,
    contract: Address,
    from: Address,
    to: Address,
    token_id: U256,
    position: EvmExecutionPosition,
) -> Result<(), EvmChangeResolutionError> {
    has_call_in_scope(execution, frame_id, contract, position, |_, _, input| {
        matches_erc721_transfer_call(input, from, Some(to), token_id)
    })
    .then_some(())
    .ok_or_else(|| mismatch(position, "ERC-721 no-op has no matching committed call"))
}

fn require_erc721_approval_evidence(
    execution: &EvmTransactionExecution,
    frame_id: EvmFrameId,
    contract: Address,
    owner: Address,
    approved: Option<Address>,
    token_id: U256,
    position: EvmExecutionPosition,
) -> Result<(), EvmChangeResolutionError> {
    let expected = encode_call(
        "approve(address,uint256)",
        (approved.unwrap_or(Address::ZERO), token_id).abi_encode_sequence(),
    );
    has_call_in_scope(execution, frame_id, contract, position, |_, _, input| {
        input == expected || matches_erc721_transfer_call(input, owner, None, token_id)
    })
    .then_some(())
    .ok_or_else(|| mismatch(position, "ERC-721 Approval no-op has no matching call"))
}

fn matches_erc721_transfer_call(
    input: &[u8],
    from: Address,
    to: Option<Address>,
    token_id: U256,
) -> bool {
    for signature in [
        "transferFrom(address,address,uint256)",
        "safeTransferFrom(address,address,uint256)",
    ] {
        if decode_call::<(Address, Address, U256)>(input, selector(signature)).is_some_and(
            |(actual_from, actual_to, actual_id)| {
                actual_from == from
                    && to.is_none_or(|expected| actual_to == expected)
                    && actual_id == token_id
            },
        ) {
            return true;
        }
    }

    decode_call::<(Address, Address, U256, Bytes)>(
        input,
        selector("safeTransferFrom(address,address,uint256,bytes)"),
    )
    .is_some_and(|(actual_from, actual_to, actual_id, _)| {
        actual_from == from
            && to.is_none_or(|expected| actual_to == expected)
            && actual_id == token_id
    })
}

#[allow(clippy::too_many_arguments)]
fn require_operator_approval_evidence(
    execution: &EvmTransactionExecution,
    frame_id: EvmFrameId,
    contract: Address,
    owner: Address,
    operator: Address,
    approved: bool,
    position: EvmExecutionPosition,
) -> Result<(), EvmChangeResolutionError> {
    let expected = encode_call(
        "setApprovalForAll(address,bool)",
        (operator, approved).abi_encode_sequence(),
    );
    has_call_in_scope(
        execution,
        frame_id,
        contract,
        position,
        |caller, _, input| caller == owner && input == expected,
    )
    .then_some(())
    .ok_or_else(|| mismatch(position, "operator Approval no-op has no matching call"))
}

#[allow(clippy::too_many_arguments)]
fn require_erc1155_transfer_evidence(
    execution: &EvmTransactionExecution,
    frame_id: EvmFrameId,
    contract: Address,
    from: Address,
    to: Address,
    items: &[(U256, U256)],
    batch: bool,
    position: EvmExecutionPosition,
) -> Result<(), EvmChangeResolutionError> {
    let matches = has_call_in_scope(execution, frame_id, contract, position, |_, _, input| {
        if batch {
            let signature =
                selector("safeBatchTransferFrom(address,address,uint256[],uint256[],bytes)");
            decode_call::<(Address, Address, Vec<U256>, Vec<U256>, Bytes)>(input, signature)
                .is_some_and(|(actual_from, actual_to, ids, amounts, _)| {
                    actual_from == from
                        && actual_to == to
                        && ids.into_iter().zip(amounts).eq(items.iter().copied())
                })
        } else {
            let signature = selector("safeTransferFrom(address,address,uint256,uint256,bytes)");
            decode_call::<(Address, Address, U256, U256, Bytes)>(input, signature).is_some_and(
                |(actual_from, actual_to, id, amount, _)| {
                    actual_from == from && actual_to == to && items == [(id, amount)].as_slice()
                },
            )
        }
    });
    matches
        .then_some(())
        .ok_or_else(|| mismatch(position, "ERC-1155 no-op has no matching committed call"))
}

fn has_call_in_scope(
    execution: &EvmTransactionExecution,
    frame_id: EvmFrameId,
    contract: Address,
    position: EvmExecutionPosition,
    matches: impl Fn(Address, U256, &[u8]) -> bool,
) -> bool {
    execution.committed_frames().iter().any(|frame| {
        let EvmFrameAction::Call {
            caller,
            target,
            bytecode_address,
            value,
            input,
            ..
        } = frame.action()
        else {
            return false;
        };
        frames_are_nested(execution, frame.id(), frame_id)
            && frame.position().index() <= position.index()
            && (*target == contract || *bytecode_address == contract)
            && matches(*caller, *value, input)
    })
}

fn has_value_call(
    execution: &EvmTransactionExecution,
    frame_id: EvmFrameId,
    from: Address,
    to: Address,
    amount: U256,
    position: EvmExecutionPosition,
) -> bool {
    execution.committed_frames().iter().any(|frame| {
        let EvmFrameAction::Call {
            caller,
            target,
            value,
            ..
        } = frame.action()
        else {
            return false;
        };
        frames_are_nested(execution, frame.id(), frame_id)
            && frame.position().index() <= position.index()
            && *caller == from
            && *target == to
            && *value == amount
    })
}

fn frames_are_nested(
    execution: &EvmTransactionExecution,
    first: EvmFrameId,
    second: EvmFrameId,
) -> bool {
    frame_is_ancestor(execution, first, second) || frame_is_ancestor(execution, second, first)
}

fn frame_is_ancestor(
    execution: &EvmTransactionExecution,
    ancestor: EvmFrameId,
    descendant: EvmFrameId,
) -> bool {
    let mut current = Some(descendant);
    while let Some(frame_id) = current {
        if frame_id == ancestor {
            return true;
        }
        current = execution
            .committed_frames()
            .iter()
            .find(|frame| frame.id() == frame_id)
            .and_then(EvmCommittedFrame::parent);
    }
    false
}

fn encode_call(signature: &str, encoded_arguments: Vec<u8>) -> Vec<u8> {
    let mut input = Vec::with_capacity(4 + encoded_arguments.len());
    input.extend_from_slice(&selector(signature));
    input.extend(encoded_arguments);
    input
}

fn decode_call<T>(input: &[u8], expected_selector: [u8; 4]) -> Option<T>
where
    T: SolValue,
    for<'a> <T::SolType as SolType>::Token<'a>: TokenSeq<'a>,
    T: From<<T::SolType as SolType>::RustType>,
{
    let arguments = input.strip_prefix(expected_selector.as_slice())?;
    T::abi_decode_sequence_validate(arguments).ok()
}

fn selector(signature: &str) -> [u8; 4] {
    let hash = keccak256(signature);
    [hash[0], hash[1], hash[2], hash[3]]
}

fn load_metadata(candidates: &[Candidate], state: &EvmStateAccess) -> MetadataValues<Address> {
    let decoded = candidates.iter().filter_map(|candidate| match candidate {
        Candidate::Standard { decoded, .. } => Some(decoded),
        Candidate::Wrapped { .. } => None,
    });
    let mut calls = metadata_calls(decoded);
    let mut seen = calls.iter().cloned().collect::<HashSet<_>>();
    for candidate in candidates {
        let Candidate::Wrapped { contract, .. } = candidate else {
            continue;
        };
        for call in [
            MetadataCall::Name {
                contract_address: *contract,
            },
            MetadataCall::Symbol {
                contract_address: *contract,
            },
            MetadataCall::Decimals {
                contract_address: *contract,
            },
        ] {
            if seen.insert(call.clone()) {
                calls.push(call);
            }
        }
    }

    let mut values = MetadataValues::default();
    for call in calls {
        let target = *call.contract_address();
        match state.finalized().read_call(target, call.call_data()) {
            Ok(EvmReadCallOutcome::Success(output)) => {
                values.record_output(call, &output);
            }
            Ok(_) | Err(_) => values.record_unavailable(call),
        }
    }
    values
}

fn mismatch(position: EvmExecutionPosition, details: &'static str) -> EvmChangeResolutionError {
    standard_error_at(position, details)
}

fn standard_error(details: impl Into<String>) -> EvmChangeResolutionError {
    EvmChangeResolutionError::resolver(
        "standard-token",
        StandardResolverError::Details {
            details: details.into(),
        },
    )
}

fn standard_error_at(
    position: EvmExecutionPosition,
    details: impl Into<String>,
) -> EvmChangeResolutionError {
    standard_error(format!(
        "at execution position {}: {}",
        position.index(),
        details.into()
    ))
}

#[derive(Debug, Error)]
enum StandardResolverError {
    #[error("{details}")]
    Details { details: String },
}
