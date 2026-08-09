use cfx_executor::{machine::Machine, state::State};
use cfx_parameters::consensus::TRANSACTION_DEFAULT_EPOCH_BOUND;
use cfx_statedb::Result as StateDbResult;
use cfx_types::U256;
use cfx_vm_types::{Env, Spec};
use primitives::{BlockNumber, SignedTransaction};
use tokio::runtime::Handle;

use crate::state::{ConfluxStateSource, new_conflux_state};

use super::{ExecutionBlockContext, ExecutionBlockContextError};

pub(crate) fn build_conflux_state(
    source: ConfluxStateSource,
    runtime_handle: Handle,
) -> StateDbResult<State> {
    new_conflux_state(source, runtime_handle)
}

fn next_execution_block_number(
    pivot_block_number: BlockNumber,
) -> Result<BlockNumber, ExecutionBlockContextError> {
    // The loaded context points at the parent state. The simulated
    // transaction executes in the next block, matching Conflux block assembly.
    pivot_block_number
        .checked_add(1)
        .ok_or(ExecutionBlockContextError::NextBlockNumberOverflow { pivot_block_number })
}

fn next_execution_epoch_height(pivot_epoch_height: u64) -> Result<u64, ExecutionBlockContextError> {
    // Epoch-dependent fork rules are evaluated at the execution epoch, not the
    // parent state epoch used for reads.
    pivot_epoch_height
        .checked_add(1)
        .ok_or(ExecutionBlockContextError::NextEpochHeightOverflow { pivot_epoch_height })
}

pub(crate) fn build_transaction_env(
    machine: &Machine,
    state: &State,
    tx: &SignedTransaction,
    input: &ExecutionBlockContext,
) -> Result<Env, ExecutionBlockContextError> {
    let execution_block_number = next_execution_block_number(input.pivot_block_number)?;
    let epoch_height = next_execution_epoch_height(input.pivot_epoch_height)?;
    let base_gas_price = input.base_fees.into_space_map();
    // Derived from state, not from public block RPC.
    let burnt_gas_price = base_gas_price.map_all(|x| state.burnt_gas_price(x));

    Ok(Env {
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
    })
}

pub(crate) fn build_execution_spec(machine: &Machine, env: &Env) -> Spec {
    machine.spec(env.number, env.epoch_height)
}
