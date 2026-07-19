mod artifacts;
mod change_extraction;
mod contract_reads;
mod env;
mod fee_settlement;
mod outcome;
mod provider;
mod read_call;

use self::artifacts::ExecutionArtifacts;
use self::{
    change_extraction::{
        build_transaction_changes, check_native_balances, collect_change_candidates,
    },
    env::{create_block_env, create_cfg_env, create_tx_env},
    fee_settlement::TransactionFeeSettlement,
    outcome::{build_execution_artifacts, build_invalid_transaction_artifacts, build_simulation},
    provider::{AlloyCacheDb, build_provider, create_database, resolve_execution_block},
};

use crate::{
    EvmEngineError, EvmExecutionInput, EvmSimulation, EvmTransaction,
    chain_spec::resolve_execution_spec_id,
    change_observation::ChangeObservationInspector,
    transaction_changes::{ChangeDataRequests, collect_change_data_requests},
};
use revm::{
    Context, ExecuteCommitEvm, InspectEvm, MainBuilder, MainContext, MainnetEvm,
    context::{BlockEnv, CfgEnv, TxEnv},
    context_interface::{result::EVMError, transaction::Transaction},
};

pub(super) type MainnetEvmWithDb<DB, INSP = ()> =
    MainnetEvm<Context<BlockEnv, TxEnv, CfgEnv, DB>, INSP>;
pub(super) type MainnetAlloyEvm<INSP = ()> = MainnetEvmWithDb<AlloyCacheDb, INSP>;

pub(crate) async fn simulate_execution(
    rpc_url: &str,
    input: EvmExecutionInput,
) -> Result<EvmSimulation, EvmEngineError> {
    let EvmExecutionInput { block, transaction } = input;
    let provider = build_provider(rpc_url)?;
    let resolved_block = resolve_execution_block(&provider, &block).await?;
    let db = create_database(&provider, &resolved_block)?;
    let spec_id = resolve_execution_spec_id(
        resolved_block.chain_id,
        resolved_block.block.number(),
        resolved_block.block.header.timestamp,
    )?;
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

fn execute_transaction(
    db: AlloyCacheDb,
    cfg_env: CfgEnv,
    block_env: BlockEnv,
    tx_env: TxEnv,
    resolved_block: &provider::ResolvedExecutionBlock,
    transaction: &EvmTransaction,
) -> Result<EvmSimulation, EvmEngineError> {
    let effective_gas_price = tx_env.effective_gas_price(block_env.basefee as u128);
    let base_fee_per_gas = block_env.basefee;
    let caller = tx_env.caller;
    let beneficiary = block_env.beneficiary;

    // Change observations are collected during execution so candidates,
    // data requests, and native balances can be checked before committing state.
    let mut evm = Context::mainnet()
        .with_db(db)
        .modify_cfg_chained(|cfg| *cfg = cfg_env)
        .modify_block_chained(|block| *block = block_env)
        .build_mainnet_with_inspector(ChangeObservationInspector::new());

    let (artifacts, change_candidates, change_data_requests) = match evm.inspect_tx(tx_env) {
        Ok(result_and_state) => {
            let result = result_and_state.result;
            let state = result_and_state.state;

            let observation_inspector = std::mem::take(&mut evm.inspector);
            let observations = observation_inspector.into_observations();
            let fee_settlement =
                TransactionFeeSettlement::new(result.gas(), effective_gas_price, base_fee_per_gas)?;

            let artifacts = build_execution_artifacts(
                result,
                observations,
                resolved_block,
                fee_settlement.clone(),
            );
            let change_candidates = collect_change_candidates(&artifacts)?;
            let change_data_requests = collect_change_data_requests(&change_candidates);
            check_native_balances(
                &state,
                &change_candidates,
                caller,
                beneficiary,
                &fee_settlement,
            )?;

            evm.commit(state);

            (artifacts, change_candidates, change_data_requests)
        }
        Err(EVMError::Transaction(error)) => (
            build_invalid_transaction_artifacts(resolved_block, transaction, error),
            Vec::new(),
            ChangeDataRequests::default(),
        ),
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

    let changes = build_transaction_changes(
        &mut evm,
        transaction,
        artifacts.chain_id,
        change_candidates,
        change_data_requests,
    );

    Ok(build_simulation(artifacts, changes))
}
