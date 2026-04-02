use crate::{
    AccessListItem, BlockRef, EvmEngineError, EvmExecutionFailure, EvmExecutionInput,
    EvmExecutionOutput, EvmExecutionStatus, EvmTransaction, EvmTransactionType, SimulatedBlock,
    artifacts::{ExecutionArtifacts, RawExecutionLog},
    asset_changes::{Erc20Metadata, extract_asset_changes, fill_erc20_metadata},
    chain_spec::resolve_execution_spec_id,
    frames::ExecutionFrame,
    trace::TraceInspector,
};
use alloy::{
    consensus::BlockHeader,
    eips::{BlockId, BlockNumberOrTag},
    network::Ethereum,
    providers::{DynProvider, Provider, ProviderBuilder},
    rpc::types::Block,
    sol,
    sol_types::SolCall,
};
use alloy_primitives::{Address, Bytes, Log, U256};
use revm::{
    Context, ExecuteEvm, InspectCommitEvm, MainBuilder, MainContext, MainnetEvm,
    context::{BlockEnv, CfgEnv, TxEnv},
    context_interface::{
        block::BlobExcessGasAndPrice,
        result::{EVMError, ExecutionResult, HaltReason, InvalidTransaction},
        transaction::{
            AccessList as RevmAccessList, AccessListItem as RevmAccessListItem, TransactionType,
        },
    },
    database::{AlloyDB, CacheDB, WrapDatabaseAsync},
    handler::EvmTr,
    primitives::{TxKind, hardfork::SpecId},
};

type AlloyCacheDb = CacheDB<WrapDatabaseAsync<AlloyDB<Ethereum, DynProvider>>>;
type MainnetAlloyEvm<INSP = ()> = MainnetEvm<Context<BlockEnv, TxEnv, CfgEnv, AlloyCacheDb>, INSP>;

const ERC20_METADATA_GAS_LIMIT: u64 = 100_000;

sol! {
    contract IERC20Metadata {
        function symbol() external view returns (string);
        function decimals() external view returns (uint8);
    }
}

#[derive(Debug, Clone)]
struct ResolvedExecutionBlock {
    chain_id: u64,
    state_block_id: BlockId,
    block: Block,
}

pub(crate) async fn simulate_execution(
    rpc_url: &str,
    input: EvmExecutionInput,
) -> Result<EvmExecutionOutput, EvmEngineError> {
    let EvmExecutionInput { block, transaction } = input;
    let provider = build_provider(rpc_url)?;
    let resolved_block = resolve_execution_block(&provider, &block).await?;
    validate_requested_chain_id(transaction.requested_chain_id, resolved_block.chain_id)?;
    let db = create_database(&provider, &resolved_block)?;
    let spec_id = resolve_execution_spec_id(
        resolved_block.chain_id,
        resolved_block.block.number(),
        resolved_block.block.header.timestamp,
    )?;
    let cfg_env = create_cfg_env(&transaction, resolved_block.chain_id, spec_id);
    let block_env = create_block_env(&resolved_block, spec_id)?;
    let tx_env = create_tx_env(&transaction, resolved_block.chain_id)?;

    execute_transaction(
        db,
        cfg_env,
        block_env,
        tx_env,
        &resolved_block,
        &transaction,
    )
}

fn build_provider(rpc_url: &str) -> Result<DynProvider, EvmEngineError> {
    let rpc_url = rpc_url
        .parse()
        .map_err(|error| EvmEngineError::config_error(format!("invalid rpc url: {error}")))?;

    Ok(ProviderBuilder::new().connect_http(rpc_url).erased())
}

async fn resolve_execution_block(
    provider: &DynProvider,
    block_ref: &BlockRef,
) -> Result<ResolvedExecutionBlock, EvmEngineError> {
    let chain_id = provider.get_chain_id().await.map_err(map_rpc_error)?;
    let (block, state_block_id) = match block_ref {
        BlockRef::Latest => {
            let block = load_block(
                provider,
                BlockId::Number(BlockNumberOrTag::Latest),
                "latest block was not returned by provider",
            )
            .await?;
            let block_number = block.number();

            (block, block_number_id(block_number))
        }
        BlockRef::Number(number) => {
            let block_id = block_number_id(*number);
            let block = load_block(
                provider,
                block_id,
                format!("block number {number} was not returned by provider"),
            )
            .await?;

            (block, block_id)
        }
        BlockRef::Hash(_) => {
            return Err(EvmEngineError::not_supported(
                "block.hash is not supported yet",
            ));
        }
    };

    Ok(ResolvedExecutionBlock {
        chain_id,
        state_block_id,
        block,
    })
}

