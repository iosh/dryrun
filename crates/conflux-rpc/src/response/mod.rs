mod core_space;
mod espace;

use alloy_primitives::{B256, U256 as AlloyU256};
use cfx_types::{H256, U256};

pub(crate) use core_space::SimulateCoreSpaceTransactionResponse;
pub(crate) use espace::SimulateEspaceTransactionResponse;

fn u256_to_wire(value: AlloyU256) -> U256 {
    U256::from_big_endian(&value.to_be_bytes::<32>())
}

fn b256_to_wire(value: B256) -> H256 {
    H256::from_slice(value.as_slice())
}
