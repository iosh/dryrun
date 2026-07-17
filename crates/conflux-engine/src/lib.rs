pub mod config;
mod error;
pub mod espace;
pub mod execution;
pub mod native;
pub mod state;

use std::sync::Arc;

use crate::{
    config::ConfluxConfig,
    espace::{
        EspaceBlockRef, EspaceExecution, SimulateEspaceTransactionInput, SimulatedBlock,
        build_espace_execution, build_espace_not_executed, build_espace_transaction_input,
        validate_espace_transaction,
    },
    execution::{
        DryRunTransactionInput, ExecutionBlockContext, ExecutionConsensusContext,
        TransactionExecutionInput, build_espace_block_context, build_execution_block_context,
        build_mainnet_machine, build_native_pivot_block_context, build_rpc_backed_state,
        execute_transaction,
    },
    native::{
        NativeEpochRef, NativeExecution, NativeExecutionFailure, NativeExecutionFailureCode,
        NativeStateAnchor, NativeTransaction, NativeTransactionVariant,
        SimulateNativeTransactionInput, build_native_execution, build_native_not_executed,
        build_native_transaction_input,
    },
    state::{
        ConfluxStateAnchor, ConfluxStatePoint, EspaceRpcBlock, HttpConfluxStateProvider,
        NativeRpcBlock, RemoteStateProvider, RemoteStateReader,
    },
};
use cfx_types::U256;
pub use error::ConfluxEngineError;

use cfx_rpc_cfx_types::EpochNumber as CfxEpochNumber;
use cfx_rpc_eth_types::BlockId as EthBlockId;

pub struct ConfluxEngine {
    config: ConfluxConfig,
    provider: Arc<dyn RemoteStateProvider>,
}

impl ConfluxEngine {
    pub fn new(config: ConfluxConfig) -> Result<Self, ConfluxEngineError> {
        let provider = Arc::new(HttpConfluxStateProvider::new(config.clone())?);
        Ok(Self { config, provider })
    }

    pub fn with_provider(config: ConfluxConfig, provider: Arc<dyn RemoteStateProvider>) -> Self {
        Self { config, provider }
    }

    pub async fn simulate_espace_transaction(
        &self,
        input: SimulateEspaceTransactionInput,
    ) -> Result<EspaceExecution, ConfluxEngineError> {
        let runtime_handle = current_runtime_handle()?;
        let SimulateEspaceTransactionInput { block, transaction } = input;
        let gas_limit = transaction.gas_limit;
        let execution_context = self.resolve_espace_execution_context(&block).await?;
        let chain_id = self.config.chain.evm_chain_id;

        if let Err(failure) = validate_espace_transaction(&transaction, chain_id) {
            return Ok(build_espace_not_executed(
                chain_id,
                execution_context.simulated_block,
                gas_limit,
                failure,
            ));
        }

        let transaction = build_espace_transaction_input(transaction);

        let execution_input = TransactionExecutionInput {
            block_context: execution_context.block_context,
            transaction: DryRunTransactionInput::Espace(transaction),
        };
        let state_reader = self
            .prepare_state_reader(execution_context.state_point)
            .await?;

        tokio::task::spawn_blocking(move || {
            let mut state =
                build_rpc_backed_state(state_reader, runtime_handle).map_err(|error| {
                    ConfluxEngineError::StateAccess {
                        message: error.to_string(),
                    }
                })?;

            let machine = build_mainnet_machine();

            let outcome =
                execute_transaction(&mut state, &machine, execution_input).map_err(|error| {
                    ConfluxEngineError::StateAccess {
                        message: error.to_string(),
                    }
                })?;

            build_espace_execution(
                chain_id,
                execution_context.simulated_block,
                gas_limit,
                outcome,
            )
        })
        .await
        .map_err(|error| ConfluxEngineError::ExecutionInternal {
            message: format!("eSpace blocking execution task failed: {error}"),
        })?
    }