async fn load_block(
    provider: &DynProvider,
    block_id: BlockId,
    missing_message: impl Into<String>,
) -> Result<Block, EvmEngineError> {
    provider
        .get_block(block_id)
        .await
        .map_err(map_rpc_error)?
        .ok_or_else(|| EvmEngineError::block_not_found(missing_message))
}

fn block_number_id(number: u64) -> BlockId {
    BlockId::Number(BlockNumberOrTag::Number(number))
}

fn create_database(
    provider: &DynProvider,
    resolved_block: &ResolvedExecutionBlock,
) -> Result<AlloyCacheDb, EvmEngineError> {
    let alloy_db = WrapDatabaseAsync::new(AlloyDB::new(
        provider.clone(),
        resolved_block.state_block_id,
    ))
    .ok_or_else(|| {
        EvmEngineError::runtime_error(
            "failed to create async database wrapper from current tokio runtime",
        )
    })?;

    Ok(CacheDB::new(alloy_db))
}

fn create_cfg_env(transaction: &EvmTransaction, chain_id: u64, spec_id: SpecId) -> CfgEnv {
    let mut cfg = CfgEnv::new_with_spec(spec_id).with_chain_id(chain_id);
    configure_preview_cfg(&mut cfg, transaction);
    cfg
}

fn configure_preview_cfg(cfg: &mut CfgEnv, transaction: &EvmTransaction) {
    cfg.disable_nonce_check = transaction.nonce.is_none();
    cfg.disable_balance_check = true;
    cfg.disable_eip3607 = true;
    cfg.disable_base_fee = !transaction_has_explicit_fee_constraints(transaction);
    cfg.disable_fee_charge = true;
}

fn transaction_has_explicit_fee_constraints(transaction: &EvmTransaction) -> bool {
    transaction.gas_price.is_some()
        || transaction.max_fee_per_gas.is_some()
        || transaction.max_priority_fee_per_gas.is_some()
}

fn create_block_env(
    resolved_block: &ResolvedExecutionBlock,
    spec_id: SpecId,
) -> Result<BlockEnv, EvmEngineError> {
    let header = &resolved_block.block.header;

    let basefee = if spec_id.is_enabled_in(SpecId::LONDON) {
        header.base_fee_per_gas().ok_or_else(|| {
            EvmEngineError::block_context_error(format!(
                "rpc block header is missing base fee for spec {spec_id:?}"
            ))
        })?
    } else {
        0
    };

    let prevrandao = if spec_id.is_enabled_in(SpecId::MERGE) {
        Some(header.mix_hash().ok_or_else(|| {
            EvmEngineError::block_context_error(format!(
                "rpc block header is missing prev randao for spec {spec_id:?}"
            ))
        })?)
    } else {
        None
    };

    let blob_excess_gas_and_price = if spec_id.is_enabled_in(SpecId::CANCUN) {
        let excess_blob_gas = header.excess_blob_gas().ok_or_else(|| {
            EvmEngineError::block_context_error(format!(
                "rpc block header is missing excess blob gas for spec {spec_id:?}"
            ))
        })?;

        Some(BlobExcessGasAndPrice::new_with_spec(
            excess_blob_gas,
            spec_id,
        ))
    } else {
        None
    };

    Ok(BlockEnv {
        number: U256::from(resolved_block.block.number()),
        beneficiary: header.beneficiary(),
        timestamp: U256::from(header.timestamp()),
        gas_limit: header.gas_limit(),
        basefee,
        difficulty: header.difficulty(),
        prevrandao,
        blob_excess_gas_and_price,
        slot_num: 0,
    })
}

fn create_tx_env(
    transaction: &EvmTransaction,
    execution_chain_id: u64,
) -> Result<TxEnv, EvmEngineError> {
    match transaction.tx_type {
        EvmTransactionType::Legacy => create_legacy_tx_env(transaction, execution_chain_id),
        EvmTransactionType::AccessList => {
            create_access_list_tx_env(transaction, execution_chain_id)
        }
        EvmTransactionType::DynamicFee => {
            create_dynamic_fee_tx_env(transaction, execution_chain_id)
        }
    }
}

