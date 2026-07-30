use std::collections::HashMap;

use alloy_primitives::Address;
use contract_standards::{
    ERC721_METADATA_INTERFACE_ID, Erc20Metadata, Erc721CollectionMetadata, MetadataRequests,
    StandardMetadata, decimals_call, decode_decimals, decode_name, decode_supports_interface,
    decode_symbol, name_call, supports_interface_call, symbol_call,
};

use crate::{EvmTransaction, NativeMetadata, changes::ChangeMetadata};

use super::{
    MainnetAlloyEvm,
    read_call::{execute_optional_read_call, with_read_call_context},
};

pub(super) fn load_change_metadata<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    requests: MetadataRequests,
) -> ChangeMetadata {
    let native = native_metadata(chain_id);

    if requests.is_empty() {
        return ChangeMetadata::new(native, StandardMetadata::default());
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
    requests: MetadataRequests,
) -> ChangeMetadata {
    let mut erc20 = HashMap::new();
    let mut erc721 = HashMap::new();

    for &contract in requests.erc20_contracts() {
        erc20.insert(
            contract,
            read_erc20_metadata(evm, transaction, chain_id, contract),
        );
    }

    for &collection in requests.erc721_collections() {
        erc721.insert(
            collection,
            read_erc721_collection_metadata(evm, transaction, chain_id, collection),
        );
    }

    ChangeMetadata::new(native, StandardMetadata::new(erc20, erc721))
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
        supports_interface_call(interface_id),
    )?;

    decode_supports_interface(output.as_ref())
}

fn read_erc20_name<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    contract: Address,
) -> Option<String> {
    let output = execute_optional_read_call(evm, transaction, chain_id, contract, name_call())?;

    decode_name(output.as_ref())
}

fn read_erc20_symbol<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    contract: Address,
) -> Option<String> {
    let output = execute_optional_read_call(evm, transaction, chain_id, contract, symbol_call())?;

    decode_symbol(output.as_ref())
}

fn read_erc20_decimals<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    contract: Address,
) -> Option<u8> {
    let output = execute_optional_read_call(evm, transaction, chain_id, contract, decimals_call())?;

    decode_decimals(output.as_ref())
}

fn read_erc721_name<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    collection: Address,
) -> Option<String> {
    let output = execute_optional_read_call(evm, transaction, chain_id, collection, name_call())?;

    decode_name(output.as_ref())
}

fn read_erc721_symbol<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    collection: Address,
) -> Option<String> {
    let output = execute_optional_read_call(evm, transaction, chain_id, collection, symbol_call())?;

    decode_symbol(output.as_ref())
}
