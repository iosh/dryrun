use cfx_parameters::internal_contract_addresses::SPONSOR_WHITELIST_CONTROL_CONTRACT_ADDRESS;
use cfx_types::{Address, U256};

use crate::state::provider::RemoteStateProviderError;

const ADDRESS_BYTES: usize = 20;
const SPONSOR_WHITELIST_KEY_BYTES: usize = ADDRESS_BYTES * 2;
const ABI_WORD_BYTES: usize = 32;
const IS_ALL_WHITELISTED_SELECTOR: [u8; 4] = [0x79, 0xb4, 0x7f, 0xaa];
const IS_WHITELISTED_SELECTOR: [u8; 4] = [0xb6, 0xb3, 0x52, 0x72];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeInternalStateItem {
    SponsorWhitelist(SponsorWhitelistStorageKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SponsorWhitelistStorageKey {
    pub(crate) contract: Address,
    pub(crate) user: Address,
}

impl SponsorWhitelistStorageKey {
    pub(crate) fn control_contract_address(self) -> Address {
        SPONSOR_WHITELIST_CONTROL_CONTRACT_ADDRESS
    }

    pub(crate) fn is_all_whitelist_key(self) -> bool {
        self.user.is_zero()
    }

    pub(crate) fn is_all_whitelisted_call_data(self) -> Vec<u8> {
        let mut data = Vec::with_capacity(4 + ABI_WORD_BYTES);
        data.extend_from_slice(&IS_ALL_WHITELISTED_SELECTOR);
        append_abi_address(&mut data, self.contract);
        data
    }

    pub(crate) fn is_user_whitelisted_call_data(self) -> Vec<u8> {
        let mut data = Vec::with_capacity(4 + ABI_WORD_BYTES * 2);
        data.extend_from_slice(&IS_WHITELISTED_SELECTOR);
        append_abi_address(&mut data, self.contract);
        append_abi_address(&mut data, self.user);
        data
    }
}

pub(crate) fn decode_abi_bool(
    value: Vec<u8>,
    field: &'static str,
) -> Result<bool, RemoteStateProviderError> {
    if value.len() != ABI_WORD_BYTES {
        return Err(RemoteStateProviderError::RpcDecode {
            field,
            message: format!("expected 32-byte ABI bool, got {} bytes", value.len()),
        });
    }

    let decoded = U256::from_big_endian(&value);
    match decoded {
        value if value.is_zero() => Ok(false),
        value if value == U256::one() => Ok(true),
        _ => Err(RemoteStateProviderError::RpcDecode {
            field,
            message: "expected ABI bool value 0 or 1".to_owned(),
        }),
    }
}

pub(crate) fn parse_native_internal_storage(
    address: Address,
    storage_key: &[u8],
) -> Option<NativeInternalStateItem> {
    // cfx_getStorageAt only accepts 32-byte positions, while this whitelist
    // key is contract + user. Handle it through the internal contract API.
    if address == SPONSOR_WHITELIST_CONTROL_CONTRACT_ADDRESS
        && storage_key.len() == SPONSOR_WHITELIST_KEY_BYTES
    {
        let (contract, user) = storage_key.split_at(ADDRESS_BYTES);
        return Some(NativeInternalStateItem::SponsorWhitelist(
            SponsorWhitelistStorageKey {
                contract: Address::from_slice(contract),
                user: Address::from_slice(user),
            },
        ));
    }

    None
}

fn append_abi_address(data: &mut Vec<u8>, address: Address) {
    data.extend_from_slice(&[0; ABI_WORD_BYTES - ADDRESS_BYTES]);
    data.extend_from_slice(address.as_bytes());
}
