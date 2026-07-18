use std::collections::HashMap;

use alloy::{sol, sol_types::SolCall};
use alloy_primitives::{Address, Bytes, FixedBytes, U256};
use revm::{
    ExecuteEvm,
    context::{CfgEnv, TxEnv},
    context_interface::{
        result::ExecutionResult,
        transaction::{AccessList as RevmAccessList, TransactionType},
    },
    handler::EvmTr,
    primitives::TxKind,
};

use crate::{
    Change, EvmEngineError, EvmExecutionStatus, EvmTransaction,
    execution::{ExecutionArtifacts, MainnetAlloyEvm},
    transaction_changes::{
        ChangeCandidate, ChangeCandidateKind, ContractKind, CurrentChangeFacts,
        Erc20AllowanceEvidence, Erc20Metadata, Erc721CollectionMetadata, build_current_changes,
        collect_candidates,
    },
};

const METADATA_CALL_GAS_LIMIT: u64 = 100_000;

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

pub(super) fn collect_change_candidates(
    artifacts: &ExecutionArtifacts,
) -> Result<Vec<ChangeCandidate>, EvmEngineError> {
    if !matches!(artifacts.status, EvmExecutionStatus::Success) {
        return Ok(Vec::new());
    }

    collect_candidates(&artifacts.observations).map_err(|error| {
        EvmEngineError::analysis_failed(format!("transaction changes failed: {error}"))
    })
}

pub(super) fn build_changes<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    artifacts: &ExecutionArtifacts,
    transaction: &EvmTransaction,
    candidates: Vec<ChangeCandidate>,
) -> Vec<Change> {
    if candidates.is_empty() {
        return Vec::new();
    }

    configure_metadata_call_context(evm);
    let facts = load_current_change_facts(evm, transaction, artifacts.chain_id, &candidates);

    build_current_changes(candidates, &facts)
}

fn load_current_change_facts<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    execution_chain_id: u64,
    candidates: &[ChangeCandidate],
) -> CurrentChangeFacts {
    let mut contract_kinds = HashMap::new();
    let mut erc20_metadata = HashMap::new();
    let mut erc721_collection_metadata = HashMap::new();

    for candidate in candidates {
        match &candidate.kind {
            ChangeCandidateKind::Erc20Transfer { token, .. }
            | ChangeCandidateKind::Erc20Allowance {
                token,
                evidence: Erc20AllowanceEvidence::ApprovalEvent { .. },
                ..
            } => {
                if !erc20_metadata.contains_key(token) {
                    erc20_metadata.insert(
                        *token,
                        load_erc20_metadata(evm, transaction, execution_chain_id, *token),
                    );
                }
            }
            ChangeCandidateKind::Erc721Transfer { collection, .. }
            | ChangeCandidateKind::Erc721Approval { collection, .. } => {
                if !erc721_collection_metadata.contains_key(collection) {
                    erc721_collection_metadata.insert(
                        *collection,
                        load_erc721_collection_metadata(
                            evm,
                            transaction,
                            execution_chain_id,
                            *collection,
                        ),
                    );
                }
            }
            ChangeCandidateKind::OperatorApproval { collection, .. } => {
                let kind = if let Some(kind) = contract_kinds.get(collection) {
                    *kind
                } else {
                    let kind =
                        resolve_contract_kind(evm, transaction, execution_chain_id, *collection);
                    contract_kinds.insert(*collection, kind);
                    kind
                };

                if kind == ContractKind::Erc721
                    && !erc721_collection_metadata.contains_key(collection)
                {
                    erc721_collection_metadata.insert(
                        *collection,
                        load_erc721_collection_metadata(
                            evm,
                            transaction,
                            execution_chain_id,
                            *collection,
                        ),
                    );
                }
            }
            ChangeCandidateKind::NativeTransfer { .. }
            | ChangeCandidateKind::Erc1155Transfer { .. }
            | ChangeCandidateKind::Erc20Allowance {
                evidence: Erc20AllowanceEvidence::TransferFromCall { .. },
                ..
            } => {}
        }
    }

    CurrentChangeFacts::new(contract_kinds, erc20_metadata, erc721_collection_metadata)
}

fn configure_relaxed_call_cfg(cfg: &mut CfgEnv) {
    cfg.disable_nonce_check = true;
    cfg.disable_balance_check = true;
    cfg.disable_eip3607 = true;
    cfg.disable_base_fee = true;
    cfg.disable_fee_charge = true;
}

// Auxiliary metadata lookups observe the post-execution local state as
// read-like calls, so they must not apply send-transaction validity or fees.
fn configure_metadata_call_context<INSP>(evm: &mut MainnetAlloyEvm<INSP>) {
    configure_relaxed_call_cfg(&mut evm.ctx_mut().cfg);
}

fn load_erc20_metadata<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    execution_chain_id: u64,
    token_address: Address,
) -> Erc20Metadata {
    // ERC20 metadata is best-effort and optional. Failures should not affect
    // change extraction itself.
    Erc20Metadata {
        name: call_erc20_name(evm, transaction, execution_chain_id, token_address),
        symbol: call_erc20_symbol(evm, transaction, execution_chain_id, token_address),
        decimals: call_erc20_decimals(evm, transaction, execution_chain_id, token_address),
    }
}

