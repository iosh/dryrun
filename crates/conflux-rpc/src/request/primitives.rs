use alloy_primitives::{Address as AlloyAddress, B256, U256 as AlloyU256};
use cfx_types::{Address, H256, U256};

pub(super) fn to_alloy_address(value: Address) -> AlloyAddress {
    AlloyAddress::from_slice(value.as_bytes())
}

pub(super) fn to_alloy_b256(value: H256) -> B256 {
    B256::from_slice(value.as_bytes())
}

pub(super) fn to_alloy_u256(value: U256) -> AlloyU256 {
    let mut bytes = [0_u8; 32];
    value.to_big_endian(&mut bytes);
    AlloyU256::from_be_bytes(bytes)
}
