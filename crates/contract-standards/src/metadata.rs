use std::collections::{HashMap, HashSet};

use alloy_primitives::{Address, Bytes, FixedBytes};
use alloy_sol_types::{SolCall, sol};

use crate::{PositionedStandardChange, StandardChange};

pub const ERC721_METADATA_INTERFACE_ID: [u8; 4] = [0x5b, 0x5e, 0x13, 0x9f];

sol! {
    contract IContractMetadata {
        function name() external view returns (string);
        function symbol() external view returns (string);
        function decimals() external view returns (uint8);
        function supportsInterface(bytes4 interfaceId) external view returns (bool);
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MetadataRequests {
    erc20_contracts: Vec<Address>,
    erc721_collections: Vec<Address>,
}

impl MetadataRequests {
    pub fn from_changes(changes: &[PositionedStandardChange]) -> Self {
        let mut requests = Self::default();
        let mut seen_erc20 = HashSet::new();
        let mut seen_erc721 = HashSet::new();

        for positioned in changes {
            match &positioned.change {
                StandardChange::Erc20Transfer {
                    contract_address, ..
                }
                | StandardChange::Erc20Mint {
                    contract_address, ..
                }
                | StandardChange::Erc20Burn {
                    contract_address, ..
                }
                | StandardChange::Erc20Allowance {
                    contract_address, ..
                } => {
                    if seen_erc20.insert(*contract_address) {
                        requests.erc20_contracts.push(*contract_address);
                    }
                }
                StandardChange::Erc721Transfer {
                    contract_address, ..
                }
                | StandardChange::Erc721Mint {
                    contract_address, ..
                }
                | StandardChange::Erc721Burn {
                    contract_address, ..
                }
                | StandardChange::Erc721TokenApproval {
                    contract_address, ..
                }
                | StandardChange::Erc721OperatorApproval {
                    contract_address, ..
                } => {
                    if seen_erc721.insert(*contract_address) {
                        requests.erc721_collections.push(*contract_address);
                    }
                }
                StandardChange::Erc1155Transfer { .. }
                | StandardChange::Erc1155Mint { .. }
                | StandardChange::Erc1155Burn { .. }
                | StandardChange::Erc1155OperatorApproval { .. } => {}
            }
        }

        requests
    }

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
    IContractMetadata::nameCall::abi_decode_returns(output).ok()
}

pub fn symbol_call() -> Bytes {
    IContractMetadata::symbolCall {}.abi_encode().into()
}

pub fn decode_symbol(output: &[u8]) -> Option<String> {
    IContractMetadata::symbolCall::abi_decode_returns(output).ok()
}

pub fn decimals_call() -> Bytes {
    IContractMetadata::decimalsCall {}.abi_encode().into()
}

pub fn decode_decimals(output: &[u8]) -> Option<u8> {
    IContractMetadata::decimalsCall::abi_decode_returns(output).ok()
}

pub fn supports_interface_call(interface_id: [u8; 4]) -> Bytes {
    IContractMetadata::supportsInterfaceCall {
        interfaceId: FixedBytes::from(interface_id),
    }
    .abi_encode()
    .into()
}

pub fn decode_supports_interface(output: &[u8]) -> Option<bool> {
    IContractMetadata::supportsInterfaceCall::abi_decode_returns(output).ok()
}
