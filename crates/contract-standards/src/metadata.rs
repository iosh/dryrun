use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

use alloy_primitives::{Address, Bytes, FixedBytes};
use alloy_sol_types::{SolCall, sol};
use thiserror::Error;

use crate::{
    change::legacy::{Change, PositionedChange},
    standard_decoder::{DecodedStandardEvent, DecodedStandardLog},
    state_codec::SupportsInterfaceCall,
};

pub const ERC721_METADATA_INTERFACE_ID: [u8; 4] = [0x5b, 0x5e, 0x13, 0x9f];

sol! {
    contract IContractMetadata {
        function name() external view returns (string);
        function symbol() external view returns (string);
        function decimals() external view returns (uint8);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Erc20Metadata {
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Erc721CollectionMetadata {
    pub name: Option<String>,
    pub symbol: Option<String>,
}

/// One isolated post-execution call needed to populate standard metadata.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MetadataCall<A = Address> {
    Name { contract_address: A },
    Symbol { contract_address: A },
    Decimals { contract_address: A },
}

impl<A> MetadataCall<A> {
    pub const fn contract_address(&self) -> &A {
        match self {
            Self::Name { contract_address }
            | Self::Symbol { contract_address }
            | Self::Decimals { contract_address } => contract_address,
        }
    }

    pub fn call_data(&self) -> Bytes {
        match self {
            Self::Name { .. } => name_call(),
            Self::Symbol { .. } => symbol_call(),
            Self::Decimals { .. } => decimals_call(),
        }
    }
}

/// Returns required metadata calls in first-use order, deduplicated by
/// contract and call.
pub fn metadata_calls<'a, A>(
    logs: impl IntoIterator<Item = &'a DecodedStandardLog<A>>,
) -> Vec<MetadataCall<A>>
where
    A: Clone + Eq + Hash + 'a,
{
    let mut calls = Vec::new();
    let mut seen = HashSet::new();

    for log in logs {
        match &log.event {
            DecodedStandardEvent::Erc20Transfer { token, .. }
            | DecodedStandardEvent::Erc20Approval { token, .. } => {
                push_metadata_call(
                    &mut calls,
                    &mut seen,
                    MetadataCall::Name {
                        contract_address: token.clone(),
                    },
                );
                push_metadata_call(
                    &mut calls,
                    &mut seen,
                    MetadataCall::Symbol {
                        contract_address: token.clone(),
                    },
                );
                push_metadata_call(
                    &mut calls,
                    &mut seen,
                    MetadataCall::Decimals {
                        contract_address: token.clone(),
                    },
                );
            }
            DecodedStandardEvent::Erc721Transfer { collection, .. }
            | DecodedStandardEvent::Erc721Approval { collection, .. } => {
                push_metadata_call(
                    &mut calls,
                    &mut seen,
                    MetadataCall::Name {
                        contract_address: collection.clone(),
                    },
                );
                push_metadata_call(
                    &mut calls,
                    &mut seen,
                    MetadataCall::Symbol {
                        contract_address: collection.clone(),
                    },
                );
            }
            DecodedStandardEvent::OperatorApproval { .. }
            | DecodedStandardEvent::Erc1155TransferSingle { .. }
            | DecodedStandardEvent::Erc1155TransferBatch { .. } => {}
        }
    }

    calls
}

fn push_metadata_call<A>(
    calls: &mut Vec<MetadataCall<A>>,
    seen: &mut HashSet<MetadataCall<A>>,
    call: MetadataCall<A>,
) where
    A: Clone + Eq + Hash,
{
    if seen.insert(call.clone()) {
        calls.push(call);
    }
}

/// Recorded outcomes for metadata calls.
///
/// A present `None` value means metadata was unavailable, for example because
/// the call reverted, halted, returned invalid ABI, or was skipped by the
/// caller's probe limit. An absent entry means no outcome was recorded and
/// prevents conversion into a public standard change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataValues<A: Eq + Hash = Address> {
    names: HashMap<A, Option<String>>,
    symbols: HashMap<A, Option<String>>,
    decimals: HashMap<A, Option<u8>>,
}

impl<A: Eq + Hash> Default for MetadataValues<A> {
    fn default() -> Self {
        Self {
            names: HashMap::new(),
            symbols: HashMap::new(),
            decimals: HashMap::new(),
        }
    }
}

