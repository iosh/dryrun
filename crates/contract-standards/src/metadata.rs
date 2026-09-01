use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

use alloy_primitives::{Address, Bytes};
use alloy_sol_types::{SolCall, sol};
use thiserror::Error;

use crate::standard_decoder::{DecodedStandardEvent, DecodedStandardLog};

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

    /// Returns ERC-721 collection metadata after both getter outcomes were recorded.
    pub fn erc721_collection_metadata(
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

    pub(crate) fn erc721(
        &self,
        collection: &A,
    ) -> Result<Erc721CollectionMetadata, MissingMetadataOutcome> {
        self.erc721_collection_metadata(collection)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("metadata call has no recorded outcome")]
pub struct MissingMetadataOutcome;

fn name_call() -> Bytes {
    IContractMetadata::nameCall {}.abi_encode().into()
}

fn decode_name(output: &[u8]) -> Option<String> {
    IContractMetadata::nameCall::abi_decode_returns_validate(output).ok()
}

fn symbol_call() -> Bytes {
    IContractMetadata::symbolCall {}.abi_encode().into()
}

fn decode_symbol(output: &[u8]) -> Option<String> {
    IContractMetadata::symbolCall::abi_decode_returns_validate(output).ok()
}

fn decimals_call() -> Bytes {
    IContractMetadata::decimalsCall {}.abi_encode().into()
}

fn decode_decimals(output: &[u8]) -> Option<u8> {
    IContractMetadata::decimalsCall::abi_decode_returns_validate(output).ok()
}
