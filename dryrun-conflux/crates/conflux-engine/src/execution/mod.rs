use cfx_execute_helper::estimation::EstimationContext;
use cfx_executor::{
    machine::{Machine, VmFactory},
    spec::CommonParams,
    state::State,
};
use cfx_statedb::Result as StateDbResult;
use cfx_vm_types::{Env, Spec};

use std::{collections::BTreeMap, sync::Arc};

use cfx_types::{Address, H256, Space, SpaceMap, U256};
use primitives::BlockNumber;

use crate::state::{ConfluxStateSnapshot, RemoteStateProvider, RpcBackedStorage, new_state_db};

#[derive(Debug, Clone)]
pub struct VirtualCallEnvInput {
    pub native_chain_id: u32,
    pub ethereum_chain_id: u32,
    pub block_number: BlockNumber,
    pub epoch_height: u64,
    pub author: Address,
    pub timestamp: u64,
    pub gas_limit: U256,
    pub last_hash: H256,
    pub pos_view: Option<u64>,
    pub finalized_epoch: Option<u64>,
    pub transaction_epoch_bound: u64,
    pub base_gas_price: SpaceMap<U256>,
    pub burnt_gas_price: SpaceMap<U256>,
    pub transaction_hash: H256,
}

pub fn build_rpc_backed_state(
    snapshot: ConfluxStateSnapshot,
    provider: Arc<dyn RemoteStateProvider>,
) -> StateDbResult<State> {
    let storage = RpcBackedStorage::new(snapshot, provider);
    let db = new_state_db(Box::new(storage));

    State::new(db)
}

pub fn build_virtual_call_env(input: VirtualCallEnvInput) -> Env {
    let chain_id = BTreeMap::from([
        (Space::Native, input.native_chain_id),
        (Space::Ethereum, input.ethereum_chain_id),
    ]);

    Env {
        chain_id,
        number: input.block_number,
        author: input.author,
        timestamp: input.timestamp,
        difficulty: U256::zero(),
        gas_limit: input.gas_limit,
        last_hash: input.last_hash,
        accumulated_gas_used: U256::zero(),
        epoch_height: input.epoch_height,
        pos_view: input.pos_view,
        finalized_epoch: input.finalized_epoch,
        transaction_epoch_bound: input.transaction_epoch_bound,
        base_gas_price: input.base_gas_price,
        burnt_gas_price: input.burnt_gas_price,
        transaction_hash: input.transaction_hash,
        ..Default::default()
    }
}

pub fn build_virtual_call_machine(params: CommonParams) -> Machine {
    Machine::new_with_builtin(params, VmFactory::default())
}

pub fn build_virtual_call_spec(machine: &Machine, env: &Env) -> Spec {
    machine.spec(env.number, env.epoch_height)
}

pub fn probe_virtual_call_context(
    state: &mut State,
    machine: &Machine,
    env_input: VirtualCallEnvInput,
) {
    let env = build_virtual_call_env(env_input);
    let spec = build_virtual_call_spec(machine, &env);

    let _context = EstimationContext::new(state, &env, machine, &spec);
}
