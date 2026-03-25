use crate::{
    AccessListItem, BlockRef, EvmEngineError, EvmExecutionFailure, EvmExecutionInput,
    EvmExecutionLog, EvmExecutionOutput, EvmExecutionStatus, EvmTransaction, EvmTransactionType,
    SimulatedBlock,
};
use alloy::{
    consensus::BlockHeader,
    eips::{BlockId, BlockNumberOrTag},
    network::Ethereum,
    providers::{DynProvider, Provider, ProviderBuilder},
    rpc::types::Block,
};
use alloy_chains::Chain;
use alloy_hardforks::EthereumHardfork;
use alloy_primitives::{Bytes, Log, U256};
use revm::{
    Context, ExecuteEvm, MainBuilder, MainContext,
    context::{BlockEnv, CfgEnv, TxEnv},
    context_interface::{
        block::BlobExcessGasAndPrice,
        result::{EVMError, ExecutionResult, HaltReason, InvalidTransaction},
        transaction::{
            AccessList as RevmAccessList, AccessListItem as RevmAccessListItem, TransactionType,
        },
    },
    database::{AlloyDB, CacheDB, WrapDatabaseAsync},
    primitives::{TxKind, hardfork::SpecId},
};

type AlloyCacheDb = CacheDB<WrapDatabaseAsync<AlloyDB<Ethereum, DynProvider>>>;

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
    let db = create_database(&provider, resolved_block.state_block_id)?;
    let spec_id = resolve_spec_id(&resolved_block)?;
    let cfg_env = create_cfg_env(resolved_block.chain_id, spec_id);
    let block_env = create_block_env(&resolved_block, spec_id)?;
    let tx_env = create_tx_env(&transaction)?;

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
        .map_err(|error| EvmEngineError::internal(format!("invalid rpc url: {error}")))?;

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
        BlockRef::Hash(hash) => {
            let block = load_block(
                provider,
                BlockId::from(*hash),
                format!("block hash {hash} was not returned by provider"),
            )
            .await?;
            let block_number = block.number();

            (block, block_number_id(block_number))
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
        .ok_or_else(|| EvmEngineError::internal(missing_message))
}

fn block_number_id(number: u64) -> BlockId {
    BlockId::Number(BlockNumberOrTag::Number(number))
}

fn create_database(
    provider: &DynProvider,
    block_id: BlockId,
) -> Result<AlloyCacheDb, EvmEngineError> {
    let alloy_db =
        WrapDatabaseAsync::new(AlloyDB::new(provider.clone(), block_id)).ok_or_else(|| {
            EvmEngineError::internal(
                "failed to create async database wrapper from current tokio runtime",
            )
        })?;

    Ok(CacheDB::new(alloy_db))
}

fn create_cfg_env(chain_id: u64, spec_id: SpecId) -> CfgEnv {
    CfgEnv::new_with_spec(spec_id).with_chain_id(chain_id)
}

