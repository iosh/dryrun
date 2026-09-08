use std::collections::HashMap;

use alloy::primitives::{Address, U256};
use contract_standards::DecodedStandardEvent;

use crate::{
    EvmChangeDerivationError,
    execution::{EvmExecutionPosition, EvmFrameId, EvmTransactionExecution},
    state::EvmStateReader,
};

use super::{
    call_evidence::{
        verify_erc20_approval_call, verify_erc20_transfer_call, verify_erc721_approval_call,
        verify_erc721_transfer_call, verify_erc1155_transfer_call, verify_operator_approval_call,
    },
    error::state_mismatch_at,
    events::{WrappedEventPair, WrappedOperation, WrappedPairProof, WrappedStateEffect},
    sequence_verification::{
        ExpectedFinalValue, FinalStateQuery, expect_decrease, expect_increase,
        record_wrapped_effect, verify_wrapped_call_and_value,
    },
    state_queries::{
        read_allowance, read_erc20_balance, read_erc20_total_supply, read_erc721_approval,
        read_erc721_approval_optional, read_erc721_owner, read_erc1155_balance,
        read_operator_approval,
    },
    verified_changes::VerifiedTokenChange,
};

pub(super) struct TokenEventVerification<'a> {
    pub(super) position: EvmExecutionPosition,
    pub(super) execution: &'a EvmTransactionExecution,
    pub(super) frame_id: EvmFrameId,
    pub(super) previous: &'a EvmStateReader,
    pub(super) current: &'a EvmStateReader,
    pub(super) pair: Option<(usize, WrappedEventPair)>,
    pub(super) wrapped_pair_proofs: &'a mut HashMap<usize, WrappedPairProof>,
    pub(super) final_state_expectations: &'a mut HashMap<FinalStateQuery, ExpectedFinalValue>,
}

