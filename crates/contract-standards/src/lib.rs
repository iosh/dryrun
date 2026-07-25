//! Deterministic semantics for selected ABI-compatible contract standards.

mod candidate;
mod change;
mod erc1155;
mod erc20;
mod erc721;
mod error;
mod event_codec;
mod operator_approval;
mod state;
mod token_contract;

pub(crate) use candidate::StandardCandidateKind;
pub use candidate::{
    Position, Record, StandardCandidate, collect_candidates, sort_candidates_by_position,
};
pub use change::{PositionedStandardChange, StandardChange};
pub use error::ContractStandardsError;
pub use event_codec::{EventCodecError, SupportedEvent};
pub use state::{
    CollectionStandards, Erc20AllowanceKey, Erc20BalanceKey, Erc721TokenKey, Erc721TokenState,
    Erc1155BalanceKey, OperatorApprovalKey, StandardStateValues, StateArithmeticOperation,
    StatePhase, StateRequirement, StateRequirements, state_requirements,
};

pub const ERC165_INTERFACE_ID: [u8; 4] = [0x01, 0xff, 0xc9, 0xa7];
pub const INVALID_ERC165_INTERFACE_ID: [u8; 4] = [0xff; 4];
pub const ERC721_INTERFACE_ID: [u8; 4] = [0x80, 0xac, 0x58, 0xcd];
pub const ERC1155_INTERFACE_ID: [u8; 4] = [0xd9, 0xb6, 0x7a, 0x26];

pub fn verify(
    candidates: &[StandardCandidate],
    before: &StandardStateValues,
    after: &StandardStateValues,
) -> Result<Vec<PositionedStandardChange>, ContractStandardsError> {
    let requirements = state_requirements(candidates);

    token_contract::check_token_contracts(candidates, &requirements, before, after)?;

    let mut changes = erc20::check_erc20_changes(candidates, &requirements, before, after)?;
    changes.extend(erc721::check_erc721_changes(candidates, before, after)?);
    changes.extend(erc1155::check_erc1155_movements(
        candidates,
        &requirements,
        before,
        after,
    )?);
    changes.extend(operator_approval::check_operator_approvals(
        candidates, before, after,
    )?);

    Ok(changes)
}

pub fn validate_collection_standards(
    collection: alloy_primitives::Address,
    supports_erc165: bool,
    supports_invalid_interface: bool,
    supports_erc721: bool,
    supports_erc1155: bool,
) -> Result<CollectionStandards, ContractStandardsError> {
    if !supports_erc165 {
        return Err(ContractStandardsError::CollectionDoesNotSupportErc165 { collection });
    }

    if supports_invalid_interface {
        return Err(
            ContractStandardsError::CollectionSupportsInvalidErc165Interface { collection },
        );
    }

    Ok(CollectionStandards {
        supports_erc721,
        supports_erc1155,
    })
}

#[cfg(test)]
mod tests;
