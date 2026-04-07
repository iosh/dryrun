mod artifacts;
mod change_extraction;
mod env;
mod outcome;
mod provider;

pub(crate) use self::artifacts::ExecutionArtifacts;
use self::{
    env::{create_block_env, create_cfg_env, create_tx_env, validate_requested_chain_id},
    outcome::{build_execution_artifacts, build_invalid_transaction_artifacts, build_simulation},
    provider::{AlloyCacheDb, build_provider, create_database, resolve_execution_block},
};

use crate::{
    EvmEngineError, EvmExecutionInput, EvmSimulation, EvmTransaction,
    chain_spec::resolve_execution_spec_id, change_observation::ChangeObservationInspector,
};
use revm::{
    Context, InspectCommitEvm, MainBuilder, MainContext, MainnetEvm,
    context::{BlockEnv, CfgEnv, TxEnv},
    context_interface::result::EVMError,
};

pub(super) type MainnetAlloyEvm<INSP = ()> =
    MainnetEvm<Context<BlockEnv, TxEnv, CfgEnv, AlloyCacheDb>, INSP>;

pub(crate) async fn simulate_execution(
    rpc_url: &str,
    input: EvmExecutionInput,
) -> Result<EvmSimulation, EvmEngineError> {
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

fn execute_transaction(
    db: AlloyCacheDb,
    cfg_env: CfgEnv,
    block_env: BlockEnv,
    tx_env: TxEnv,
    resolved_block: &provider::ResolvedExecutionBlock,
    transaction: &EvmTransaction,
) -> Result<EvmSimulation, EvmEngineError> {
    // Change observations are collected during execution so semantic detection
    // can reuse the same finalized state snapshot afterward.
    let mut evm = Context::mainnet()
        .with_db(db)
        .modify_cfg_chained(|cfg| *cfg = cfg_env)
        .modify_block_chained(|block| *block = block_env)
        .build_mainnet_with_inspector(ChangeObservationInspector::new());

    let artifacts = match evm.inspect_tx_commit(tx_env) {
        Ok(result) => {
            let observation_inspector = std::mem::take(&mut evm.inspector);
            let observations = observation_inspector.into_observations();

            build_execution_artifacts(result, observations, resolved_block)
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

    Ok(build_simulation(&mut evm, artifacts, transaction))
}
