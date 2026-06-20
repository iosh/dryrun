use cfx_executor::{
    executive::{ExecutionOutcome, ExecutiveContext, TransactOptions},
    machine::{Machine, VmFactory},
    state::State,
};
use cfx_statedb::Result as StateDbResult;
use cfx_vm_types::{Env, Spec};

use std::sync::Arc;

use cfx_types::{Address, AddressSpaceUtil, H256, SpaceMap, U256};

use crate::state::{
    ConfluxStateSnapshot, EspaceRpcBlock, NativeRpcBlock, RemoteStateProvider, RpcBackedStorage,
    new_state_db,
};

use cfx_parameters::consensus::TRANSACTION_DEFAULT_EPOCH_BOUND;
use primitives::{BlockNumber, SignedTransaction, transaction::EthereumTransaction};

use thiserror::Error;

mod params;
pub use params::pinned_mainnet_common_params;
// Public block RPC does not expose the upstream-resolved PoS view/decision height.
#[derive(Debug, Clone, Copy)]
pub struct ExecutionFinalityContext {
    pub pos_view: Option<u64>,
    pub finalized_epoch: Option<u64>,
}
impl ExecutionFinalityContext {
    pub fn unavailable() -> Self {
        Self {
            pos_view: None,
            finalized_epoch: None,
        }
    }
}

// Native and eSpace base fees come from different public RPC block views.
#[derive(Debug, Clone, Copy)]
pub struct ExecutionBaseGasPriceInput {
    pub native_base_fee_per_gas: Option<U256>,
    pub ethereum_base_fee_per_gas: Option<U256>,
}

impl ExecutionBaseGasPriceInput {
    pub fn into_space_map(self) -> SpaceMap<U256> {
        SpaceMap::new(
            self.native_base_fee_per_gas.unwrap_or(U256::zero()),
            self.ethereum_base_fee_per_gas.unwrap_or(U256::zero()),
        )
    }
}

#[derive(Debug, Clone)]
pub struct NativePivotBlockContext {
    pub block_number: BlockNumber,
    pub epoch_height: u64,
    pub author: Address,
    pub timestamp: u64,
    pub hash: H256,
    pub base_fee_per_gas: Option<U256>,
}

#[derive(Debug, Clone, Copy)]
pub struct EspaceBlockContext {
    pub base_fee_per_gas: Option<U256>,
}