fn create_legacy_tx_env(
    transaction: &EvmTransaction,
    execution_chain_id: u64,
) -> Result<TxEnv, EvmEngineError> {
    if !transaction.access_list.is_empty() {
        return Err(EvmEngineError::internal(
            "legacy transaction must not include access_list",
        ));
    }

    Ok(base_tx_env(
        transaction,
        TransactionType::Legacy,
        MaterializedTxEnvValues {
            effective_gas_price: transaction.gas_price.unwrap_or(0),
            gas_priority_fee: None,
            nonce: materialize_preview_nonce(transaction),
            chain_id: materialize_tx_chain_id(transaction, execution_chain_id),
            access_list: RevmAccessList::default(),
        },
    ))
}
fn create_access_list_tx_env(
    transaction: &EvmTransaction,
    execution_chain_id: u64,
) -> Result<TxEnv, EvmEngineError> {
    Ok(base_tx_env(
        transaction,
        TransactionType::Eip2930,
        MaterializedTxEnvValues {
            effective_gas_price: transaction.gas_price.unwrap_or(0),
            gas_priority_fee: None,
            nonce: materialize_preview_nonce(transaction),
            chain_id: materialize_tx_chain_id(transaction, execution_chain_id),
            access_list: map_access_list(&transaction.access_list),
        },
    ))
}

fn create_dynamic_fee_tx_env(
    transaction: &EvmTransaction,
    execution_chain_id: u64,
) -> Result<TxEnv, EvmEngineError> {
    let (materialized_max_fee_per_gas, materialized_max_priority_fee_per_gas) =
        materialize_preview_dynamic_fee(transaction);

    Ok(base_tx_env(
        transaction,
        TransactionType::Eip1559,
        MaterializedTxEnvValues {
            effective_gas_price: materialized_max_fee_per_gas,
            gas_priority_fee: Some(materialized_max_priority_fee_per_gas),
            nonce: materialize_preview_nonce(transaction),
            chain_id: materialize_tx_chain_id(transaction, execution_chain_id),
            access_list: map_access_list(&transaction.access_list),
        },
    ))
}

struct MaterializedTxEnvValues {
    effective_gas_price: u128,
    gas_priority_fee: Option<u128>,
    nonce: u64,
    chain_id: u64,
    access_list: RevmAccessList,
}

fn materialize_preview_nonce(transaction: &EvmTransaction) -> u64 {
    transaction.nonce.unwrap_or(0)
}
fn materialize_tx_chain_id(transaction: &EvmTransaction, execution_chain_id: u64) -> u64 {
    transaction.requested_chain_id.unwrap_or(execution_chain_id)
}
fn base_tx_env(
    transaction: &EvmTransaction,
    tx_type: TransactionType,
    values: MaterializedTxEnvValues,
) -> TxEnv {
    TxEnv {
        tx_type: tx_type as u8,
        caller: transaction.from,
        gas_limit: transaction.gas_limit,
        gas_price: values.effective_gas_price,
        kind: TxKind::from(transaction.to),
        value: transaction.value,
        data: transaction.data.clone(),
        nonce: values.nonce,
        chain_id: Some(values.chain_id),
        access_list: values.access_list,
        gas_priority_fee: values.gas_priority_fee,
        blob_hashes: Vec::new(),
        max_fee_per_blob_gas: 0,
        authorization_list: Vec::new(),
    }
}

fn materialize_preview_dynamic_fee(transaction: &EvmTransaction) -> (u128, u128) {
    match (
        transaction.max_fee_per_gas,
        transaction.max_priority_fee_per_gas,
    ) {
        (Some(max_fee_per_gas), Some(max_priority_fee_per_gas)) => {
            (max_fee_per_gas, max_priority_fee_per_gas)
        }
        (Some(max_fee_per_gas), None) => (max_fee_per_gas, 0),
        (None, Some(max_priority_fee_per_gas)) => {
            (max_priority_fee_per_gas, max_priority_fee_per_gas)
        }
        (None, None) => (0, 0),
    }
}

fn map_access_list(items: &[AccessListItem]) -> RevmAccessList {
    items
        .iter()
        .map(|item| RevmAccessListItem {
            address: item.address,
            storage_keys: item.storage_keys.clone(),
        })
        .collect::<Vec<_>>()
        .into()
}

