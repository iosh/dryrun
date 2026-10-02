use cfx_parameters::internal_contract_addresses::SPONSOR_WHITELIST_CONTROL_CONTRACT_ADDRESS;
use cfx_types::Address;
const ADDRESS_BYTES: usize = 20;
const SPONSOR_WHITELIST_KEY_BYTES: usize = ADDRESS_BYTES * 2;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CoreSpaceInternalStateItem {
    SponsorWhitelist(SponsorWhitelistStorageKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SponsorWhitelistStorageKey {
    pub(crate) contract_address: Address,
    pub(crate) account_address: Address,
}

pub(crate) fn parse_core_space_internal_storage(
    address: Address,
    storage_key: &[u8],
) -> Option<CoreSpaceInternalStateItem> {
    // cfx_getStorageAt only accepts 32-byte positions, while this whitelist
    // key is contract + user. Handle it through the internal contract API.
    if address == SPONSOR_WHITELIST_CONTROL_CONTRACT_ADDRESS
        && storage_key.len() == SPONSOR_WHITELIST_KEY_BYTES
    {
        let (contract_address, account_address) = storage_key.split_at(ADDRESS_BYTES);
        return Some(CoreSpaceInternalStateItem::SponsorWhitelist(
            SponsorWhitelistStorageKey {
                contract_address: Address::from_slice(contract_address),
                account_address: Address::from_slice(account_address),
            },
        ));
    }

    None
}
