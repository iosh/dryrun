use std::collections::HashMap;

use alloy::{
    primitives::{Address, U256},
    sol_types::SolValue,
};

use crate::espace::{
    EspaceCallKind, EspaceChangesError, EspaceExecutedTransaction, EspaceExecutionPosition,
    EspaceFrameId, EspaceStateAccess, EspaceStateReader,
};

use super::{
    call_evidence::{encode_call, has_matching_committed_call, has_matching_value_call, selector},
    error::{state_mismatch_at, token_change_error},
    event_verification::TokenEventVerification,
    events::{
        ObservedTokenEvent, WrappedEventPair, WrappedOperation, WrappedPairProof,
        WrappedStateEffect,
    },
    state_queries::{
        read_allowance, read_erc20_balance, read_erc20_total_supply, read_erc721_approval,
        read_erc721_approval_optional, read_erc721_owner, read_erc1155_balance,
        read_operator_approval,
    },
};

pub(super) fn verify_event(
    event_index: usize,
    event: &ObservedTokenEvent<'_>,
    execution: &EspaceExecutedTransaction,
    state: &EspaceStateAccess,
    pairs: &[WrappedEventPair],
    wrapped_pair_proofs: &mut HashMap<usize, WrappedPairProof>,
    final_state_expectations: &mut HashMap<FinalStateQuery, ExpectedFinalValue>,
) -> Result<(), EspaceChangesError> {
    let pair = pairs.iter().enumerate().find_map(|(index, pair)| {
        (pair.transfer_event_index == event_index || pair.wrapped_event_index == event_index)
            .then_some((index, *pair))
    });
    match event {
        ObservedTokenEvent::Standard {
            occurrence,
            decoded,
        } => {
            let around = state.around(occurrence.handle())?;
            TokenEventVerification {
                position: occurrence.position(),
                execution,
                previous: around.previous(),
                current: around.current(),
                frame_id: occurrence.frame_id(),
                pair,
                wrapped_pair_proofs,
                final_state_expectations,
            }
            .verify_standard_event(decoded.event())
        }
        ObservedTokenEvent::Wrapped {
            occurrence,
            contract,
            account,
            amount,
            direction,
        } => {
            let around = state.around(occurrence.handle())?;
            let (after, total_supply_after) = verify_wrapped_event(
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
                wrapped_pair_proofs,
            )?;
            final_state_expectations.insert(
                FinalStateQuery::Erc20Balance {
                    contract: *contract,
                    account: *account,
                },
                ExpectedFinalValue::Amount(after),
            );
            final_state_expectations.insert(
                FinalStateQuery::Erc20TotalSupply {
                    contract: *contract,
                },
                ExpectedFinalValue::Amount(total_supply_after),
            );
            Ok(())
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn verify_wrapped_event(
    execution: &EspaceExecutedTransaction,
    frame_id: EspaceFrameId,
    position: EspaceExecutionPosition,
    previous: &EspaceStateReader,
    current: &EspaceStateReader,
    contract: Address,
    account: Address,
    amount: U256,
    direction: WrappedOperation,
    pair: Option<(usize, WrappedEventPair)>,
    wrapped_pair_proofs: &mut HashMap<usize, WrappedPairProof>,
) -> Result<(U256, U256), EspaceChangesError> {
    verify_wrapped_call_and_value(
        execution, frame_id, contract, account, amount, direction, position,
    )?;
    let before = read_erc20_balance(previous, contract, account)?;
    let after = read_erc20_balance(current, contract, account)?;
    let total_supply_before = read_erc20_total_supply(previous, contract)?;
    let total_supply_after = read_erc20_total_supply(current, contract)?;
    let expected = match direction {
        WrappedOperation::Deposit => before.checked_add(amount),
        WrappedOperation::Withdrawal => before.checked_sub(amount),
    };
    let expected_total_supply = match direction {
        WrappedOperation::Deposit => total_supply_before.checked_add(amount),
        WrappedOperation::Withdrawal => total_supply_before.checked_sub(amount),
    };

    if expected == Some(after) && expected_total_supply == Some(total_supply_after) {
        record_wrapped_effect(wrapped_pair_proofs, pair, WrappedStateEffect::Exact);
        return Ok((after, total_supply_after));
    }

    if before != after || total_supply_before != total_supply_after {
        return Err(state_mismatch_at(
            position,
            "wrapped-native balance or total supply does not match Deposit/Withdrawal",
        ));
    }

    let Some(pair) = pair else {
        return Err(state_mismatch_at(
            position,
            "wrapped-native event did not change the expected balance",
        ));
    };
    record_wrapped_effect(
        wrapped_pair_proofs,
        Some(pair),
        WrappedStateEffect::Unchanged,
    );
    Ok((after, total_supply_after))
}

pub(super) fn verify_wrapped_call_and_value(
    execution: &EspaceExecutedTransaction,
    frame_id: EspaceFrameId,
    contract: Address,
    account: Address,
    amount: U256,
    direction: WrappedOperation,
    position: EspaceExecutionPosition,
) -> Result<(), EspaceChangesError> {
    let has_evidence = match direction {
        WrappedOperation::Deposit => has_matching_committed_call(
            execution,
            frame_id,
            contract,
            position,
            |kind, caller, value, input| {
                kind == EspaceCallKind::Call
                    && caller == account
                    && value == amount
                    && (input.is_empty() || input == selector("deposit()").as_slice())
            },
        ),
        WrappedOperation::Withdrawal => {
            let input = encode_call("withdraw(uint256)", (amount,).abi_encode_sequence());
            has_matching_committed_call(
                execution,
                frame_id,
                contract,
                position,
                |_, caller, _, actual| caller == account && actual == input,
            ) && has_matching_value_call(execution, frame_id, contract, account, amount, position)
        }
    };
    has_evidence.then_some(()).ok_or_else(|| {
        state_mismatch_at(position, "wrapped-native event has no matching value flow")
    })
}

pub(super) fn record_wrapped_effect(
    evidence: &mut HashMap<usize, WrappedPairProof>,
    pair: Option<(usize, WrappedEventPair)>,
    effect: WrappedStateEffect,
) {
    if let Some((pair_index, _)) = pair {
        evidence.entry(pair_index).or_default().record(effect);
    }
}

pub(super) fn expect_increase(
    before: U256,
    after: U256,
    amount: U256,
    position: EspaceExecutionPosition,
    label: &'static str,
) -> Result<(), EspaceChangesError> {
    let expected = before
        .checked_add(amount)
        .ok_or_else(|| state_mismatch_at(position, "balance increase overflow"))?;
    (after == expected)
        .then_some(())
        .ok_or_else(|| state_mismatch_at(position, label))
}

pub(super) fn expect_decrease(
    before: U256,
    after: U256,
    amount: U256,
    position: EspaceExecutionPosition,
    label: &'static str,
) -> Result<(), EspaceChangesError> {
    let expected = before
        .checked_sub(amount)
        .ok_or_else(|| state_mismatch_at(position, "balance decrease underflow"))?;
    (after == expected)
        .then_some(())
        .ok_or_else(|| state_mismatch_at(position, label))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum FinalStateQuery {
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
pub(super) enum ExpectedFinalValue {
    Amount(U256),
    Owner(Option<Address>),
    Bool(bool),
}

pub(super) fn verify_final_state(
    expectations: &HashMap<FinalStateQuery, ExpectedFinalValue>,
    state: &EspaceStateAccess,
) -> Result<(), EspaceChangesError> {
    for (query, expected) in expectations {
        let actual = match query {
            FinalStateQuery::Erc20Balance { contract, account } => ExpectedFinalValue::Amount(
                read_erc20_balance(state.finalized(), *contract, *account)?,
            ),
            FinalStateQuery::Erc20TotalSupply { contract } => {
                ExpectedFinalValue::Amount(read_erc20_total_supply(state.finalized(), *contract)?)
            }
            FinalStateQuery::Erc1155Balance {
                contract,
                account,
                token_id,
            } => ExpectedFinalValue::Amount(read_erc1155_balance(
                state.finalized(),
                *contract,
                *account,
                *token_id,
            )?),
            FinalStateQuery::Allowance {
                contract,
                owner,
                spender,
            } => ExpectedFinalValue::Amount(read_allowance(
                state.finalized(),
                *contract,
                *owner,
                *spender,
            )?),
            FinalStateQuery::Owner { contract, token_id } => ExpectedFinalValue::Owner(
                read_erc721_owner(state.finalized(), *contract, *token_id)?,
            ),
            FinalStateQuery::Approved { contract, token_id } => {
                let owner_key = FinalStateQuery::Owner {
                    contract: *contract,
                    token_id: *token_id,
                };
                let token_is_missing =
                    expectations.get(&owner_key) == Some(&ExpectedFinalValue::Owner(None));
                let approved = if token_is_missing {
                    read_erc721_approval_optional(state.finalized(), *contract, *token_id)?
                } else {
                    read_erc721_approval(state.finalized(), *contract, *token_id)?
                };
                ExpectedFinalValue::Owner(approved)
            }
            FinalStateQuery::Operator {
                contract,
                owner,
                operator,
            } => ExpectedFinalValue::Bool(read_operator_approval(
                state.finalized(),
                *contract,
                *owner,
                *operator,
            )?),
        };
        if &actual != expected {
            return Err(token_change_error(
                "finalized state differs from the last verified occurrence",
            ));
        }
    }
    Ok(())
}