fn create_block_env(
    resolved_block: &ResolvedExecutionBlock,
    spec_id: SpecId,
) -> Result<BlockEnv, EvmEngineError> {
    let header = &resolved_block.block.header;

    let basefee = if spec_id.is_enabled_in(SpecId::LONDON) {
        header.base_fee_per_gas().ok_or_else(|| {
            EvmEngineError::internal(format!(
                "rpc block header is missing base fee for spec {spec_id:?}"
            ))
        })?
    } else {
        0
    };

    let prevrandao = if spec_id.is_enabled_in(SpecId::MERGE) {
        Some(header.mix_hash().ok_or_else(|| {
            EvmEngineError::internal(format!(
                "rpc block header is missing prev randao for spec {spec_id:?}"
            ))
        })?)
    } else {
        None
    };

    let blob_excess_gas_and_price = if spec_id.is_enabled_in(SpecId::CANCUN) {
        let excess_blob_gas = header.excess_blob_gas().ok_or_else(|| {
            EvmEngineError::internal(format!(
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

fn create_tx_env(transaction: &EvmTransaction) -> Result<TxEnv, EvmEngineError> {
    match transaction.tx_type {
        EvmTransactionType::Legacy => create_legacy_tx_env(transaction),
        EvmTransactionType::AccessList => create_access_list_tx_env(transaction),
        EvmTransactionType::DynamicFee => create_dynamic_fee_tx_env(transaction),
    }
}

fn create_legacy_tx_env(transaction: &EvmTransaction) -> Result<TxEnv, EvmEngineError> {
    if !transaction.access_list.is_empty() {
        return Err(EvmEngineError::internal(
            "legacy transaction must not include access_list",
        ));
    }

    let gas_price = transaction
        .gas_price
        .ok_or_else(|| EvmEngineError::internal("legacy transaction requires gas_price"))?;

    Ok(base_tx_env(
        transaction,
        TransactionType::Legacy,
        gas_price,
        RevmAccessList::default(),
        None,
    ))
}

fn create_access_list_tx_env(transaction: &EvmTransaction) -> Result<TxEnv, EvmEngineError> {
    let gas_price = transaction
        .gas_price
        .ok_or_else(|| EvmEngineError::internal("access-list transaction requires gas_price"))?;

    Ok(base_tx_env(
        transaction,
        TransactionType::Eip2930,
        gas_price,
        map_access_list(&transaction.access_list),
        None,
    ))
}

fn create_dynamic_fee_tx_env(transaction: &EvmTransaction) -> Result<TxEnv, EvmEngineError> {
    let max_fee_per_gas = transaction.max_fee_per_gas.ok_or_else(|| {
        EvmEngineError::internal("dynamic-fee transaction requires max_fee_per_gas")
    })?;

    let max_priority_fee_per_gas = transaction.max_priority_fee_per_gas.ok_or_else(|| {
        EvmEngineError::internal("dynamic-fee transaction requires max_priority_fee_per_gas")
    })?;

    Ok(base_tx_env(
        transaction,
        TransactionType::Eip1559,
        max_fee_per_gas,
        map_access_list(&transaction.access_list),
        Some(max_priority_fee_per_gas),
    ))
}

fn base_tx_env(
    transaction: &EvmTransaction,
    tx_type: TransactionType,
    gas_price: u128,
    access_list: RevmAccessList,
    gas_priority_fee: Option<u128>,
) -> TxEnv {
    TxEnv {
        tx_type: tx_type as u8,
        caller: transaction.from,
        gas_limit: transaction.gas_limit,
        gas_price,
        kind: TxKind::from(transaction.to),
        value: transaction.value,
        data: transaction.data.clone(),
        nonce: transaction.nonce,
        chain_id: Some(transaction.chain_id),
        access_list,
        gas_priority_fee,
        blob_hashes: Vec::new(),
        max_fee_per_blob_gas: 0,
        authorization_list: Vec::new(),
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
        .build_mainnet();

    match evm.transact(tx_env) {
        Ok(result_and_state) => Ok(map_execution_result(
            result_and_state.result,
            resolved_block,
            transaction,
        )),
        Err(EVMError::Transaction(error)) => Ok(build_failed_output(
            resolved_block,
            transaction,
            0,
            transaction.gas_limit,
            Bytes::new(),
            map_invalid_transaction_failure(error),
        )),
        Err(EVMError::Header(error)) => Err(EvmEngineError::internal(format!(
            "revm header validation failed: {error}"
        ))),
        Err(EVMError::Database(error)) => Err(EvmEngineError::internal(format!(
            "revm database access failed: {error}"
        ))),
        Err(EVMError::Custom(error)) => Err(EvmEngineError::internal(format!(
            "revm execution failed: {error}"
        ))),
    }
}

fn map_execution_result(
    result: ExecutionResult<HaltReason>,
    resolved_block: &ResolvedExecutionBlock,
    transaction: &EvmTransaction,
) -> EvmExecutionOutput {
    match result {
        ExecutionResult::Success {
            gas, logs, output, ..
        } => EvmExecutionOutput {
            chain_id: transaction.chain_id,
            block: simulated_block(resolved_block),
            status: EvmExecutionStatus::Success,
            gas_used: gas.used(),
            gas_limit: gas.limit(),
            output: output.into_data(),
            failure: None,
            logs: map_execution_logs(logs),
        },
        ExecutionResult::Revert { gas, output, .. } => build_failed_output(
            resolved_block,
            transaction,
            gas.used(),
            gas.limit(),
            output,
            EvmExecutionFailure {
                code: "REVERT".to_string(),
                message: "execution reverted".to_string(),
                reason: None,
            },
        ),
        ExecutionResult::Halt { reason, gas, .. } => build_failed_output(
            resolved_block,
            transaction,
            gas.used(),
            gas.limit(),
            Bytes::new(),
            map_halt_failure(reason),
        ),
    }
}

fn build_failed_output(
    resolved_block: &ResolvedExecutionBlock,
    transaction: &EvmTransaction,
    gas_used: u64,
    gas_limit: u64,
    output: Bytes,
    failure: EvmExecutionFailure,
) -> EvmExecutionOutput {
    EvmExecutionOutput {
        chain_id: transaction.chain_id,
        block: simulated_block(resolved_block),
        status: EvmExecutionStatus::Failed,
        gas_used,
        gas_limit,
        output,
        failure: Some(failure),
        logs: Vec::new(),
    }
}

fn simulated_block(resolved_block: &ResolvedExecutionBlock) -> SimulatedBlock {
    SimulatedBlock {
        number: resolved_block.block.number(),
        hash: resolved_block.block.hash(),
    }
}

fn map_execution_logs(logs: Vec<Log>) -> Vec<EvmExecutionLog> {
    logs.into_iter()
        .enumerate()
        .map(|(index, log)| EvmExecutionLog {
            log_index: index as u64,
            address: log.address,
            topics: log.data.topics().to_vec(),
            data: log.data.data,
        })
        .collect()
}

fn map_invalid_transaction_failure(error: InvalidTransaction) -> EvmExecutionFailure {
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

fn map_halt_failure(reason: HaltReason) -> EvmExecutionFailure {
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
    EvmEngineError::internal(format!("rpc request failed: {error}"))
}

fn resolve_spec_id(resolved_block: &ResolvedExecutionBlock) -> Result<SpecId, EvmEngineError> {
    let chain = Chain::from_id(resolved_block.chain_id);
    let timestamp = resolved_block.block.header.timestamp;

    let hardfork =
        EthereumHardfork::from_chain_and_timestamp(chain, timestamp).ok_or_else(|| {
            EvmEngineError::not_ready(format!(
                "hardfork resolution is not implemented for chain_id={}",
                resolved_block.chain_id
            ))
        })?;

    map_hardfork_to_spec_id(hardfork)
}

fn map_hardfork_to_spec_id(hardfork: EthereumHardfork) -> Result<SpecId, EvmEngineError> {
    let spec_id = match hardfork {
        EthereumHardfork::Frontier => SpecId::FRONTIER,
        EthereumHardfork::Homestead => SpecId::HOMESTEAD,
        EthereumHardfork::Dao => SpecId::DAO_FORK,
        EthereumHardfork::Tangerine => SpecId::TANGERINE,
        EthereumHardfork::SpuriousDragon => SpecId::SPURIOUS_DRAGON,
        EthereumHardfork::Byzantium => SpecId::BYZANTIUM,
        EthereumHardfork::Constantinople => SpecId::CONSTANTINOPLE,
        EthereumHardfork::Petersburg => SpecId::PETERSBURG,
        EthereumHardfork::Istanbul => SpecId::ISTANBUL,
        EthereumHardfork::MuirGlacier => SpecId::MUIR_GLACIER,
        EthereumHardfork::Berlin => SpecId::BERLIN,
        EthereumHardfork::London => SpecId::LONDON,
        EthereumHardfork::ArrowGlacier => SpecId::ARROW_GLACIER,
        EthereumHardfork::GrayGlacier => SpecId::GRAY_GLACIER,
        EthereumHardfork::Paris => SpecId::MERGE,
        EthereumHardfork::Shanghai => SpecId::SHANGHAI,
        EthereumHardfork::Cancun => SpecId::CANCUN,
        EthereumHardfork::Prague => SpecId::PRAGUE,
        EthereumHardfork::Osaka
        | EthereumHardfork::Bpo1
        | EthereumHardfork::Bpo2
        | EthereumHardfork::Bpo3
        | EthereumHardfork::Bpo4
        | EthereumHardfork::Bpo5 => SpecId::OSAKA,
        EthereumHardfork::Amsterdam => SpecId::AMSTERDAM,
        _ => {
            return Err(EvmEngineError::not_ready(format!(
                "hardfork {hardfork:?} is not mapped to revm::SpecId yet"
            )));
        }
    };

    Ok(spec_id)
}
