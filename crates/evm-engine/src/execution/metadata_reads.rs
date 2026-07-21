use std::collections::HashMap;

use alloy::{sol, sol_types::SolCall};
use alloy_primitives::{Address, FixedBytes};

use crate::{
    Erc20Metadata, Erc721CollectionMetadata, EvmTransaction, NativeMetadata,
    changes::{ChangeMetadata, ChangeMetadataRequests},
};

use super::{
    MainnetAlloyEvm,
    read_call::{execute_optional_read_call, with_read_call_context},
};

const ERC721_METADATA_INTERFACE_ID: [u8; 4] = [0x5b, 0x5e, 0x13, 0x9f];

sol! {
    contract IERC20Metadata {
        function name() external view returns (string);
        function symbol() external view returns (string);
        function decimals() external view returns (uint8);
    }

    contract IERC721Metadata {
        function name() external view returns (string);
        function symbol() external view returns (string);
    }

    contract IERC165 {
        function supportsInterface(bytes4 interfaceId) external view returns (bool);
    }
}

pub(super) fn load_change_metadata<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    requests: ChangeMetadataRequests,
) -> ChangeMetadata {
    let native = native_metadata(chain_id);

    if requests.erc20_contracts.is_empty() && requests.erc721_collections.is_empty() {
        return ChangeMetadata::new(native, HashMap::new(), HashMap::new());
    }

    with_read_call_context(evm, |evm| {
        read_change_metadata(evm, transaction, chain_id, native, requests)
    })
}

fn read_change_metadata<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    native: NativeMetadata,
    requests: ChangeMetadataRequests,
) -> ChangeMetadata {
    let mut erc20 = HashMap::new();
    let mut erc721 = HashMap::new();

    for contract in requests.erc20_contracts {
        erc20.insert(
            contract,
            read_erc20_metadata(evm, transaction, chain_id, contract),
        );
    }

    for collection in requests.erc721_collections {
        erc721.insert(
            collection,
            read_erc721_collection_metadata(evm, transaction, chain_id, collection),
        );
    }

    ChangeMetadata::new(native, erc20, erc721)
}

fn native_metadata(chain_id: u64) -> NativeMetadata {
    match chain_id {
        1 => NativeMetadata {
            name: Some("Ether".to_string()),
            symbol: Some("ETH".to_string()),
            decimals: Some(18),
        },
        _ => NativeMetadata::default(),
    }
}

fn read_erc20_metadata<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    contract: Address,
) -> Erc20Metadata {
    // Contract metadata is optional. Individual read failures leave only that field absent.
    Erc20Metadata {
        name: read_erc20_name(evm, transaction, chain_id, contract),
        symbol: read_erc20_symbol(evm, transaction, chain_id, contract),
        decimals: read_erc20_decimals(evm, transaction, chain_id, contract),
    }
}

fn read_erc721_collection_metadata<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    collection: Address,
) -> Erc721CollectionMetadata {
    let supports_metadata = read_interface_support(
        evm,
        transaction,
        chain_id,
        collection,
        ERC721_METADATA_INTERFACE_ID,
    );

    if supports_metadata != Some(true) {
        return Erc721CollectionMetadata::default();
    }

    Erc721CollectionMetadata {
        name: read_erc721_name(evm, transaction, chain_id, collection),
        symbol: read_erc721_symbol(evm, transaction, chain_id, collection),
    }
}

fn read_interface_support<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    contract: Address,
    interface_id: [u8; 4],
) -> Option<bool> {
    let output = execute_optional_read_call(
        evm,
        transaction,
        chain_id,
        contract,
        IERC165::supportsInterfaceCall {
            interfaceId: FixedBytes::<4>::from(interface_id),
        }
        .abi_encode()
        .into(),
    )?;

    IERC165::supportsInterfaceCall::abi_decode_returns(output.as_ref()).ok()
}

fn read_erc20_name<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    contract: Address,
) -> Option<String> {
    let output = execute_optional_read_call(
        evm,
        transaction,
        chain_id,
        contract,
        IERC20Metadata::nameCall {}.abi_encode().into(),
    )?;

    IERC20Metadata::nameCall::abi_decode_returns(output.as_ref()).ok()
}

fn read_erc20_symbol<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    contract: Address,
) -> Option<String> {
    let output = execute_optional_read_call(
        evm,
        transaction,
        chain_id,
        contract,
        IERC20Metadata::symbolCall {}.abi_encode().into(),
    )?;

    IERC20Metadata::symbolCall::abi_decode_returns(output.as_ref()).ok()
}

fn read_erc20_decimals<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    contract: Address,
) -> Option<u8> {
    let output = execute_optional_read_call(
        evm,
        transaction,
        chain_id,
        contract,
        IERC20Metadata::decimalsCall {}.abi_encode().into(),
    )?;

    IERC20Metadata::decimalsCall::abi_decode_returns(output.as_ref()).ok()
}

fn read_erc721_name<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    collection: Address,
) -> Option<String> {
    let output = execute_optional_read_call(
        evm,
        transaction,
        chain_id,
        collection,
        IERC721Metadata::nameCall {}.abi_encode().into(),
    )?;

    IERC721Metadata::nameCall::abi_decode_returns(output.as_ref()).ok()
}

fn read_erc721_symbol<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    collection: Address,
) -> Option<String> {
    let output = execute_optional_read_call(
        evm,
        transaction,
        chain_id,
        collection,
        IERC721Metadata::symbolCall {}.abi_encode().into(),
    )?;

    IERC721Metadata::symbolCall::abi_decode_returns(output.as_ref()).ok()
}