#[derive(Debug, Clone)]
pub struct ExecutionBlockContext {
    pub pivot_block_number: BlockNumber,
    pub pivot_epoch_height: u64,
    pub author: Address,
    pub timestamp: u64,
    pub epoch_hash: H256,
    pub finality: ExecutionFinalityContext,
    pub base_gas_price: ExecutionBaseGasPriceInput,
}
#[derive(Debug, Error)]
pub enum ExecutionBlockContextError {
    #[error("native pivot block is missing blockNumber")]
    MissingBlockNumber,
    #[error("native pivot block {field} exceeds u64: {value:?}")]
    U64Overflow { field: &'static str, value: U256 },
}

#[derive(Debug, Clone)]
pub struct EspaceTransactionInput {
    pub tx: EthereumTransaction,
    pub sender: Address,
}

pub struct TransactionExecutionInput {
    pub block_context: ExecutionBlockContext,
    pub transaction: EspaceTransactionInput,
}
pub fn build_native_pivot_block_context(
    block: NativeRpcBlock,
) -> Result<NativePivotBlockContext, ExecutionBlockContextError> {
    Ok(NativePivotBlockContext {
        block_number: required_block_number(block.block_number)?,
        epoch_height: u256_to_u64(block.height, "height")?,
        author: block.miner.hex_address,
        timestamp: u256_to_u64(block.timestamp, "timestamp")?,
        hash: block.hash,
        base_fee_per_gas: block.base_fee_per_gas,
    })
}
pub fn build_espace_block_context(block: EspaceRpcBlock) -> EspaceBlockContext {
    EspaceBlockContext {
        base_fee_per_gas: block.base_fee_per_gas,
    }
}

pub fn build_execution_block_context(
    pivot: NativePivotBlockContext,
    espace: EspaceBlockContext,
    finality: ExecutionFinalityContext,
) -> ExecutionBlockContext {
    ExecutionBlockContext {
        pivot_block_number: pivot.block_number,
        pivot_epoch_height: pivot.epoch_height,
        author: pivot.author,
        timestamp: pivot.timestamp,
        epoch_hash: pivot.hash,
        finality,
        base_gas_price: ExecutionBaseGasPriceInput {
            native_base_fee_per_gas: pivot.base_fee_per_gas,
            ethereum_base_fee_per_gas: espace.base_fee_per_gas,
        },
    }
}

pub fn build_rpc_backed_state(
    snapshot: ConfluxStateSnapshot,
    provider: Arc<dyn RemoteStateProvider>,
) -> StateDbResult<State> {
    let storage = RpcBackedStorage::new(snapshot, provider);
    let db = new_state_db(Box::new(storage));

    State::new(db)
}

fn next_execution_block_number(pivot_block_number: BlockNumber) -> BlockNumber {
    pivot_block_number + 1
}

fn next_execution_epoch_height(pivot_epoch_height: u64) -> u64 {
    pivot_epoch_height + 1
}

pub fn build_execution_env(
    machine: &Machine,
    state: &State,
    tx: &SignedTransaction,
    input: ExecutionBlockContext,
) -> Env {
    let execution_block_number = next_execution_block_number(input.pivot_block_number);
    let epoch_height = next_execution_epoch_height(input.pivot_epoch_height);
    let base_gas_price = input.base_gas_price.into_space_map();
    // Derived from state, not from public block RPC.
    let burnt_gas_price = base_gas_price.map_all(|x| state.burnt_gas_price(x));

    Env {
        chain_id: machine.params().chain_id_map(epoch_height),
        number: execution_block_number,
        author: input.author,
        timestamp: input.timestamp,
        difficulty: U256::zero(),
        gas_limit: tx.gas().clone(),
        last_hash: input.epoch_hash,
        accumulated_gas_used: U256::zero(),
        epoch_height,
        pos_view: input.finality.pos_view,
        finalized_epoch: input.finality.finalized_epoch,
        // Upstream verification default, not a public block field.
        transaction_epoch_bound: TRANSACTION_DEFAULT_EPOCH_BOUND,
        base_gas_price,
        burnt_gas_price,
        transaction_hash: tx.hash(),
        ..Default::default()
    }
}

pub fn build_mainnet_machine() -> Machine {
    Machine::new_with_builtin(pinned_mainnet_common_params(), VmFactory::default())
}

pub fn build_execution_spec(machine: &Machine, env: &Env) -> Spec {
    machine.spec(env.number, env.epoch_height)
}

pub fn fake_sign_evm_transaction(tx: EthereumTransaction, sender: Address) -> SignedTransaction {
    tx.fake_sign_rpc(sender.with_evm_space())
}

pub fn build_signed_transaction(input: EspaceTransactionInput) -> SignedTransaction {
    fake_sign_evm_transaction(input.tx, input.sender)
}

pub fn execute_transaction(
    state: &mut State,
    machine: &Machine,
    input: TransactionExecutionInput,
) -> StateDbResult<ExecutionOutcome> {
    let tx = build_signed_transaction(input.transaction);
    let env = build_execution_env(machine, state, &tx, input.block_context);
    let spec = build_execution_spec(machine, &env);

    let outcome = ExecutiveContext::new(state, &env, machine, &spec)
        .transact(&tx, TransactOptions::default())?;

    state.update_state_post_tx_execution(!spec.cip645.fix_eip1153);

    if let Some(burnt_fee) = outcome.try_as_executed().and_then(|e| e.burnt_fee) {
        state.burn_by_cip1559(burnt_fee);
    }

    Ok(outcome)
}

fn required_block_number(value: Option<U256>) -> Result<BlockNumber, ExecutionBlockContextError> {
    value
        .ok_or(ExecutionBlockContextError::MissingBlockNumber)
        .and_then(|value| u256_to_u64(value, "blockNumber"))
}

fn u256_to_u64(value: U256, field: &'static str) -> Result<u64, ExecutionBlockContextError> {
    if value > U256::from(u64::MAX) {
        return Err(ExecutionBlockContextError::U64Overflow { field, value });
    }

    Ok(value.as_u64())
}
