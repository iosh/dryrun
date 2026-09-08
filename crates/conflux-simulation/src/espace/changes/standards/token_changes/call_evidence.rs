use alloy::{
    primitives::{Address, Bytes, U256, keccak256},
    sol_types::{SolType, SolValue, abi::TokenSeq},
};

use crate::espace::{
    EspaceCallKind, EspaceChangesError, EspaceCommittedFrame, EspaceExecutedTransaction,
    EspaceExecutionPosition, EspaceExecutionSpace, EspaceFrameAction, EspaceFrameId,
};

use super::error::state_mismatch_at;

pub(super) fn verify_erc20_transfer_call(
    execution: &EspaceExecutedTransaction,
    frame_id: EspaceFrameId,
    contract: Address,
    from: Address,
    to: Address,
    amount: U256,
    position: EspaceExecutionPosition,
) -> Result<(), EspaceChangesError> {
    let transfer = encode_call(
        "transfer(address,uint256)",
        (to, amount).abi_encode_sequence(),
    );
    let transfer_from = encode_call(
        "transferFrom(address,address,uint256)",
        (from, to, amount).abi_encode_sequence(),
    );
    has_matching_committed_call(execution, frame_id, contract, position, |_, _, _, input| {
        input == transfer || input == transfer_from
    })
    .then_some(())
    .ok_or_else(|| state_mismatch_at(position, "ERC-20 no-op has no matching committed call"))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn verify_erc20_approval_call(
    execution: &EspaceExecutedTransaction,
    frame_id: EspaceFrameId,
    contract: Address,
    owner: Address,
    spender: Address,
    amount: U256,
    position: EspaceExecutionPosition,
) -> Result<(), EspaceChangesError> {
    let expected = encode_call(
        "approve(address,uint256)",
        (spender, amount).abi_encode_sequence(),
    );
    has_matching_committed_call(
        execution,
        frame_id,
        contract,
        position,
        |_, caller, _, input| caller == owner && input == expected,
    )
    .then_some(())
    .ok_or_else(|| state_mismatch_at(position, "ERC-20 Approval no-op has no matching call"))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn verify_erc721_transfer_call(
    execution: &EspaceExecutedTransaction,
    frame_id: EspaceFrameId,
    contract: Address,
    from: Address,
    to: Address,
    token_id: U256,
    position: EspaceExecutionPosition,
) -> Result<(), EspaceChangesError> {
    has_matching_committed_call(execution, frame_id, contract, position, |_, _, _, input| {
        matches_erc721_transfer_call(input, from, Some(to), token_id)
    })
    .then_some(())
    .ok_or_else(|| state_mismatch_at(position, "ERC-721 no-op has no matching committed call"))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn verify_erc721_approval_call(
    execution: &EspaceExecutedTransaction,
    frame_id: EspaceFrameId,
    contract: Address,
    owner: Address,
    approved: Option<Address>,
    token_id: U256,
    position: EspaceExecutionPosition,
) -> Result<(), EspaceChangesError> {
    let expected = encode_call(
        "approve(address,uint256)",
        (approved.unwrap_or(Address::ZERO), token_id).abi_encode_sequence(),
    );
    has_matching_committed_call(execution, frame_id, contract, position, |_, _, _, input| {
        input == expected || matches_erc721_transfer_call(input, owner, None, token_id)
    })
    .then_some(())
    .ok_or_else(|| state_mismatch_at(position, "ERC-721 Approval no-op has no matching call"))
}

pub(super) fn matches_erc721_transfer_call(
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
pub(super) fn verify_operator_approval_call(
    execution: &EspaceExecutedTransaction,
    frame_id: EspaceFrameId,
    contract: Address,
    owner: Address,
    operator: Address,
    approved: bool,
    position: EspaceExecutionPosition,
) -> Result<(), EspaceChangesError> {
    let expected = encode_call(
        "setApprovalForAll(address,bool)",
        (operator, approved).abi_encode_sequence(),
    );
    has_matching_committed_call(
        execution,
        frame_id,
        contract,
        position,
        |_, caller, _, input| caller == owner && input == expected,
    )
    .then_some(())
    .ok_or_else(|| state_mismatch_at(position, "operator Approval no-op has no matching call"))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn verify_erc1155_transfer_call(
    execution: &EspaceExecutedTransaction,
    frame_id: EspaceFrameId,
    contract: Address,
    from: Address,
    to: Address,
    items: &[(U256, U256)],
    batch: bool,
    position: EspaceExecutionPosition,
) -> Result<(), EspaceChangesError> {
    let matches =
        has_matching_committed_call(execution, frame_id, contract, position, |_, _, _, input| {
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
        .ok_or_else(|| state_mismatch_at(position, "ERC-1155 no-op has no matching committed call"))
}

pub(super) fn has_matching_committed_call(
    execution: &EspaceExecutedTransaction,
    frame_id: EspaceFrameId,
    contract: Address,
    position: EspaceExecutionPosition,
    matches: impl Fn(EspaceCallKind, Address, U256, &[u8]) -> bool,
) -> bool {
    execution.committed_frames().iter().any(|frame| {
        let EspaceFrameAction::Call {
            kind,
            caller,
            target,
            code_address,
            value,
            calldata,
        } = frame.action()
        else {
            return false;
        };
        frame.space() == EspaceExecutionSpace::Espace
            && frames_are_nested(execution, frame.id(), frame_id)
            && frame.position().index() <= position.index()
            && (*target == contract || *code_address == contract)
            && matches(*kind, *caller, *value, calldata)
    })
}

pub(super) fn has_matching_value_call(
    execution: &EspaceExecutedTransaction,
    frame_id: EspaceFrameId,
    from: Address,
    to: Address,
    amount: U256,
    position: EspaceExecutionPosition,
) -> bool {
    execution.committed_frames().iter().any(|frame| {
        let EspaceFrameAction::Call {
            kind,
            caller,
            target,
            value,
            ..
        } = frame.action()
        else {
            return false;
        };
        frame.space() == EspaceExecutionSpace::Espace
            && *kind == EspaceCallKind::Call
            && frames_are_nested(execution, frame.id(), frame_id)
            && frame.position().index() <= position.index()
            && *caller == from
            && *target == to
            && *value == amount
    })
}

pub(super) fn frames_are_nested(
    execution: &EspaceExecutedTransaction,
    first: EspaceFrameId,
    second: EspaceFrameId,
) -> bool {
    frame_is_ancestor(execution, first, second) || frame_is_ancestor(execution, second, first)
}

pub(super) fn frame_is_ancestor(
    execution: &EspaceExecutedTransaction,
    ancestor: EspaceFrameId,
    descendant: EspaceFrameId,
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
            .and_then(EspaceCommittedFrame::parent);
    }
    false
}

pub(super) fn encode_call(signature: &str, encoded_arguments: Vec<u8>) -> Vec<u8> {
    let mut input = Vec::with_capacity(4 + encoded_arguments.len());
    input.extend_from_slice(&selector(signature));
    input.extend(encoded_arguments);
    input
}

pub(super) fn decode_call<T>(input: &[u8], expected_selector: [u8; 4]) -> Option<T>
where
    T: SolValue,
    for<'a> <T::SolType as SolType>::Token<'a>: TokenSeq<'a>,
    T: From<<T::SolType as SolType>::RustType>,
{
    let arguments = input.strip_prefix(expected_selector.as_slice())?;
    T::abi_decode_sequence_validate(arguments).ok()
}

pub(super) fn selector(signature: &str) -> [u8; 4] {
    let hash = keccak256(signature);
    [hash[0], hash[1], hash[2], hash[3]]
}
