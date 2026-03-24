use crate::{EvmEngineError, EvmExecutionInput, EvmExecutionOutput};
use alloy::{
    eips::{BlockId, BlockNumberOrTag},
    network::Ethereum,
    providers::{DynProvider, Provider, ProviderBuilder},
    rpc::types::Block,
};
use alloy_chains::Chain;
use alloy_hardforks::EthereumHardfork;
use revm::{
    context::CfgEnv,
    database::{AlloyDB, CacheDB, WrapDatabaseAsync},
    primitives::hardfork::SpecId,
};

type AlloyCacheDb = CacheDB<WrapDatabaseAsync<AlloyDB<Ethereum, DynProvider>>>;

#[derive(Debug, Clone)]
struct ResolvedExecutionBlock {
    chain_id: u64,
    state_block_id: BlockId,
    block: Block,
}

pub(crate) async fn simulate_latest_dynamic_fee(
    rpc_url: &str,
    _input: EvmExecutionInput,
) -> Result<EvmExecutionOutput, EvmEngineError> {
    let provider = build_provider(rpc_url)?;
    let resolved_block = load_latest_execution_block(&provider).await?;
    let _db = create_database(&provider, resolved_block.state_block_id)?;
    let spec_id = resolve_spec_id(&resolved_block)?;
    let _cfg_env = create_cfg_env(resolved_block.chain_id, spec_id);
    let _chain_id = resolved_block.chain_id;
    let _block_number = resolved_block.block.number();
    let _block_hash = resolved_block.block.hash();

    Err(EvmEngineError::not_ready(
        "latest dynamic-fee execution path is not implemented yet",
    ))
}

fn build_provider(rpc_url: &str) -> Result<DynProvider, EvmEngineError> {
    let rpc_url = rpc_url
        .parse()
        .map_err(|error| EvmEngineError::internal(format!("invalid rpc url: {error}")))?;

    Ok(ProviderBuilder::new().connect_http(rpc_url).erased())
}

async fn load_latest_execution_block(
    provider: &DynProvider,
) -> Result<ResolvedExecutionBlock, EvmEngineError> {
    let chain_id = provider.get_chain_id().await.map_err(map_rpc_error)?;
    let latest_block = provider
        .get_block(BlockId::Number(BlockNumberOrTag::Latest))
        .await
        .map_err(map_rpc_error)?
        .ok_or_else(|| EvmEngineError::internal("latest block was not returned by provider"))?;

    let state_block_id = BlockId::Number(BlockNumberOrTag::Number(latest_block.number()));

    Ok(ResolvedExecutionBlock {
        chain_id,
        state_block_id,
        block: latest_block,
    })
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
