use alloy::{
    eips::eip2930::AccessListItem,
    primitives::{Address, B256, U256, U512},
};
use cfx_types::{Address as CfxAddress, H256 as CfxH256, U256 as CfxU256, U512 as CfxU512};
use primitives::AccessListItem as CfxAccessListItem;

pub(crate) fn address_to_cfx(address: Address) -> CfxAddress {
    CfxAddress::from_slice(address.as_slice())
}

pub(crate) fn address_from_cfx(address: CfxAddress) -> Address {
    Address::from_slice(address.as_bytes())
}

pub(crate) fn b256_to_cfx(value: B256) -> CfxH256 {
    CfxH256::from_slice(value.as_slice())
}

pub(crate) fn u256_to_cfx(value: U256) -> CfxU256 {
    CfxU256::from_big_endian(&value.to_be_bytes::<32>())
}

pub(crate) fn b256_from_cfx(value: CfxH256) -> B256 {
    B256::from_slice(value.as_bytes())
}

pub(crate) fn u256_from_cfx(value: CfxU256) -> U256 {
    let mut bytes = [0_u8; 32];
    value.to_big_endian(&mut bytes);
    U256::from_be_bytes(bytes)
}

pub(crate) fn u512_from_cfx(value: CfxU512) -> U512 {
    let mut bytes = [0_u8; 64];
    value.to_big_endian(&mut bytes);
    U512::from_be_bytes(bytes)
}

pub(crate) fn alloy_u256_from_u64(value: u64) -> U256 {
    U256::from_limbs([value, 0, 0, 0])
}

pub(crate) fn access_list_to_cfx(items: Vec<AccessListItem>) -> Vec<CfxAccessListItem> {
    items
        .into_iter()
        .map(|item| CfxAccessListItem {
            address: address_to_cfx(item.address),
            storage_keys: item.storage_keys.into_iter().map(b256_to_cfx).collect(),
        })
        .collect()
}