impl TokenEventVerification<'_> {
    pub(super) fn verify_standard_event(
        &mut self,
        event: &DecodedStandardEvent<Address>,
    ) -> Result<VerifiedTokenChange, EvmChangeDerivationError> {
        let change = match event {
            DecodedStandardEvent::Erc20Transfer {
                token,
                from,
                to,
                amount,
            } => {
                self.verify_erc20_transfer(*token, *from, *to, *amount)?;
                VerifiedTokenChange::Erc20Transfer {
                    contract: *token,
                    from: *from,
                    to: *to,
                    amount: *amount,
                }
            }
            DecodedStandardEvent::Erc20Approval {
                token,
                owner,
                spender,
                value,
            } => {
                let before = read_allowance(self.previous, *token, *owner, *spender)?;
                let after = read_allowance(self.current, *token, *owner, *spender)?;
                if after != *value {
                    return Err(state_mismatch_at(
                        self.position,
                        "ERC-20 allowance does not match Approval",
                    ));
                }
                if before == after {
                    verify_erc20_approval_call(
                        self.execution,
                        self.frame_id,
                        *token,
                        *owner,
                        *spender,
                        *value,
                        self.position,
                    )?;
                }
                self.final_state_expectations.insert(
                    FinalStateQuery::Allowance {
                        contract: *token,
                        owner: *owner,
                        spender: *spender,
                    },
                    ExpectedFinalValue::Amount(after),
                );
                VerifiedTokenChange::Erc20Approval {
                    contract: *token,
                    owner: *owner,
                    spender: *spender,
                    before,
                    after,
                }
            }
            DecodedStandardEvent::Erc721Transfer {
                collection,
                from,
                to,
                token_id,
            } => {
                if *from == Address::ZERO && *to == Address::ZERO {
                    return Err(state_mismatch_at(
                        self.position,
                        "ERC-721 Transfer cannot mint to or burn from the zero address",
                    ));
                }
                let before = read_erc721_owner(self.previous, *collection, *token_id)?;
                let after = read_erc721_owner(self.current, *collection, *token_id)?;
                if *from == Address::ZERO {
                    if before.is_some() || after != Some(*to) {
                        return Err(state_mismatch_at(
                            self.position,
                            "ERC-721 mint owner does not match Transfer",
                        ));
                    }
                } else if *to == Address::ZERO {
                    if before != Some(*from) || after.is_some() {
                        return Err(state_mismatch_at(
                            self.position,
                            "ERC-721 burn owner does not match Transfer",
                        ));
                    }
                } else if before != Some(*from) || after != Some(*to) {
                    return Err(state_mismatch_at(
                        self.position,
                        "ERC-721 owner does not match Transfer",
                    ));
                }
                if before == after {
                    verify_erc721_transfer_call(
                        self.execution,
                        self.frame_id,
                        *collection,
                        *from,
                        *to,
                        *token_id,
                        self.position,
                    )?;
                }
                let approval_before = if before.is_some() {
                    read_erc721_approval(self.previous, *collection, *token_id)?
                } else {
                    read_erc721_approval_optional(self.previous, *collection, *token_id)?
                };
                if *from == Address::ZERO && approval_before.is_some() {
                    return Err(state_mismatch_at(
                        self.position,
                        "ERC-721 mint started with an unexpected token approval",
                    ));
                }
                let approval_after = if after.is_some() {
                    read_erc721_approval(self.current, *collection, *token_id)?
                } else {
                    read_erc721_approval_optional(self.current, *collection, *token_id)?
                };
                if approval_after.is_some() {
                    return Err(state_mismatch_at(
                        self.position,
                        "ERC-721 Transfer did not clear the token approval",
                    ));
                }
                self.final_state_expectations.insert(
                    FinalStateQuery::Owner {
                        contract: *collection,
                        token_id: *token_id,
                    },
                    ExpectedFinalValue::Owner(after),
                );
                self.final_state_expectations.insert(
                    FinalStateQuery::Approved {
                        contract: *collection,
                        token_id: *token_id,
                    },
                    ExpectedFinalValue::Owner(None),
                );
                VerifiedTokenChange::Erc721Transfer {
                    contract: *collection,
                    from: *from,
                    to: *to,
                    token_id: *token_id,
                }
            }
            DecodedStandardEvent::Erc721Approval {
                collection,
                owner,
                approved_address,
                token_id,
            } => {
                let owner_state = read_erc721_owner(self.current, *collection, *token_id)?;
                if owner_state != Some(*owner) {
                    return Err(state_mismatch_at(
                        self.position,
                        "ERC-721 Approval owner does not own the token",
                    ));
                }
                let before = read_erc721_approval(self.previous, *collection, *token_id)?;
                let after = read_erc721_approval(self.current, *collection, *token_id)?;
                if after != *approved_address {
                    return Err(state_mismatch_at(
                        self.position,
                        "ERC-721 approval does not match Approval",
                    ));
                }
                if before == after {
                    verify_erc721_approval_call(
                        self.execution,
                        self.frame_id,
                        *collection,
                        *owner,
                        *approved_address,
                        *token_id,
                        self.position,
                    )?;
                }
                self.final_state_expectations.insert(
                    FinalStateQuery::Approved {
                        contract: *collection,
                        token_id: *token_id,
                    },
                    ExpectedFinalValue::Owner(after),
                );
                VerifiedTokenChange::Erc721Approval {
                    contract: *collection,
                    owner: *owner,
                    before,
                    after,
                    token_id: *token_id,
                }
            }
            DecodedStandardEvent::OperatorApproval {
                collection,
                owner,
                operator,
                approved,
            } => {
                let before = read_operator_approval(self.previous, *collection, *owner, *operator)?;
                let after = read_operator_approval(self.current, *collection, *owner, *operator)?;
                if after != *approved {
                    return Err(state_mismatch_at(
                        self.position,
                        "operator approval does not match event",
                    ));
                }
                if before == after {
                    verify_operator_approval_call(
                        self.execution,
                        self.frame_id,
                        *collection,
                        *owner,
                        *operator,
                        *approved,
                        self.position,
                    )?;
                }
                self.final_state_expectations.insert(
                    FinalStateQuery::Operator {
                        contract: *collection,
                        owner: *owner,
                        operator: *operator,
                    },
                    ExpectedFinalValue::Bool(after),
                );
                VerifiedTokenChange::OperatorApproval {
                    contract: *collection,
                    owner: *owner,
                    operator: *operator,
                    before,
                    after,
                }
            }
            DecodedStandardEvent::Erc1155TransferSingle {
                collection,
                operator,
                from,
                to,
                token_id,
                amount,
            } => {
                self.verify_erc1155_transfer(
                    *collection,
                    *from,
                    *to,
                    &[(*token_id, *amount)],
                    false,
                )?;
                VerifiedTokenChange::Erc1155TransferSingle {
                    contract: *collection,
                    operator: *operator,
                    from: *from,
                    to: *to,
                    token_id: *token_id,
                    amount: *amount,
                }
            }
            DecodedStandardEvent::Erc1155TransferBatch {
                collection,
                operator,
                from,
                to,
                items,
            } => {
                let items = items
                    .iter()
                    .map(|item| (item.token_id, item.raw_amount))
                    .collect::<Vec<_>>();
                self.verify_erc1155_transfer(*collection, *from, *to, &items, true)?;
                VerifiedTokenChange::Erc1155TransferBatch {
                    contract: *collection,
                    operator: *operator,
                    from: *from,
                    to: *to,
                    items,
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
    ) -> Result<(), EvmChangeDerivationError> {
        if from == Address::ZERO && to == Address::ZERO {
            return Err(state_mismatch_at(
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
                return Err(state_mismatch_at(
                    self.position,
                    "self ERC-20 Transfer changed an unexpected balance",
                ));
            }
            self.verify_erc20_transfer_call(contract, from, to, amount)?;
        } else if source_exact && target_exact && supply_exact {
            if amount == U256::ZERO {
                self.verify_erc20_transfer_call(contract, from, to, amount)?;
            }
            self.record_wrapped_effect(WrappedStateEffect::Exact);
        } else if source_unchanged && target_unchanged && supply_unchanged {
            let Some((_, pair)) = self.pair else {
                return Err(state_mismatch_at(
                    self.position,
                    "ERC-20 Transfer did not change the expected balances",
                ));
            };
            let (account, direction) = match pair.direction {
                WrappedOperation::Deposit => (to, WrappedOperation::Deposit),
                WrappedOperation::Withdrawal => (from, WrappedOperation::Withdrawal),
            };
            verify_wrapped_call_and_value(
                self.execution,
                self.frame_id,
                contract,
                account,
                amount,
                direction,
                self.position,
            )?;
            self.record_wrapped_effect(WrappedStateEffect::Unchanged);
        } else {
            return Err(state_mismatch_at(
                self.position,
                "ERC-20 Transfer did not change the expected balances",
            ));
        }

        if let Some(value) = from_after {
            self.final_state_expectations.insert(
                FinalStateQuery::Erc20Balance {
                    contract,
                    account: from,
                },
                ExpectedFinalValue::Amount(value),
            );
        }
        if let Some(value) = to_after {
            self.final_state_expectations.insert(
                FinalStateQuery::Erc20Balance {
                    contract,
                    account: to,
                },
                ExpectedFinalValue::Amount(value),
            );
        }
        if let Some(value) = total_supply_after {
            self.final_state_expectations.insert(
                FinalStateQuery::Erc20TotalSupply { contract },
                ExpectedFinalValue::Amount(value),
            );
        }
        Ok(())
    }

    fn verify_erc20_transfer_call(
        &self,
        contract: Address,
        from: Address,
        to: Address,
        amount: U256,
    ) -> Result<(), EvmChangeDerivationError> {
        if let Some((_, pair)) = self.pair {
            let account = match pair.direction {
                WrappedOperation::Deposit => to,
                WrappedOperation::Withdrawal => from,
            };
            verify_wrapped_call_and_value(
                self.execution,
                self.frame_id,
                contract,
                account,
                amount,
                pair.direction,
                self.position,
            )
        } else {
            verify_erc20_transfer_call(
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

    fn record_wrapped_effect(&mut self, effect: WrappedStateEffect) {
        record_wrapped_effect(self.wrapped_pair_proofs, self.pair, effect);
    }

    fn verify_erc1155_transfer(
        &mut self,
        contract: Address,
        from: Address,
        to: Address,
        items: &[(U256, U256)],
        batch: bool,
    ) -> Result<(), EvmChangeDerivationError> {
        if from == Address::ZERO && to == Address::ZERO {
            return Err(state_mismatch_at(
                self.position,
                "ERC-1155 transfer cannot mint to or burn from the zero address",
            ));
        }
        if items.is_empty() || from == to || items.iter().any(|(_, amount)| *amount == U256::ZERO) {
            verify_erc1155_transfer_call(
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
            *entry = entry.checked_add(amount).ok_or_else(|| {
                state_mismatch_at(self.position, "ERC-1155 batch amount overflow")
            })?;
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
                    return Err(state_mismatch_at(
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
                self.final_state_expectations.insert(
                    FinalStateQuery::Erc1155Balance {
                        contract,
                        account: from,
                        token_id,
                    },
                    ExpectedFinalValue::Amount(value),
                );
            }
            if let Some(value) = to_after {
                self.final_state_expectations.insert(
                    FinalStateQuery::Erc1155Balance {
                        contract,
                        account: to,
                        token_id,
                    },
                    ExpectedFinalValue::Amount(value),
                );
            }
        }
        Ok(())
    }
}
