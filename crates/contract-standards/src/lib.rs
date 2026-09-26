//! Deterministic semantics for selected ABI-compatible contract standards.

mod change;
mod event_codec;
pub mod getter_abi;
mod metadata;
mod revert;
mod standard_decoder;

pub use change::{Erc1155TransferItem, StandardChange, StandardEffect};
pub use event_codec::{StandardEventDecodeError, is_supported_event_topic, supported_event_topics};
pub use metadata::{
    Erc20Metadata, Erc721CollectionMetadata, MetadataKind, MetadataReader, MetadataStore,
    TokenMetadata, load_metadata,
};
pub use revert::SolidityRevertReason;
pub use standard_decoder::{DecodedStandardEvent, DecodedStandardLog, decode_standard_log};

pub mod analysis;
