use std::sync::LazyLock;

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
    Change, EvmExecutionStatus, EvmTransaction,
    change_detection::{ChangeDetectionPipeline, ContractKind, DetectionSupport, Erc20Metadata},
    execution::ExecutionArtifacts,
    execution::MainnetAlloyEvm,
};

const METADATA_CALL_GAS_LIMIT: u64 = 100_000;

sol! {
    contract IERC20Metadata {
        function symbol() external view returns (string);
        function decimals() external view returns (uint8);
    }

    contract IERC165 {
        function supportsInterface(bytes4 interfaceId) external view returns (bool);
    }
}

static BUILTIN_CHANGE_DETECTION_PIPELINE: LazyLock<ChangeDetectionPipeline> =
    LazyLock::new(ChangeDetectionPipeline::builtin);

pub(super) fn extract_changes<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    artifacts: &ExecutionArtifacts,
    transaction: &EvmTransaction,
) -> Vec<Change> {
    if !matches!(artifacts.status, EvmExecutionStatus::Success) {
        return Vec::new();
    }

    configure_metadata_call_context(evm);
    let mut detection_support = ExecutionDetectionSupport {
        evm,
        transaction,
        execution_chain_id: artifacts.chain_id,
    };

    BUILTIN_CHANGE_DETECTION_PIPELINE.extract_changes(artifacts, &mut detection_support)
}

struct ExecutionDetectionSupport<'a, INSP> {
    evm: &'a mut MainnetAlloyEvm<INSP>,
    transaction: &'a EvmTransaction,
    execution_chain_id: u64,
}

impl<INSP> DetectionSupport for ExecutionDetectionSupport<'_, INSP> {
    fn resolve_contract_kind(&mut self, contract_address: Address) -> ContractKind {
        resolve_contract_kind(
            self.evm,
            self.transaction,
            self.execution_chain_id,
            contract_address,
        )
    }

    fn load_erc20_metadata(&mut self, token_address: Address) -> Erc20Metadata {
        load_erc20_metadata(
            self.evm,
            self.transaction,
            self.execution_chain_id,
            token_address,
        )
    }
}

fn configure_relaxed_call_cfg(cfg: &mut CfgEnv) {
    cfg.disable_nonce_check = true;
    cfg.disable_balance_check = true;
    cfg.disable_eip3607 = true;
    cfg.disable_base_fee = true;
    cfg.disable_fee_charge = true;
}

// Auxiliary metadata lookups should observe the current local state without
// being blocked by preview-only transaction validation rules.
fn configure_metadata_call_context<INSP>(evm: &mut MainnetAlloyEvm<INSP>) {
    configure_relaxed_call_cfg(&mut evm.ctx_mut().cfg);
}

fn load_erc20_metadata<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    execution_chain_id: u64,
    token_address: Address,
) -> Erc20Metadata {
    // This slice keeps ERC20 metadata loading minimal. We only probe stable
    // display fields that already exist in the current implementation path.
    Erc20Metadata {
        name: None,
        symbol: call_erc20_symbol(evm, transaction, execution_chain_id, token_address),
        decimals: call_erc20_decimals(evm, transaction, execution_chain_id, token_address),
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
        nonce: transaction.nonce.unwrap_or(0).saturating_add(1),
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
