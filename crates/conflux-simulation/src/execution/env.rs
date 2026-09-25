use std::sync::Arc;

use cfx_executor::{machine::Machine, state::State};
use cfx_parameters::consensus::TRANSACTION_DEFAULT_EPOCH_BOUND;
use cfx_statedb::Result as StateDbResult;
use cfx_types::U256;
use cfx_vm_types::Env;
use primitives::SignedTransaction;
use tokio::runtime::Handle;

use crate::{
    context::ExecutionBlockContext,
    state::{ConfluxStateSource, new_conflux_state},
};

pub(crate) fn build_conflux_state(
    source: impl Into<Arc<ConfluxStateSource>>,
    runtime_handle: Handle,
) -> StateDbResult<State> {
    new_conflux_state(source.into(), runtime_handle)
}

pub(crate) fn build_transaction_env(
    machine: &Machine,
    state: &State,
    tx: &SignedTransaction,
    input: &ExecutionBlockContext,
) -> Env {
    let base_gas_price = input.base_gas_price;
    // Derived from state, not from public block RPC.
    let burnt_gas_price = base_gas_price.map_all(|x| state.burnt_gas_price(x));

    Env {
        chain_id: machine.params().chain_id_map(input.epoch_height),
        number: input.number,
        author: input.author,
        timestamp: input.timestamp,
        difficulty: U256::zero(),
        gas_limit: *tx.gas(),
        last_hash: input.epoch_hash,
        accumulated_gas_used: U256::zero(),
        epoch_height: input.epoch_height,
        pos_view: input.consensus.pos_view,
        finalized_epoch: input.consensus.finalized_epoch,
        // Upstream verification default, not a public block field.
        transaction_epoch_bound: TRANSACTION_DEFAULT_EPOCH_BOUND,
        base_gas_price,
        burnt_gas_price,
        transaction_hash: tx.hash(),
    }
}