fn execute_transaction(
    db: AlloyCacheDb,
    cfg_env: CfgEnv,
    block_env: BlockEnv,
    tx_env: TxEnv,
    resolved_block: &ResolvedExecutionBlock,
    transaction: &EvmTransaction,
) -> Result<EvmExecutionOutput, EvmEngineError> {
    let mut evm = Context::mainnet()
        .with_db(db)
        .modify_cfg_chained(|cfg| *cfg = cfg_env)
        .modify_block_chained(|block| *block = block_env)
        .build_mainnet_with_inspector(TraceInspector::new());

    let artifacts = match evm.inspect_tx_commit(tx_env) {
        Ok(result) => {
            let frames = std::mem::take(&mut evm.inspector).into_frames();
            build_execution_artifacts(result, frames, resolved_block)
        }
        Err(EVMError::Transaction(error)) => {
            build_invalid_transaction_artifacts(resolved_block, transaction, error)
        }
        Err(EVMError::Header(error)) => {
            return Err(EvmEngineError::block_context_error(format!(
                "engine header validation failed: {error}"
            )));
        }
        Err(EVMError::Database(error)) => {
            return Err(EvmEngineError::state_access_error(format!(
                "state access failed during execution: {error}"
            )));
        }
        Err(EVMError::Custom(error)) => {
            return Err(EvmEngineError::engine_execution_error(format!(
                "engine execution failed: {error}"
            )));
        }
    };

    Ok(build_preview_output(&mut evm, artifacts, transaction))
}

fn build_execution_artifacts(
    result: ExecutionResult<HaltReason>,
    frames: Vec<ExecutionFrame>,
    resolved_block: &ResolvedExecutionBlock,
) -> ExecutionArtifacts {
    match result {
        ExecutionResult::Success {
            gas, logs, output, ..
        } => ExecutionArtifacts {
            chain_id: resolved_block.chain_id,
            block: simulated_block(resolved_block),
            status: EvmExecutionStatus::Success,
            gas_used: gas.used(),
            gas_limit: gas.limit(),
            output: output.into_data(),
            failure: None,
            logs: map_execution_logs(logs),
            frames,
        },
        ExecutionResult::Revert { gas, output, .. } => {
            build_revert_artifacts(resolved_block, gas.used(), gas.limit(), output, frames)
        }
        ExecutionResult::Halt { reason, gas, .. } => {
            build_halt_artifacts(resolved_block, gas.used(), gas.limit(), reason, frames)
        }
    }
}

fn build_preview_output<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    artifacts: ExecutionArtifacts,
    transaction: &EvmTransaction,
) -> EvmExecutionOutput {
    let mut asset_changes = extract_asset_changes(&artifacts);

    if matches!(artifacts.status, EvmExecutionStatus::Success) {
        configure_metadata_call_context(evm);
        fill_erc20_metadata(&mut asset_changes, |token_address| {
            load_erc20_metadata(evm, transaction, artifacts.chain_id, token_address)
        });
    }

    let ExecutionArtifacts {
        chain_id,
        block,
        status,
        gas_used,
        gas_limit,
        output,
        failure,
        ..
    } = artifacts;

    EvmExecutionOutput {
        chain_id,
        block,
        status,
        gas_used,
        gas_limit,
        output,
        failure,
        asset_changes,
    }
}

fn configure_relaxed_call_cfg(cfg: &mut CfgEnv) {
    cfg.disable_nonce_check = true;
    cfg.disable_balance_check = true;
    cfg.disable_eip3607 = true;
    cfg.disable_base_fee = true;
    cfg.disable_fee_charge = true;
}

fn configure_metadata_call_context<INSP>(evm: &mut MainnetAlloyEvm<INSP>) {
    configure_relaxed_call_cfg(&mut evm.ctx_mut().cfg);
}

