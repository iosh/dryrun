use std::sync::Arc;

use cfx_executor::{
    machine::{Machine, VmFactory},
    state::State,
};
use cfx_parameters::consensus::TRANSACTION_DEFAULT_EPOCH_BOUND;
use cfx_statedb::Result as StateDbResult;
use cfx_types::U256;
use cfx_vm_types::{Env, Spec};
use primitives::{BlockNumber, SignedTransaction};
use tokio::runtime::Handle;

use crate::state::{ConfluxStatePoint, RemoteStateProvider, new_rpc_backed_state};

use super::{ExecutionBlockContext, params::mainnet_common_params};

pub fn build_rpc_backed_state(
    state_point: ConfluxStatePoint,
    provider: Arc<dyn RemoteStateProvider>,
    runtime_handle: Handle,
) -> StateDbResult<State> {
    new_rpc_backed_state(state_point, provider, runtime_handle)
}

fn next_execution_block_number(pivot_block_number: BlockNumber) -> BlockNumber {
    // The state/context we resolve is the parent snapshot. The simulated
    // transaction executes in the next block, matching Conflux block assembly.
    pivot_block_number + 1
}

fn next_execution_epoch_height(pivot_epoch_height: u64) -> u64 {
    // Epoch-dependent fork rules are evaluated at the execution epoch, not the
    // parent state epoch used for reads.
    pivot_epoch_height + 1
}

pub fn build_transaction_env(
    machine: &Machine,
    state: &State,
    tx: &SignedTransaction,
    input: &ExecutionBlockContext,
) -> Env {
    let execution_block_number = next_execution_block_number(input.pivot_block_number);
    let epoch_height = next_execution_epoch_height(input.pivot_epoch_height);
    let base_gas_price = input.base_fees.into_space_map();
    // Derived from state, not from public block RPC.
    let burnt_gas_price = base_gas_price.map_all(|x| state.burnt_gas_price(x));

    Env {
        chain_id: machine.params().chain_id_map(epoch_height),
        number: execution_block_number,
        author: input.author,
        timestamp: input.timestamp,
        difficulty: U256::zero(),
        gas_limit: *tx.gas(),
        last_hash: input.epoch_hash,
        accumulated_gas_used: U256::zero(),
        epoch_height,
        pos_view: input.consensus.pos_view,
        finalized_epoch: input.consensus.finalized_epoch,
        // Upstream verification default, not a public block field.
        transaction_epoch_bound: TRANSACTION_DEFAULT_EPOCH_BOUND,
        base_gas_price,
        burnt_gas_price,
        transaction_hash: tx.hash(),
    }
}

pub fn build_mainnet_machine() -> Machine {
    Machine::new_with_builtin(mainnet_common_params(), VmFactory::default())
}

pub fn build_execution_spec(machine: &Machine, env: &Env) -> Spec {
    machine.spec(env.number, env.epoch_height)
}
