//! Deterministic semantics for selected ABI-compatible contract standards.

mod candidate;
mod change;
mod erc1155;
mod erc20;
mod erc721;
mod error;
mod event_codec;
mod metadata;
mod operator_approval;
mod standard_decoder;
mod state;
mod state_codec;
mod token_contract;

pub(crate) use candidate::Position;
pub(crate) use candidate::StandardCandidate;
pub(crate) use candidate::StandardCandidateKind;
pub use change::{Erc1155TransferItem, StandardChange};
pub(crate) use error::ContractStandardsError;
pub(crate) use event_codec::EventCodecError;
pub use metadata::{
    Erc20Metadata, Erc721CollectionMetadata, MetadataCall, MetadataValues, MissingMetadataOutcome,
    metadata_calls,
};
pub use standard_decoder::{DecodedStandardLog, decode_standard_log};
pub(crate) use state::{
    CollectionStandards, Erc20AllowanceKey, Erc20BalanceKey, Erc721TokenKey, Erc721TokenState,
    Erc1155BalanceKey, OperatorApprovalKey, StandardStateValues, StateArithmeticOperation,
    StatePhase, StateRequirement, StateRequirements,
};

#[doc(hidden)]
pub mod legacy {
    pub use super::candidate::{
        Position, Record, StandardCandidate, collect_candidates, sort_candidates_by_position,
    };
    pub use super::change::legacy::{Change, PositionedChange};
    pub use super::error::ContractStandardsError;
    pub use super::event_codec::{EventCodecError, SupportedEvent};
    pub use super::metadata::{
        ERC721_METADATA_INTERFACE_ID, MetadataRequests, StandardMetadata, decimals_call,
        decode_decimals, decode_name, decode_supports_interface, decode_symbol, metadata_requests,
        name_call, supports_interface_call, symbol_call,
    };
    pub use super::state::{
        CollectionStandards, Erc20AllowanceKey, Erc20BalanceKey, Erc721TokenKey, Erc721TokenState,
        Erc1155BalanceKey, OperatorApprovalKey, StandardStateValues, StateArithmeticOperation,
        StatePhase, StateRequirement, StateRequirements, state_requirements,
    };
    pub use super::state_codec::{
        Erc20AllowanceCall, Erc20BalanceCall, Erc20TotalSupplyCall, Erc721GetApprovedCall,
        Erc721OwnerCall, Erc1155BalanceCall, OperatorApprovalCall, SupportsInterfaceCall,
    };

    pub const ERC165_INTERFACE_ID: [u8; 4] = [0x01, 0xff, 0xc9, 0xa7];
    pub const INVALID_ERC165_INTERFACE_ID: [u8; 4] = [0xff; 4];
    pub const ERC721_INTERFACE_ID: [u8; 4] = [0x80, 0xac, 0x58, 0xcd];
    pub const ERC1155_INTERFACE_ID: [u8; 4] = [0xd9, 0xb6, 0x7a, 0x26];

    pub fn verify(
        candidates: &[StandardCandidate],
        before: &StandardStateValues,
        after: &StandardStateValues,
    ) -> Result<Vec<PositionedChange>, ContractStandardsError> {
        let requirements = state_requirements(candidates);

        super::token_contract::check_token_contracts(candidates, &requirements, before, after)?;

        let mut changes =
            super::erc20::check_erc20_changes(candidates, &requirements, before, after)?;
        changes.extend(super::erc721::check_erc721_changes(
            candidates, before, after,
        )?);
        changes.extend(super::erc1155::check_erc1155_movements(
            candidates,
            &requirements,
            before,
            after,
        )?);
        changes.extend(super::operator_approval::check_operator_approvals(
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
}