fn load_erc20_metadata<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    execution_chain_id: u64,
    token_address: Address,
) -> Erc20Metadata {
    Erc20Metadata {
        symbol: call_erc20_symbol(evm, transaction, execution_chain_id, token_address),
        decimals: call_erc20_decimals(evm, transaction, execution_chain_id, token_address),
    }
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
    TxEnv {
        tx_type: TransactionType::Legacy as u8,
        caller: transaction.from,
        gas_limit: ERC20_METADATA_GAS_LIMIT,
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

fn transact_metadata_call<INSP>(evm: &mut MainnetAlloyEvm<INSP>, tx_env: TxEnv) -> Option<Bytes> {
    let result = evm.transact(tx_env).ok()?.result;

    match result {
        ExecutionResult::Success { output, .. } => Some(output.into_data()),
        ExecutionResult::Revert { .. } | ExecutionResult::Halt { .. } => None,
    }
}

fn build_invalid_transaction_artifacts(
    resolved_block: &ResolvedExecutionBlock,
    transaction: &EvmTransaction,
    error: InvalidTransaction,
) -> ExecutionArtifacts {
    build_failed_artifacts(
        resolved_block,
        0,
        transaction.gas_limit,
        Bytes::new(),
        build_invalid_transaction_failure(error),
        Vec::new(),
    )
}

fn build_revert_artifacts(
    resolved_block: &ResolvedExecutionBlock,
    gas_used: u64,
    gas_limit: u64,
    output: Bytes,
    frames: Vec<ExecutionFrame>,
) -> ExecutionArtifacts {
    build_failed_artifacts(
        resolved_block,
        gas_used,
        gas_limit,
        output,
        build_revert_failure(),
        frames,
    )
}

fn build_halt_artifacts(
    resolved_block: &ResolvedExecutionBlock,
    gas_used: u64,
    gas_limit: u64,
    reason: HaltReason,
    frames: Vec<ExecutionFrame>,
) -> ExecutionArtifacts {
    build_failed_artifacts(
        resolved_block,
        gas_used,
        gas_limit,
        Bytes::new(),
        build_halt_failure(reason),
        frames,
    )
}

fn build_failed_artifacts(
    resolved_block: &ResolvedExecutionBlock,
    gas_used: u64,
    gas_limit: u64,
    output: Bytes,
    failure: EvmExecutionFailure,
    frames: Vec<ExecutionFrame>,
) -> ExecutionArtifacts {
    ExecutionArtifacts {
        chain_id: resolved_block.chain_id,
        block: simulated_block(resolved_block),
        status: EvmExecutionStatus::Failed,
        gas_used,
        gas_limit,
        output,
        failure: Some(failure),
        logs: Vec::new(),
        frames,
    }
}

fn simulated_block(resolved_block: &ResolvedExecutionBlock) -> SimulatedBlock {
    SimulatedBlock {
        number: resolved_block.block.number(),
        hash: resolved_block.block.hash(),
    }
}

fn map_execution_logs(logs: Vec<Log>) -> Vec<RawExecutionLog> {
    logs.into_iter()
        .enumerate()
        .map(|(index, log)| RawExecutionLog {
            log_index: index as u64,
            address: log.address,
            topics: log.data.topics().to_vec(),
            data: log.data.data,
        })
        .collect()
}

fn build_invalid_transaction_failure(error: InvalidTransaction) -> EvmExecutionFailure {
    let code = match error {
        InvalidTransaction::NonceTooLow { .. } => "NONCE_TOO_LOW",
        InvalidTransaction::NonceTooHigh { .. } => "NONCE_TOO_HIGH",
        InvalidTransaction::LackOfFundForMaxFee { .. } => "INSUFFICIENT_FUNDS",
        InvalidTransaction::GasPriceLessThanBasefee => "GAS_PRICE_LESS_THAN_BASE_FEE",
        InvalidTransaction::CallerGasLimitMoreThanBlock => "GAS_LIMIT_EXCEEDS_BLOCK_GAS_LIMIT",
        InvalidTransaction::Eip2930NotSupported => "EIP2930_NOT_SUPPORTED",
        InvalidTransaction::Eip1559NotSupported => "EIP1559_NOT_SUPPORTED",
        _ => "INVALID_TRANSACTION",
    };

    EvmExecutionFailure {
        code: code.to_string(),
        message: error.to_string(),
        reason: None,
    }
}

fn build_revert_failure() -> EvmExecutionFailure {
    EvmExecutionFailure {
        code: "REVERT".to_string(),
        message: "execution reverted".to_string(),
        reason: None,
    }
}

fn build_halt_failure(reason: HaltReason) -> EvmExecutionFailure {
    let code = match reason {
        HaltReason::OutOfGas(_) => "OUT_OF_GAS",
        HaltReason::OpcodeNotFound | HaltReason::InvalidFEOpcode => "INVALID_OPCODE",
        HaltReason::InvalidJump => "INVALID_JUMP",
        HaltReason::StackUnderflow => "STACK_UNDERFLOW",
        HaltReason::StackOverflow => "STACK_OVERFLOW",
        HaltReason::OutOfOffset => "OUT_OF_OFFSET",
        HaltReason::CreateCollision => "CREATE_COLLISION",
        HaltReason::NotActivated => "NOT_ACTIVATED",
        HaltReason::PrecompileError | HaltReason::PrecompileErrorWithContext(_) => {
            "PRECOMPILE_ERROR"
        }
        HaltReason::NonceOverflow => "NONCE_OVERFLOW",
        HaltReason::CreateContractSizeLimit => "CREATE_CONTRACT_SIZE_LIMIT",
        HaltReason::CreateContractStartingWithEF => "CREATE_CONTRACT_STARTING_WITH_EF",
        HaltReason::CreateInitCodeSizeLimit => "CREATE_INITCODE_SIZE_LIMIT",
        HaltReason::OverflowPayment => "OVERFLOW_PAYMENT",
        HaltReason::StateChangeDuringStaticCall => "STATE_CHANGE_DURING_STATIC_CALL",
        HaltReason::CallNotAllowedInsideStatic => "CALL_NOT_ALLOWED_INSIDE_STATIC",
        HaltReason::OutOfFunds => "OUT_OF_FUNDS",
        HaltReason::CallTooDeep => "CALL_TOO_DEEP",
    };

    EvmExecutionFailure {
        code: code.to_string(),
        message: reason.to_string(),
        reason: None,
    }
}

fn map_rpc_error(error: impl std::fmt::Display) -> EvmEngineError {
    EvmEngineError::rpc_error(format!("rpc request failed: {error}"))
}

fn validate_requested_chain_id(
    requested_chain_id: Option<u64>,
    actual_chain_id: u64,
) -> Result<(), EvmEngineError> {
    if requested_chain_id.is_none_or(|chain_id| chain_id == actual_chain_id) {
        return Ok(());
    }

    Err(EvmEngineError::not_supported(format!(
        "transaction.chainId does not match the execution chain: requested={}, actual={actual_chain_id}",
        requested_chain_id.expect("checked to be present above")
    )))
}

#[cfg(test)]
mod tests {
    use alloy_chains::Chain;

    use super::*;

    #[test]
    fn create_legacy_tx_env_defaults_call_like_send_fields() {
        let transaction = EvmTransaction {
            tx_type: EvmTransactionType::Legacy,
            requested_chain_id: None,
            from: Address::ZERO,
            to: Some(Address::repeat_byte(0x11)),
            nonce: None,
            gas_limit: 21_000,
            value: U256::ZERO,
            data: Bytes::new(),
            access_list: Vec::new(),
            gas_price: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        };

        let tx_env = create_tx_env(&transaction, Chain::mainnet().id()).expect("tx env");

        assert_eq!(tx_env.gas_price, 0);
        assert_eq!(tx_env.nonce, 0);
        assert_eq!(tx_env.chain_id, Some(Chain::mainnet().id()));
    }

    #[test]
    fn create_dynamic_fee_tx_env_materializes_missing_fee_fields() {
        let mut transaction = EvmTransaction {
            tx_type: EvmTransactionType::DynamicFee,
            requested_chain_id: None,
            from: Address::ZERO,
            to: Some(Address::repeat_byte(0x22)),
            nonce: None,
            gas_limit: 21_000,
            value: U256::ZERO,
            data: Bytes::new(),
            access_list: Vec::new(),
            gas_price: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        };

        let tx_env = create_tx_env(&transaction, Chain::mainnet().id()).expect("tx env");
        assert_eq!(tx_env.gas_price, 0);
        assert_eq!(tx_env.gas_priority_fee, Some(0));

        transaction.max_priority_fee_per_gas = Some(7);

        let tx_env = create_tx_env(&transaction, Chain::mainnet().id()).expect("tx env");
        assert_eq!(tx_env.gas_price, 7);
        assert_eq!(tx_env.gas_priority_fee, Some(7));
    }
}
