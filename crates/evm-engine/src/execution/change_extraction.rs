use std::collections::HashMap;

use alloy::{sol, sol_types::SolCall};
use alloy_primitives::{Address, Bytes, FixedBytes, U256};
use revm::{
    Database, ExecuteEvm,
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
    execution::{ExecutionArtifacts, MainnetAlloyEvm, MainnetEvmWithDb},
    transaction_changes::{
        ChangeCandidate, ContractKind, CurrentChangeFacts, CurrentFactRequests, Erc20Metadata,
        Erc721CollectionMetadata, build_current_changes, collect_candidates,
        derive_current_fact_requests,
    },
};

const AUXILIARY_CALL_GAS_LIMIT: u64 = 100_000;

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

    let requests = derive_current_fact_requests(&candidates);
    let facts = with_auxiliary_call_context(evm, |evm| {
        load_current_change_facts(evm, transaction, artifacts.chain_id, requests)
    });

    build_current_changes(candidates, &facts)
}

fn with_auxiliary_call_context<DB, INSP, T>(
    evm: &mut MainnetEvmWithDb<DB, INSP>,
    read: impl FnOnce(&mut MainnetEvmWithDb<DB, INSP>) -> T,
) -> T
where
    DB: Database,
{
    let original_cfg = evm.ctx().cfg.clone();
    let original_tx = evm.ctx().tx.clone();

    configure_auxiliary_call_context(evm);
    let output = read(evm);

    evm.ctx_mut().cfg = original_cfg;
    evm.ctx_mut().tx = original_tx;

    output
}

fn load_current_change_facts<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    execution_chain_id: u64,
    requests: CurrentFactRequests,
) -> CurrentChangeFacts {
    let mut contract_kinds = HashMap::new();
    let mut erc20_metadata = HashMap::new();
    let mut erc721_collection_metadata = HashMap::new();

    for contract in requests.contract_kinds {
        contract_kinds.insert(
            contract,
            resolve_contract_kind(evm, transaction, execution_chain_id, contract),
        );
    }

    for token in requests.erc20_metadata {
        erc20_metadata.insert(
            token,
            load_erc20_metadata(evm, transaction, execution_chain_id, token),
        );
    }

    for request in requests.erc721_collection_metadata {
        if request.only_if_classified_as_erc721
            && contract_kinds.get(&request.collection) != Some(&ContractKind::Erc721)
        {
            continue;
        }

        erc721_collection_metadata.insert(
            request.collection,
            load_erc721_collection_metadata(
                evm,
                transaction,
                execution_chain_id,
                request.collection,
            ),
        );
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
fn configure_auxiliary_call_context<DB, INSP>(evm: &mut MainnetEvmWithDb<DB, INSP>)
where
    DB: Database,
{
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
    let output = transact_auxiliary_call(
        evm,
        create_auxiliary_call_tx_env(
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
    let output = transact_auxiliary_call(
        evm,
        create_auxiliary_call_tx_env(
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
    let output = transact_auxiliary_call(
        evm,
        create_auxiliary_call_tx_env(
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
    let output = transact_auxiliary_call(
        evm,
        create_auxiliary_call_tx_env(
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
    let output = transact_auxiliary_call(
        evm,
        create_auxiliary_call_tx_env(
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
    let output = transact_auxiliary_call(
        evm,
        create_auxiliary_call_tx_env(
            transaction,
            execution_chain_id,
            contract_address,
            IERC721Metadata::symbolCall {}.abi_encode().into(),
        ),
    )?;

    IERC721Metadata::symbolCall::abi_decode_returns(output.as_ref()).ok()
}

fn create_auxiliary_call_tx_env(
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
        gas_limit: AUXILIARY_CALL_GAS_LIMIT,
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
fn transact_auxiliary_call<DB, INSP>(
    evm: &mut MainnetEvmWithDb<DB, INSP>,
    tx_env: TxEnv,
) -> Option<Bytes>
where
    DB: Database,
{
    let result = evm.transact(tx_env).ok()?.result;

    match result {
        ExecutionResult::Success { output, .. } => Some(output.into_data()),
        ExecutionResult::Revert { .. } | ExecutionResult::Halt { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, Bytes, U256};
    use revm::{
        Context, Database, MainBuilder, MainContext,
        context::TxEnv,
        database::InMemoryDB,
        handler::EvmTr,
        primitives::TxKind,
        state::{AccountInfo, Bytecode, bytecode::opcode},
    };

    use super::{AUXILIARY_CALL_GAS_LIMIT, transact_auxiliary_call, with_auxiliary_call_context};

    #[test]
    fn scopes_auxiliary_call_context_without_committing_state() {
        let caller = Address::repeat_byte(0x01);
        let contract = Address::repeat_byte(0x02);

        let code = vec![
            opcode::PUSH1,
            0x2a,
            opcode::PUSH1,
            0x00,
            opcode::SSTORE,
            opcode::PUSH1,
            0x00,
            opcode::PUSH1,
            0x00,
            opcode::RETURN,
        ];

        let mut db = InMemoryDB::default();
        db.insert_account_info(
            caller,
            AccountInfo::default().with_balance(U256::from(1_000_000_000_u64)),
        );
        db.insert_account_info(
            contract,
            AccountInfo::default()
                .with_nonce(1)
                .with_code(Bytecode::new_raw(Bytes::from(code))),
        );

        let mut evm = Context::mainnet().with_db(db).build_mainnet();

        let original_tx = TxEnv {
            caller: Address::repeat_byte(0x03),
            kind: TxKind::Call(Address::repeat_byte(0x04)),
            nonce: 7,
            gas_limit: 21_000,
            ..TxEnv::default()
        };
        evm.ctx_mut().tx = original_tx.clone();
        let original_cfg = evm.ctx().cfg.clone();

        let auxiliary_tx = TxEnv {
            caller,
            kind: TxKind::Call(contract),
            gas_limit: AUXILIARY_CALL_GAS_LIMIT,
            ..TxEnv::default()
        };

        let output = with_auxiliary_call_context(&mut evm, |evm| {
            assert!(evm.ctx().cfg.disable_nonce_check);
            assert!(evm.ctx().cfg.disable_balance_check);
            assert!(evm.ctx().cfg.disable_eip3607);
            assert!(evm.ctx().cfg.disable_base_fee);
            assert!(evm.ctx().cfg.disable_fee_charge);

            transact_auxiliary_call(evm, auxiliary_tx)
        });

        assert_eq!(output, Some(Bytes::new()));
        assert_eq!(evm.ctx().cfg, original_cfg);
        assert_eq!(evm.ctx().tx, original_tx);

        let stored_value = evm
            .ctx_mut()
            .journaled_state
            .database
            .storage(contract, U256::ZERO)
            .expect("contract storage");

        assert_eq!(stored_value, U256::ZERO);
    }
}
