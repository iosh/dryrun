use cfx_execute_helper::estimation::{EstimateExt, EstimateRequest, EstimationContext};
use cfx_executor::{
    executive::ExecutionOutcome,
    machine::{Machine, VmFactory},
    state::State,
};
use cfx_statedb::Result as StateDbResult;
use cfx_vm_types::{Env, Spec};

use std::sync::Arc;

use cfx_types::{Address, AddressSpaceUtil, H256, SpaceMap, U256};

use crate::state::{
    ConfluxStateSnapshot, EspaceRpcBlock, RemoteStateProvider, RpcBackedStorage, new_state_db,
};

use cfx_parameters::consensus::TRANSACTION_DEFAULT_EPOCH_BOUND;
use primitives::{BlockNumber, SignedTransaction, transaction::EthereumTransaction};

use cfx_rpc_cfx_types::Block as NativeRpcBlock;
use thiserror::Error;

mod params;
pub use params::pinned_mainnet_common_params;
// Public block RPC does not expose the upstream-resolved PoS view/decision height.
#[derive(Debug, Clone, Copy)]
pub struct VirtualCallFinalityContext {
    pub pos_view: Option<u64>,
    pub finalized_epoch: Option<u64>,
}
impl VirtualCallFinalityContext {
    pub fn unavailable() -> Self {
        Self {
            pos_view: None,
            finalized_epoch: None,
        }
    }
}

// Native and eSpace base fees come from different public RPC block views.
#[derive(Debug, Clone, Copy)]
pub struct VirtualCallBaseGasPriceInput {
    pub native_base_fee_per_gas: Option<U256>,
    pub ethereum_base_fee_per_gas: Option<U256>,
}

impl VirtualCallBaseGasPriceInput {
    pub fn into_space_map(self) -> SpaceMap<U256> {
        SpaceMap::new(
            self.native_base_fee_per_gas.unwrap_or(U256::zero()),
            self.ethereum_base_fee_per_gas.unwrap_or(U256::zero()),
        )
    }
}

#[derive(Debug, Clone)]
pub struct VirtualCallNativePivotBlockInput {
    pub block_number: BlockNumber,
    pub epoch_height: u64,
    pub author: Address,
    pub timestamp: u64,
    pub hash: H256,
    pub base_fee_per_gas: Option<U256>,
}

#[derive(Debug, Clone, Copy)]
pub struct VirtualCallEspaceBlockInput {
    pub base_fee_per_gas: Option<U256>,
}