    pub async fn simulate_native_transaction(
        &self,
        input: SimulateNativeTransactionInput,
    ) -> Result<NativeExecution, ConfluxEngineError> {
        let runtime_handle = current_runtime_handle()?;
        let SimulateNativeTransactionInput { epoch, transaction } = input;
        let gas_limit = transaction.gas_limit;
        let execution_context = self.resolve_native_execution_context(&epoch).await?;
        let chain_id = self.config.chain.native_chain_id;
        let state_anchor = NativeStateAnchor {
            epoch_number: execution_context.state_point.anchor().epoch_number(),
            pivot_hash: execution_context.state_point.anchor().pivot_hash(),
        };

        if let Err(failure) = validate_native_transaction(&transaction, chain_id) {
            return Ok(build_native_not_executed(
                chain_id,
                state_anchor,
                gas_limit,
                failure,
            ));
        }

        let transaction = build_native_transaction_input(transaction);

        let execution_input = TransactionExecutionInput {
            block_context: execution_context.block_context,
            transaction: DryRunTransactionInput::Native(transaction),
        };
        let state_reader = self
            .prepare_state_reader(execution_context.state_point)
            .await?;

        tokio::task::spawn_blocking(move || {
            let mut state =
                build_rpc_backed_state(state_reader, runtime_handle).map_err(|error| {
                    ConfluxEngineError::StateAccess {
                        message: error.to_string(),
                    }
                })?;

            let machine = build_mainnet_machine();

            let outcome =
                execute_transaction(&mut state, &machine, execution_input).map_err(|error| {
                    ConfluxEngineError::StateAccess {
                        message: error.to_string(),
                    }
                })?;

            Ok(build_native_execution(
                chain_id,
                state_anchor,
                gas_limit,
                outcome,
            ))
        })
        .await
        .map_err(|error| ConfluxEngineError::ExecutionInternal {
            message: format!("Native blocking execution task failed: {error}"),
        })?
    }

    async fn prepare_state_reader(
        &self,
        state_point: ConfluxStatePoint,
    ) -> Result<RemoteStateReader, ConfluxEngineError> {
        RemoteStateReader::prepare(state_point, Arc::clone(&self.provider))
            .await
            .map_err(|error| ConfluxEngineError::StateAccess {
                message: error.to_string(),
            })
    }

    async fn resolve_espace_execution_context(
        &self,
        block: &EspaceBlockRef,
    ) -> Result<EspaceExecutionContext, ConfluxEngineError> {
        let espace_block = self
            .provider
            .get_espace_block_by_number(espace_block_selector(block))
            .await?
            .ok_or_else(|| ConfluxEngineError::BlockNotFound {
                block: "eSpace block".to_string(),
            })?;
        let state_anchor = state_anchor_from_espace_block(&espace_block)?;
        let native_pivot_block = self.resolve_native_pivot_block(state_anchor).await?;

        let simulated_block = SimulatedBlock {
            number: state_anchor.epoch_number(),
            hash: espace_block.hash,
        };

        let native_pivot = build_native_pivot_block_context(&native_pivot_block)?;
        validate_same_state_anchor(state_anchor, state_anchor_from_native_pivot(&native_pivot))?;
        let espace = build_espace_block_context(&espace_block);

        let block_context = build_execution_block_context(
            &native_pivot,
            &espace,
            ExecutionConsensusContext::default(),
        );

        Ok(EspaceExecutionContext {
            block_context,
            state_point: ConfluxStatePoint::from_anchor(state_anchor),
            simulated_block,
        })
    }

    async fn resolve_native_execution_context(
        &self,
        epoch: &NativeEpochRef,
    ) -> Result<NativeExecutionContext, ConfluxEngineError> {
        let native_pivot_block = self
            .provider
            .get_native_block_by_epoch_number(native_epoch_selector(epoch))
            .await?
            .ok_or_else(|| ConfluxEngineError::BlockNotFound {
                block: "native pivot block".to_string(),
            })?;

        let native_pivot = build_native_pivot_block_context(&native_pivot_block)?;
        let state_anchor = state_anchor_from_native_pivot(&native_pivot);
        let espace_block = self.resolve_espace_block_for_anchor(state_anchor).await?;
        validate_same_state_anchor(state_anchor, state_anchor_from_espace_block(&espace_block)?)?;
        let espace = build_espace_block_context(&espace_block);
        let block_context = build_execution_block_context(
            &native_pivot,
            &espace,
            ExecutionConsensusContext::default(),
        );

        Ok(NativeExecutionContext {
            block_context,
            state_point: ConfluxStatePoint::from_anchor(state_anchor),
        })
    }

    async fn resolve_native_pivot_block(
        &self,
        anchor: ConfluxStateAnchor,
    ) -> Result<NativeRpcBlock, ConfluxEngineError> {
        self.provider
            .get_native_block_by_epoch_number(native_epoch_from_anchor(anchor))
            .await?
            .ok_or_else(|| ConfluxEngineError::BlockNotFound {
                block: "native pivot block".to_string(),
            })
    }

