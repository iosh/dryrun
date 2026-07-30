mod env;
mod fee_settlement;
mod metadata_reads;
mod outcome;
mod provider;
mod read_call;
mod token_state_reads;

use self::{
    env::{create_block_env, create_cfg_env, create_tx_env},
    fee_settlement::TransactionFeeSettlement,
    metadata_reads::load_change_metadata,
    outcome::{build_execution, build_not_executed},
    provider::{AlloyCacheDb, create_database},
    token_state_reads::read_token_state_values,
};

use crate::{
    EvmEngineError, EvmExecutionInput, EvmSimulation, EvmTransaction, ResolvedBlock,
    chain_spec::resolve_execution_spec_id,
    changes::{
        ChangeObservationInspector, PositionedChange, check_native_balances,
        collect_contract_candidates, collect_native_candidates, into_enriched_changes,
        sort_changes_by_position,
    },
};
use alloy::providers::RootProvider;
use contract_standards::{MetadataRequests, state_requirements, verify};
use revm::{
    Context, ExecuteCommitEvm, InspectEvm, MainBuilder, MainContext, MainnetEvm,
    context::{BlockEnv, CfgEnv, TxEnv},
    context_interface::{
        result::{EVMError, ExecutionResult},
        transaction::Transaction,
    },
};
use tokio::runtime::Handle;

pub(super) type MainnetEvmWithDb<DB, INSP = ()> =
    MainnetEvm<Context<BlockEnv, TxEnv, CfgEnv, DB>, INSP>;
pub(super) type MainnetAlloyEvm<INSP = ()> = MainnetEvmWithDb<AlloyCacheDb, INSP>;

pub(crate) fn simulate_execution(
    provider: &RootProvider,
    runtime_handle: &Handle,
    chain_id: u64,
    input: EvmExecutionInput,
) -> Result<EvmSimulation, EvmEngineError> {
    let EvmExecutionInput { block, transaction } = input;
    let resolved_block = block;
    let db = create_database(provider, runtime_handle, &resolved_block);
    let spec_id = resolve_execution_spec_id(
        chain_id,
        resolved_block.number(),
        resolved_block.header().timestamp,
    )?;
    let cfg_env = create_cfg_env(chain_id, spec_id);
    let block_env = create_block_env(&resolved_block, spec_id)?;
    let tx_env = create_tx_env(&transaction)?;

    execute_transaction(
        db,
        cfg_env,
        block_env,
        tx_env,
        chain_id,
        &resolved_block,
        &transaction,
    )
}

fn execute_transaction(
    db: AlloyCacheDb,
    cfg_env: CfgEnv,
    block_env: BlockEnv,
    tx_env: TxEnv,
    chain_id: u64,
    resolved_block: &ResolvedBlock,
    transaction: &EvmTransaction,
) -> Result<EvmSimulation, EvmEngineError> {
    let effective_gas_price = tx_env.effective_gas_price(block_env.basefee as u128);
    let base_fee_per_gas = block_env.basefee;
    let caller = tx_env.caller;
    let beneficiary = block_env.beneficiary;

    // Change observations are collected during execution so candidates and
    // pre-state facts can be checked before committing the transaction state.
    let mut evm = Context::mainnet()
        .with_db(db)
        .modify_cfg_chained(|cfg| *cfg = cfg_env)
        .modify_block_chained(|block| *block = block_env)
        .build_mainnet_with_inspector(ChangeObservationInspector::new());

    let (execution, mut positioned_changes, metadata_requests) = match evm.inspect_tx(tx_env) {
        Ok(result_and_state) => {
            let result = result_and_state.result;
            let state = result_and_state.state;
            let fee_settlement =
                TransactionFeeSettlement::new(result.gas(), effective_gas_price, base_fee_per_gas)?;
            let succeeded = matches!(&result, ExecutionResult::Success { .. });
            let execution = build_execution(result, chain_id, resolved_block, &fee_settlement);

            if !succeeded {
                return Ok(EvmSimulation::new(execution, Vec::new()));
            }

            let observation_inspector = std::mem::take(&mut evm.inspector);
            let observations = observation_inspector.into_observations();
            let native_candidates = collect_native_candidates(&observations)?;
            let candidates = collect_contract_candidates(&observations)?;
            let requirements = state_requirements(&candidates);

            let mut positioned_changes = check_native_balances(
                &state,
                &native_candidates,
                caller,
                beneficiary,
                fee_settlement.gas_precharge,
                fee_settlement.caller_refund,
                fee_settlement.beneficiary_reward,
            )?;

            let before_token_state =
                read_token_state_values(&mut evm, transaction, chain_id, &requirements)?;

            evm.commit(state);

            let after_token_state =
                read_token_state_values(&mut evm, transaction, chain_id, &requirements)?;

            let standard_changes = verify(&candidates, &before_token_state, &after_token_state)?;
            let metadata_requests = MetadataRequests::from_changes(&standard_changes);
            positioned_changes.extend(standard_changes.into_iter().map(PositionedChange::from));

            (execution, positioned_changes, metadata_requests)
        }
        Err(EVMError::Transaction(error)) => {
            let execution = build_not_executed(chain_id, resolved_block, transaction, error);
            return Ok(EvmSimulation::new(execution, Vec::new()));
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

    let changes = if positioned_changes.is_empty() {
        Vec::new()
    } else {
        sort_changes_by_position(&mut positioned_changes);
        let metadata = load_change_metadata(&mut evm, transaction, chain_id, metadata_requests)?;

        into_enriched_changes(positioned_changes, &metadata)
    };

    Ok(EvmSimulation::new(execution, changes))
}