#[derive(Debug, Clone)]
pub struct VirtualCallBlockContextInput {
    pub pivot_block_number: BlockNumber,
    pub pivot_epoch_height: u64,
    pub author: Address,
    pub timestamp: u64,
    pub epoch_hash: H256,
    pub finality: VirtualCallFinalityContext,
    pub base_gas_price: VirtualCallBaseGasPriceInput,
}
#[derive(Debug, Error)]
pub enum VirtualCallBlockContextError {
    #[error("native pivot block is missing blockNumber")]
    MissingBlockNumber,
    #[error("native pivot block {field} exceeds u64: {value:?}")]
    U64Overflow { field: &'static str, value: U256 },
}

#[derive(Debug, Clone, Copy)]
pub struct VirtualCallEstimateRequestInput {
    pub has_sender: bool,
    pub has_gas_limit: bool,
    pub has_gas_price: bool,
    pub has_nonce: bool,
    pub collect_access_list: bool,
}

#[derive(Debug, Clone)]
pub struct VirtualCallTransactionInput {
    pub tx: EthereumTransaction,
    pub sender: Address,
}

#[derive(Debug, Clone)]
pub struct VirtualCallInput {
    pub block_context: VirtualCallBlockContextInput,
    pub transaction: VirtualCallTransactionInput,
    pub estimate_request: VirtualCallEstimateRequestInput,
}

pub fn build_virtual_call_native_pivot_block_input(
    block: NativeRpcBlock,
) -> Result<VirtualCallNativePivotBlockInput, VirtualCallBlockContextError> {
    Ok(VirtualCallNativePivotBlockInput {
        block_number: required_block_number(block.block_number)?,
        epoch_height: u256_to_u64(block.height, "height")?,
        author: block.miner.hex_address,
        timestamp: u256_to_u64(block.timestamp, "timestamp")?,
        hash: block.hash,
        base_fee_per_gas: block.base_fee_per_gas,
    })
}
pub fn build_virtual_call_espace_block_input(block: EspaceRpcBlock) -> VirtualCallEspaceBlockInput {
    VirtualCallEspaceBlockInput {
        base_fee_per_gas: block.base_fee_per_gas,
    }
}

pub fn build_virtual_call_block_context(
    pivot: VirtualCallNativePivotBlockInput,
    espace: VirtualCallEspaceBlockInput,
    finality: VirtualCallFinalityContext,
) -> VirtualCallBlockContextInput {
    VirtualCallBlockContextInput {
        pivot_block_number: pivot.block_number,
        pivot_epoch_height: pivot.epoch_height,
        author: pivot.author,
        timestamp: pivot.timestamp,
        epoch_hash: pivot.hash,
        finality,
        base_gas_price: VirtualCallBaseGasPriceInput {
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

fn virtual_call_execution_block_number(pivot_block_number: BlockNumber) -> BlockNumber {
    pivot_block_number + 1
}

fn virtual_call_epoch_height(pivot_epoch_height: u64) -> u64 {
    pivot_epoch_height + 1
}

pub fn build_virtual_call_env(
    machine: &Machine,
    state: &State,
    tx: &SignedTransaction,
    input: VirtualCallBlockContextInput,
) -> Env {
    let execution_block_number = virtual_call_execution_block_number(input.pivot_block_number);
    let epoch_height = virtual_call_epoch_height(input.pivot_epoch_height);
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

pub fn build_virtual_call_machine() -> Machine {
    Machine::new_with_builtin(pinned_mainnet_common_params(), VmFactory::default())
}

pub fn build_virtual_call_spec(machine: &Machine, env: &Env) -> Spec {
    machine.spec(env.number, env.epoch_height)
}

pub fn build_virtual_call_estimate_request(
    input: VirtualCallEstimateRequestInput,
) -> EstimateRequest {
    EstimateRequest {
        has_sender: input.has_sender,
        has_gas_limit: input.has_gas_limit,
        has_gas_price: input.has_gas_price,
        has_nonce: input.has_nonce,
        has_storage_limit: false, // eSpace eth_call path does not use storage_limit.
        collect_access_list: input.collect_access_list,
    }
}

pub fn fake_sign_evm_transaction(tx: EthereumTransaction, sender: Address) -> SignedTransaction {
    tx.fake_sign_rpc(sender.with_evm_space())
}

pub fn build_virtual_call_transaction(input: VirtualCallTransactionInput) -> SignedTransaction {
    fake_sign_evm_transaction(input.tx, input.sender)
}

pub fn execute_virtual_call(
    state: &mut State,
    machine: &Machine,
    input: VirtualCallInput,
) -> StateDbResult<(ExecutionOutcome, EstimateExt)> {
    let tx = build_virtual_call_transaction(input.transaction);
    let env = build_virtual_call_env(machine, state, &tx, input.block_context);
    let spec = build_virtual_call_spec(machine, &env);
    let request = build_virtual_call_estimate_request(input.estimate_request);

    let mut context = EstimationContext::new(state, &env, machine, &spec);

    context.transact_virtual(tx, request)
}

fn required_block_number(value: Option<U256>) -> Result<BlockNumber, VirtualCallBlockContextError> {
    value
        .ok_or(VirtualCallBlockContextError::MissingBlockNumber)
        .and_then(|value| u256_to_u64(value, "blockNumber"))
}

fn u256_to_u64(value: U256, field: &'static str) -> Result<u64, VirtualCallBlockContextError> {
    if value > U256::from(u64::MAX) {
        return Err(VirtualCallBlockContextError::U64Overflow { field, value });
    }

    Ok(value.as_u64())
}