    async fn resolve_espace_block_for_anchor(
        &self,
        anchor: ConfluxStateAnchor,
    ) -> Result<EspaceRpcBlock, ConfluxEngineError> {
        self.provider
            .get_espace_block_by_number(anchor.espace_block())
            .await?
            .ok_or_else(|| ConfluxEngineError::BlockNotFound {
                block: "eSpace block".to_string(),
            })
    }
}
struct EspaceExecutionContext {
    block_context: ExecutionBlockContext,
    state_point: ConfluxStatePoint,
    simulated_block: SimulatedBlock,
}

struct NativeExecutionContext {
    block_context: ExecutionBlockContext,
    state_point: ConfluxStatePoint,
}

fn current_runtime_handle() -> Result<tokio::runtime::Handle, ConfluxEngineError> {
    tokio::runtime::Handle::try_current().map_err(|error| ConfluxEngineError::ExecutionInternal {
        message: format!("Conflux simulation requires a Tokio runtime: {error}"),
    })
}

fn espace_block_selector(block: &EspaceBlockRef) -> EthBlockId {
    match block {
        EspaceBlockRef::Latest => EthBlockId::Latest,
        EspaceBlockRef::Number(number) => EthBlockId::Num(*number),
    }
}

fn native_epoch_selector(epoch: &NativeEpochRef) -> CfxEpochNumber {
    match epoch {
        NativeEpochRef::LatestState => CfxEpochNumber::LatestState,
        NativeEpochRef::Number(number) => CfxEpochNumber::Num((*number).into()),
    }
}

fn native_epoch_from_anchor(anchor: ConfluxStateAnchor) -> CfxEpochNumber {
    CfxEpochNumber::Num(anchor.epoch_number().into())
}

fn state_anchor_from_espace_block(
    block: &EspaceRpcBlock,
) -> Result<ConfluxStateAnchor, ConfluxEngineError> {
    Ok(ConfluxStateAnchor::new(
        espace_block_number(block)?,
        block.hash,
    ))
}

fn state_anchor_from_native_pivot(
    pivot: &execution::NativePivotBlockContext,
) -> ConfluxStateAnchor {
    ConfluxStateAnchor::new(pivot.epoch_height, pivot.hash)
}

fn validate_same_state_anchor(
    expected: ConfluxStateAnchor,
    actual: ConfluxStateAnchor,
) -> Result<(), ConfluxEngineError> {
    if actual != expected {
        return Err(ConfluxEngineError::StateAnchorInconsistent);
    }

    Ok(())
}

fn validate_native_transaction(
    transaction: &NativeTransaction,
    expected_chain_id: u32,
) -> Result<(), NativeExecutionFailure> {
    if transaction.chain_id != expected_chain_id {
        return Err(NativeExecutionFailure {
            code: NativeExecutionFailureCode::ChainIdMismatch,
            message: format!(
                "transaction chain id {} does not match engine chain id {}",
                transaction.chain_id, expected_chain_id
            ),
            reason: None,
        });
    }

    match &transaction.variant {
        NativeTransactionVariant::Cip155 { gas_price }
        | NativeTransactionVariant::Cip2930 { gas_price, .. } => {
            if gas_price.is_zero() {
                return Err(NativeExecutionFailure {
                    code: NativeExecutionFailureCode::ZeroGasPrice,
                    message: "transaction gas price must be greater than zero".to_string(),
                    reason: None,
                });
            }
        }
        NativeTransactionVariant::Cip1559 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            ..
        } => {
            if max_fee_per_gas.is_zero() {
                return Err(NativeExecutionFailure {
                    code: NativeExecutionFailureCode::ZeroGasPrice,
                    message: "transaction max fee per gas must be greater than zero".to_string(),
                    reason: None,
                });
            }

            if max_priority_fee_per_gas > max_fee_per_gas {
                return Err(NativeExecutionFailure {
                    code: NativeExecutionFailureCode::PriorityFeeExceedsMaxFee,
                    message: format!(
                        "max priority fee per gas {} exceeds max fee per gas {}",
                        max_priority_fee_per_gas, max_fee_per_gas
                    ),
                    reason: None,
                });
            }
        }
    }

    Ok(())
}

fn espace_block_number(block: &EspaceRpcBlock) -> Result<u64, ConfluxEngineError> {
    if block.number > U256::from(u64::MAX) {
        return Err(ConfluxEngineError::InvalidBlockContext {
            message: format!("eSpace block number exceeds u64: {:?}", block.number),
        });
    }

    Ok(block.number.as_u64())
}
