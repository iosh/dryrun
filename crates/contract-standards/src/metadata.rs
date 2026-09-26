use alloy_primitives::{Address, Bytes};
use alloy_sol_types::{SolCall, sol};
use std::collections::BTreeMap;

sol! {
    contract IContractMetadata {
        function name() external view returns (string);
        function symbol() external view returns (string);
        function decimals() external view returns (uint8);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Erc20Metadata {
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Erc721CollectionMetadata {
    pub name: Option<String>,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataKind {
    Erc20,
    Erc721,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum TokenMetadata {
    Erc20(Erc20Metadata),
    Erc721(Erc721CollectionMetadata),
}

/// One owned display record per asset. Changes and serializers borrow it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataStore<A: Ord = Address> {
    values: BTreeMap<A, TokenMetadata>,
}

impl<A: Ord> Default for MetadataStore<A> {
    fn default() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }
}

impl<A: Ord> MetadataStore<A> {
    pub fn get(&self, address: &A) -> Option<&TokenMetadata> {
        self.values.get(address)
    }
    pub fn insert(&mut self, address: A, value: TokenMetadata) {
        self.values.insert(address, value);
    }
}

/// Executes optional display getters against the selected final state.
/// A revert or halt has no metadata value; state and resource errors must propagate.
pub trait MetadataReader<A> {
    type Error;

    fn metadata_call(&self, address: &A, input: Bytes) -> Result<Option<Bytes>, Self::Error>;
}

pub fn load_metadata<A: Ord, R: MetadataReader<A> + ?Sized>(
    reader: &R,
    assets: impl IntoIterator<Item = (A, MetadataKind)>,
) -> Result<MetadataStore<A>, R::Error> {
    let assets: BTreeMap<_, _> = assets.into_iter().collect();
    let mut values = MetadataStore::default();
    for (address, kind) in assets {
        let name = read_optional(reader, &address, IContractMetadata::nameCall {})?;
        let symbol = read_optional(reader, &address, IContractMetadata::symbolCall {})?;
        let metadata = match kind {
            MetadataKind::Erc20 => TokenMetadata::Erc20(Erc20Metadata {
                name,
                symbol,
                decimals: read_optional(reader, &address, IContractMetadata::decimalsCall {})?,
            }),
            MetadataKind::Erc721 => {
                TokenMetadata::Erc721(Erc721CollectionMetadata { name, symbol })
            }
        };
        values.insert(address, metadata);
    }
    Ok(values)
}

fn read_optional<A, R: MetadataReader<A> + ?Sized, C: SolCall>(
    reader: &R,
    address: &A,
    call: C,
) -> Result<Option<C::Return>, R::Error> {
    Ok(reader
        .metadata_call(address, call.abi_encode().into())?
        .and_then(|output| C::abi_decode_returns_validate(&output).ok()))
}