impl<A> MetadataValues<A>
where
    A: Eq + Hash,
{
    /// Records a successful call. Invalid return data is recorded as an
    /// unavailable value rather than left unresolved.
    pub fn record_output(&mut self, call: MetadataCall<A>, output: &[u8]) {
        match call {
            MetadataCall::Name { contract_address } => {
                self.names.insert(contract_address, decode_name(output));
            }
            MetadataCall::Symbol { contract_address } => {
                self.symbols.insert(contract_address, decode_symbol(output));
            }
            MetadataCall::Decimals { contract_address } => {
                self.decimals
                    .insert(contract_address, decode_decimals(output));
            }
        }
    }

    /// Records a metadata value as unavailable.
    pub fn record_unavailable(&mut self, call: MetadataCall<A>) {
        match call {
            MetadataCall::Name { contract_address } => {
                self.names.insert(contract_address, None);
            }
            MetadataCall::Symbol { contract_address } => {
                self.symbols.insert(contract_address, None);
            }
            MetadataCall::Decimals { contract_address } => {
                self.decimals.insert(contract_address, None);
            }
        }
    }

    /// Returns ERC-20 metadata after all three getter outcomes were recorded.
    pub fn erc20_metadata(&self, contract: &A) -> Result<Erc20Metadata, MissingMetadataOutcome> {
        Ok(Erc20Metadata {
            name: self
                .names
                .get(contract)
                .ok_or(MissingMetadataOutcome)?
                .clone(),
            symbol: self
                .symbols
                .get(contract)
                .ok_or(MissingMetadataOutcome)?
                .clone(),
            decimals: *self.decimals.get(contract).ok_or(MissingMetadataOutcome)?,
        })
    }

    pub(crate) fn erc721(
        &self,
        collection: &A,
    ) -> Result<Erc721CollectionMetadata, MissingMetadataOutcome> {
        Ok(Erc721CollectionMetadata {
            name: self
                .names
                .get(collection)
                .ok_or(MissingMetadataOutcome)?
                .clone(),
            symbol: self
                .symbols
                .get(collection)
                .ok_or(MissingMetadataOutcome)?
                .clone(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("metadata call has no recorded outcome")]
pub struct MissingMetadataOutcome;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MetadataRequests {
    erc20_contracts: Vec<Address>,
    erc721_collections: Vec<Address>,
}

pub fn metadata_requests(changes: &[PositionedChange]) -> MetadataRequests {
    let mut requests = MetadataRequests::default();
    let mut seen_erc20 = HashSet::new();
    let mut seen_erc721 = HashSet::new();

    for positioned in changes {
        match &positioned.change {
            Change::Erc20Transfer {
                contract_address, ..
            }
            | Change::Erc20Mint {
                contract_address, ..
            }
            | Change::Erc20Burn {
                contract_address, ..
            }
            | Change::Erc20Allowance {
                contract_address, ..
            } => {
                if seen_erc20.insert(*contract_address) {
                    requests.erc20_contracts.push(*contract_address);
                }
            }
            Change::Erc721Transfer {
                contract_address, ..
            }
            | Change::Erc721Mint {
                contract_address, ..
            }
            | Change::Erc721Burn {
                contract_address, ..
            }
            | Change::Erc721TokenApproval {
                contract_address, ..
            }
            | Change::Erc721OperatorApproval {
                contract_address, ..
            } => {
                if seen_erc721.insert(*contract_address) {
                    requests.erc721_collections.push(*contract_address);
                }
            }
            Change::Erc1155Transfer { .. }
            | Change::Erc1155Mint { .. }
            | Change::Erc1155Burn { .. }
            | Change::Erc1155OperatorApproval { .. } => {}
        }
    }

    requests
}

impl MetadataRequests {
    pub fn erc20_contracts(&self) -> &[Address] {
        &self.erc20_contracts
    }

    pub fn erc721_collections(&self) -> &[Address] {
        &self.erc721_collections
    }

    pub fn is_empty(&self) -> bool {
        self.erc20_contracts.is_empty() && self.erc721_collections.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StandardMetadata {
    erc20: HashMap<Address, Erc20Metadata>,
    erc721: HashMap<Address, Erc721CollectionMetadata>,
}

impl StandardMetadata {
    pub fn new(
        erc20: HashMap<Address, Erc20Metadata>,
        erc721: HashMap<Address, Erc721CollectionMetadata>,
    ) -> Self {
        Self { erc20, erc721 }
    }

    pub fn erc20(&self, contract: &Address) -> Option<&Erc20Metadata> {
        self.erc20.get(contract)
    }

    pub fn erc721(&self, collection: &Address) -> Option<&Erc721CollectionMetadata> {
        self.erc721.get(collection)
    }
}

pub fn name_call() -> Bytes {
    IContractMetadata::nameCall {}.abi_encode().into()
}

pub fn decode_name(output: &[u8]) -> Option<String> {
    IContractMetadata::nameCall::abi_decode_returns_validate(output).ok()
}

pub fn symbol_call() -> Bytes {
    IContractMetadata::symbolCall {}.abi_encode().into()
}

pub fn decode_symbol(output: &[u8]) -> Option<String> {
    IContractMetadata::symbolCall::abi_decode_returns_validate(output).ok()
}

pub fn decimals_call() -> Bytes {
    IContractMetadata::decimalsCall {}.abi_encode().into()
}

pub fn decode_decimals(output: &[u8]) -> Option<u8> {
    IContractMetadata::decimalsCall::abi_decode_returns_validate(output).ok()
}

pub fn supports_interface_call(interface_id: [u8; 4]) -> Bytes {
    SupportsInterfaceCall {
        interfaceId: FixedBytes::from(interface_id),
    }
    .abi_encode()
    .into()
}

pub fn decode_supports_interface(output: &[u8]) -> Option<bool> {
    SupportsInterfaceCall::abi_decode_returns_validate(output).ok()
}
