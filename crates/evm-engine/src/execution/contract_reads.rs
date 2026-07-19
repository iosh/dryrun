use std::collections::HashMap;

use alloy::{sol, sol_types::SolCall};
use alloy_primitives::{Address, FixedBytes};

use crate::{
    EvmTransaction,
    transaction_changes::{
        ChangeData, ChangeDataRequests, ContractKind, Erc20Metadata, Erc721CollectionMetadata,
    },
};

use super::{
    MainnetAlloyEvm,
    read_call::{execute_optional_read_call, with_read_call_context},
};

const ERC721_INTERFACE_ID: [u8; 4] = [0x80, 0xac, 0x58, 0xcd];
const ERC721_METADATA_INTERFACE_ID: [u8; 4] = [0x5b, 0x5e, 0x13, 0x9f];
const ERC1155_INTERFACE_ID: [u8; 4] = [0xd9, 0xb6, 0x7a, 0x26];

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

pub(super) fn load_change_data<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    requests: ChangeDataRequests,
) -> ChangeData {
    with_read_call_context(evm, |evm| {
        read_change_data(evm, transaction, chain_id, requests)
    })
}

fn read_change_data<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    requests: ChangeDataRequests,
) -> ChangeData {
    let ChangeDataRequests {
        contract_kinds: contracts_to_classify,
        erc20_metadata: erc20_tokens,
        erc721_collection_metadata: erc721_metadata_requests,
    } = requests;

    let mut contract_kinds = HashMap::new();
    let mut erc20_metadata = HashMap::new();
    let mut erc721_collection_metadata = HashMap::new();

    for contract in contracts_to_classify {
        contract_kinds.insert(
            contract,
            classify_contract(evm, transaction, chain_id, contract),
        );
    }

    for token in erc20_tokens {
        erc20_metadata.insert(
            token,
            read_erc20_metadata(evm, transaction, chain_id, token),
        );
    }

    for metadata_request in erc721_metadata_requests {
        if metadata_request.only_if_classified_as_erc721
            && contract_kinds.get(&metadata_request.collection) != Some(&ContractKind::Erc721)
        {
            continue;
        }

        erc721_collection_metadata.insert(
            metadata_request.collection,
            read_erc721_collection_metadata(
                evm,
                transaction,
                chain_id,
                metadata_request.collection,
            ),
        );
    }

    ChangeData::new(contract_kinds, erc20_metadata, erc721_collection_metadata)
}

fn classify_contract<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    contract: Address,
) -> ContractKind {
    // ERC20 has no standard ERC165 interface id, so this is a best-effort
    // NFT-vs-fungible classification rather than a strict proof.
    let supports_erc721 =
        read_interface_support(evm, transaction, chain_id, contract, ERC721_INTERFACE_ID);
    if supports_erc721 == Some(true) {
        return ContractKind::Erc721;
    }

    let supports_erc1155 =
        read_interface_support(evm, transaction, chain_id, contract, ERC1155_INTERFACE_ID);
    if supports_erc1155 == Some(true) {
        return ContractKind::Erc1155;
    }

    if supports_erc721.is_none() && supports_erc1155.is_none() {
        ContractKind::Unknown
    } else {
        ContractKind::FungibleLike
    }
}

fn read_erc20_metadata<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    token: Address,
) -> Erc20Metadata {
    // Contract metadata is optional. Individual read failures leave only that field absent.
    Erc20Metadata {
        name: read_erc20_name(evm, transaction, chain_id, token),
        symbol: read_erc20_symbol(evm, transaction, chain_id, token),
        decimals: read_erc20_decimals(evm, transaction, chain_id, token),
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
    token: Address,
) -> Option<String> {
    let output = execute_optional_read_call(
        evm,
        transaction,
        chain_id,
        token,
        IERC20Metadata::nameCall {}.abi_encode().into(),
    )?;

    IERC20Metadata::nameCall::abi_decode_returns(output.as_ref()).ok()
}

fn read_erc20_symbol<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    token: Address,
) -> Option<String> {
    let output = execute_optional_read_call(
        evm,
        transaction,
        chain_id,
        token,
        IERC20Metadata::symbolCall {}.abi_encode().into(),
    )?;

    IERC20Metadata::symbolCall::abi_decode_returns(output.as_ref()).ok()
}

fn read_erc20_decimals<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    token: Address,
) -> Option<u8> {
    let output = execute_optional_read_call(
        evm,
        transaction,
        chain_id,
        token,
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
