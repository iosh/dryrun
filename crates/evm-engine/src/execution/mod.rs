mod contract_reads;
mod env;
mod fee_settlement;
mod outcome;
mod provider;
mod read_call;
mod token_state_reads;

use self::{
    contract_reads::load_change_metadata,
    env::{create_block_env, create_cfg_env, create_tx_env},
    fee_settlement::TransactionFeeSettlement,
    outcome::{build_execution, build_not_executed},
    provider::{AlloyCacheDb, build_provider, create_database, resolve_execution_block},
    token_state_reads::read_token_state_values,
};

use crate::{
    EvmEngineError, EvmExecutionInput, EvmSimulation, EvmTransaction,
    chain_spec::resolve_execution_spec_id,
    change_observation::ChangeObservationInspector,
    transaction_changes::{
        build_changes, check_erc20_changes, check_erc721_changes, check_erc1155_movements,
        check_native_balances, check_operator_approvals, check_token_contracts, collect_candidates,
        collect_change_metadata_requests, collect_token_state_keys, sort_changes_by_position,
    },
};
use revm::{
    Context, ExecuteCommitEvm, InspectEvm, MainBuilder, MainContext, MainnetEvm,
    context::{BlockEnv, CfgEnv, TxEnv},
    context_interface::{
        result::{EVMError, ExecutionResult},
        transaction::Transaction,
    },
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

    // Change observations are collected during execution so candidates and
    // pre-state facts can be checked before committing the transaction state.
    let mut evm = Context::mainnet()
        .with_db(db)
        .modify_cfg_chained(|cfg| *cfg = cfg_env)
        .modify_block_chained(|block| *block = block_env)
        .build_mainnet_with_inspector(ChangeObservationInspector::new());

    let (execution, mut positioned_changes) = match evm.inspect_tx(tx_env) {
        Ok(result_and_state) => {
            let result = result_and_state.result;
            let state = result_and_state.state;

            let observation_inspector = std::mem::take(&mut evm.inspector);
            let observations = observation_inspector.into_observations();
            let fee_settlement =
                TransactionFeeSettlement::new(result.gas(), effective_gas_price, base_fee_per_gas)?;

            let change_candidates = if matches!(&result, ExecutionResult::Success { .. }) {
                collect_candidates(&observations)?
            } else {
                Vec::new()
            };
            let token_state_keys = collect_token_state_keys(&change_candidates);
            let execution = build_execution(result, resolved_block, &fee_settlement);

            let mut positioned_changes = check_native_balances(
                &state,
                &change_candidates,
                caller,
                beneficiary,
                fee_settlement.gas_precharge,
                fee_settlement.caller_refund,
                fee_settlement.beneficiary_reward,
            )?;

            let before_token_state = read_token_state_values(
                &mut evm,
                transaction,
                resolved_block.chain_id,
                &token_state_keys,
            )?;

            evm.commit(state);

            let after_token_state = read_token_state_values(
                &mut evm,
                transaction,
                resolved_block.chain_id,
                &token_state_keys,
            )?;

            check_token_contracts(
                &change_candidates,
                &token_state_keys,
                &before_token_state,
                &after_token_state,
            )?;

            positioned_changes.extend(check_erc20_changes(
                &change_candidates,
                &token_state_keys,
                &before_token_state,
                &after_token_state,
            )?);

            positioned_changes.extend(check_erc721_changes(
                &change_candidates,
                &token_state_keys,
                &before_token_state,
                &after_token_state,
            )?);

            positioned_changes.extend(check_erc1155_movements(
                &change_candidates,
                &token_state_keys,
                &before_token_state,
                &after_token_state,
            )?);

            positioned_changes.extend(check_operator_approvals(
                &change_candidates,
                &token_state_keys,
                &before_token_state,
                &after_token_state,
            )?);

            (execution, positioned_changes)
        }
        Err(EVMError::Transaction(error)) => (
            build_not_executed(resolved_block, transaction, error),
            Vec::new(),
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

    let changes = if positioned_changes.is_empty() {
        Vec::new()
    } else {
        sort_changes_by_position(&mut positioned_changes);
        let requests = collect_change_metadata_requests(&positioned_changes);
        let metadata =
            load_change_metadata(&mut evm, transaction, resolved_block.chain_id, requests);

        build_changes(positioned_changes, &metadata)
    };

    Ok(EvmSimulation::new(execution, changes))
}