fn load_erc721_collection_metadata<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    execution_chain_id: u64,
    contract_address: Address,
) -> Erc721CollectionMetadata {
    // Only load collection metadata from the standard ERC721 metadata
    // extension, and leave optional fields empty when that support is absent.
    let supports_metadata = try_supports_interface(
        evm,
        transaction,
        execution_chain_id,
        contract_address,
        [0x5b, 0x5e, 0x13, 0x9f],
    );

    if supports_metadata != Some(true) {
        return Erc721CollectionMetadata::default();
    }

    Erc721CollectionMetadata {
        name: call_erc721_name(evm, transaction, execution_chain_id, contract_address),
        symbol: call_erc721_symbol(evm, transaction, execution_chain_id, contract_address),
    }
}

fn resolve_contract_kind<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    execution_chain_id: u64,
    contract_address: Address,
) -> ContractKind {
    // ERC20 does not expose a standard ERC165 interface id, so this is a
    // best-effort NFT-vs-fungible classification rather than a strict proof.
    let supports_erc721 = try_supports_interface(
        evm,
        transaction,
        execution_chain_id,
        contract_address,
        [0x80, 0xac, 0x58, 0xcd],
    );
    if supports_erc721 == Some(true) {
        return ContractKind::Erc721;
    }

    let supports_erc1155 = try_supports_interface(
        evm,
        transaction,
        execution_chain_id,
        contract_address,
        [0xd9, 0xb6, 0x7a, 0x26],
    );
    if supports_erc1155 == Some(true) {
        return ContractKind::Erc1155;
    }

    if supports_erc721.is_none() && supports_erc1155.is_none() {
        ContractKind::Unknown
    } else {
        ContractKind::FungibleLike
    }
}

fn try_supports_interface<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    execution_chain_id: u64,
    contract_address: Address,
    interface_id: [u8; 4],
) -> Option<bool> {
    let output = transact_metadata_call(
        evm,
        create_metadata_call_tx_env(
            transaction,
            execution_chain_id,
            contract_address,
            IERC165::supportsInterfaceCall {
                interfaceId: FixedBytes::<4>::from(interface_id),
            }
            .abi_encode()
            .into(),
        ),
    );

    output
        .and_then(|output| IERC165::supportsInterfaceCall::abi_decode_returns(output.as_ref()).ok())
}

fn call_erc20_symbol<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    execution_chain_id: u64,
    token_address: Address,
) -> Option<String> {
    let output = transact_metadata_call(
        evm,
        create_metadata_call_tx_env(
            transaction,
            execution_chain_id,
            token_address,
            IERC20Metadata::symbolCall {}.abi_encode().into(),
        ),
    )?;

    IERC20Metadata::symbolCall::abi_decode_returns(output.as_ref()).ok()
}

fn call_erc20_name<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    execution_chain_id: u64,
    token_address: Address,
) -> Option<String> {
    let output = transact_metadata_call(
        evm,
        create_metadata_call_tx_env(
            transaction,
            execution_chain_id,
            token_address,
            IERC20Metadata::nameCall {}.abi_encode().into(),
        ),
    )?;

    IERC20Metadata::nameCall::abi_decode_returns(output.as_ref()).ok()
}

fn call_erc20_decimals<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    execution_chain_id: u64,
    token_address: Address,
) -> Option<u8> {
    let output = transact_metadata_call(
        evm,
        create_metadata_call_tx_env(
            transaction,
            execution_chain_id,
            token_address,
            IERC20Metadata::decimalsCall {}.abi_encode().into(),
        ),
    )?;

    IERC20Metadata::decimalsCall::abi_decode_returns(output.as_ref()).ok()
}

fn call_erc721_name<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    execution_chain_id: u64,
    contract_address: Address,
) -> Option<String> {
    let output = transact_metadata_call(
        evm,
        create_metadata_call_tx_env(
            transaction,
            execution_chain_id,
            contract_address,
            IERC721Metadata::nameCall {}.abi_encode().into(),
        ),
    )?;

    IERC721Metadata::nameCall::abi_decode_returns(output.as_ref()).ok()
}

fn call_erc721_symbol<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    execution_chain_id: u64,
    contract_address: Address,
) -> Option<String> {
    let output = transact_metadata_call(
        evm,
        create_metadata_call_tx_env(
            transaction,
            execution_chain_id,
            contract_address,
            IERC721Metadata::symbolCall {}.abi_encode().into(),
        ),
    )?;

    IERC721Metadata::symbolCall::abi_decode_returns(output.as_ref()).ok()
}

fn create_metadata_call_tx_env(
    transaction: &EvmTransaction,
    execution_chain_id: u64,
    token_address: Address,
    data: Bytes,
) -> TxEnv {
    // Use a distinct preview nonce so the auxiliary call does not collide with
    // the user transaction when both are executed against the same local state.
    TxEnv {
        tx_type: TransactionType::Legacy as u8,
        caller: transaction.from,
        gas_limit: METADATA_CALL_GAS_LIMIT,
        gas_price: 0,
        kind: TxKind::Call(token_address),
        value: U256::ZERO,
        data,
        nonce: transaction.nonce.saturating_add(1),
        chain_id: Some(execution_chain_id),
        access_list: RevmAccessList::default(),
        gas_priority_fee: None,
        blob_hashes: Vec::new(),
        max_fee_per_blob_gas: 0,
        authorization_list: Vec::new(),
    }
}

// Runs a read-like auxiliary call against the current in-memory EVM state and
// returns only successfully decoded output bytes.
fn transact_metadata_call<INSP>(evm: &mut MainnetAlloyEvm<INSP>, tx_env: TxEnv) -> Option<Bytes> {
    let result = evm.transact(tx_env).ok()?.result;

    match result {
        ExecutionResult::Success { output, .. } => Some(output.into_data()),
        ExecutionResult::Revert { .. } | ExecutionResult::Halt { .. } => None,
    }
}
