use alloy::{
    consensus::{BlockHeader, Header, Sealed},
    network::Ethereum,
    providers::{DynProvider, Provider},
};
use contract_standards::legacy::{metadata_requests, state_requirements, verify};
use simulation_changes::{
    ChangeMetadata, PositionedChange, into_enriched_changes, sort_changes_by_position,
};
use tokio::runtime::Handle;

use crate::{
    CompleteTransaction, EthereumChainSpec, EvmBlockContext, EvmExecutionObserver,
    EvmExecutionOutcome, EvmInitializationError, EvmSimulation, EvmSimulationError,
    EvmSimulationRequest, EvmTransactionExecution, EvmTransactionExecutor,
    changes::{
        analyze_native_changes, collect_standard_candidates, load_standard_metadata,
        read_standard_state_values,
    },
    create_database, map_executed_outcome, resolve_block,
};

#[derive(Debug, Clone)]
pub struct EvmTransactionSimulator {
    provider: DynProvider<Ethereum>,
    chain_spec: EthereumChainSpec,
}

impl EvmTransactionSimulator {
    pub async fn ethereum_mainnet(
        provider: DynProvider<Ethereum>,
    ) -> Result<Self, EvmInitializationError> {
        let chain_spec = EthereumChainSpec::mainnet();
        let actual_chain_id = provider
            .get_chain_id()
            .await
            .map_err(EvmInitializationError::chain_id_request)?;

        if actual_chain_id != chain_spec.chain_id() {
            return Err(EvmInitializationError::ChainIdMismatch {
                expected: chain_spec.chain_id(),
                actual: actual_chain_id,
            });
        }

        Ok(Self {
            provider,
            chain_spec,
        })
    }

    pub async fn simulate(
        &self,
        request: EvmSimulationRequest,
    ) -> Result<EvmSimulation, EvmSimulationError> {
        let EvmSimulationRequest { block, transaction } = request;
        transaction.validate_requirements()?;
        let block = resolve_block(&self.provider, block).await?;
        let transaction =
            crate::complete_transaction(transaction, &self.provider, &block, self.chain_spec)
                .await?;

        let provider = self.provider.clone();
        let chain_spec = self.chain_spec;
        let runtime_handle = Handle::current();

        tokio::task::spawn_blocking(move || {
            simulate_blocking(provider, runtime_handle, chain_spec, block, transaction)
        })
        .await
        .map_err(EvmSimulationError::execution_task)?
    }
}

fn simulate_blocking(
    provider: DynProvider<Ethereum>,
    runtime_handle: Handle,
    chain_spec: EthereumChainSpec,
    block: Sealed<Header>,
    transaction: CompleteTransaction,
) -> Result<EvmSimulation, EvmSimulationError> {
    let chain_id = chain_spec.chain_id();
    let context = EvmBlockContext {
        number: block.number(),
        hash: block.hash(),
    };
    let database = create_database(provider, runtime_handle, block.hash());
    let executor =
        EvmTransactionExecutor::new(database, block, chain_spec, EvmExecutionObserver::new())?;

    let mut output = match executor.execute(&transaction)? {
        EvmTransactionExecution::Executed(output) => output,
        EvmTransactionExecution::NotExecuted(rejection) => {
            return Ok(EvmSimulation {
                context,
                transaction,
                execution: EvmExecutionOutcome::NotExecuted(rejection),
                changes: Vec::new(),
            });
        }
    };

    if !output.is_success() {
        let (engine_result, execution_result) = (*output).into_outcome_parts();
        let execution = map_executed_outcome(engine_result, &transaction, execution_result)?;
        return Ok(EvmSimulation {
            context,
            transaction,
            execution,
            changes: Vec::new(),
        });
    }

    let observations = output.take_observations();
    let candidates = collect_standard_candidates(&observations)?;
    let requirements = state_requirements(&candidates);
    let mut positioned_changes = analyze_native_changes(
        output.transition()?,
        &observations,
        output.caller(),
        output.beneficiary(),
        output.fee_settlement(),
    )
    .map_err(|error| EvmSimulationError::changes(error.to_string()))?;

    let before_token_state =
        read_standard_state_values(output.evm_mut(), &transaction, chain_id, &requirements)?;

    output.apply_transition()?;

    let after_token_state =
        read_standard_state_values(output.evm_mut(), &transaction, chain_id, &requirements)?;
    let standard_changes = verify(&candidates, &before_token_state, &after_token_state)?;
    let metadata_requests = metadata_requests(&standard_changes);
    positioned_changes.extend(standard_changes.into_iter().map(PositionedChange::from));

    let changes = if positioned_changes.is_empty() {
        Vec::new()
    } else {
        sort_changes_by_position(&mut positioned_changes);
        let standard_metadata =
            load_standard_metadata(output.evm_mut(), &transaction, chain_id, metadata_requests)?;
        let metadata = ChangeMetadata::new(native_metadata(chain_spec), standard_metadata);
        into_enriched_changes(positioned_changes, &metadata)
    };

    let (engine_result, execution_result) = (*output).into_outcome_parts();
    let execution = map_executed_outcome(engine_result, &transaction, execution_result)?;

    Ok(EvmSimulation {
        context,
        transaction,
        execution,
        changes,
    })
}

fn native_metadata(chain_spec: EthereumChainSpec) -> crate::NativeMetadata {
    let native_currency = chain_spec.native_currency();
    crate::NativeMetadata {
        name: Some(native_currency.name.to_string()),
        symbol: Some(native_currency.symbol.to_string()),
        decimals: Some(native_currency.decimals),
    }
}
