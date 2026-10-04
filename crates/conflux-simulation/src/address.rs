use alloy::primitives::Address;
use cfx_types::{AddressSpaceUtil, AddressWithSpace, Space};
use conflux_provider::Network;
use simulation_core::ChainAddress;

/// Converts identities at the Conflux VM boundary without losing their space.
pub(crate) trait VmAddress: ChainAddress {
    fn accepts(space: Space) -> bool;
    fn from_vm(address: AddressWithSpace, network: Network) -> Self;
    fn to_vm(self) -> AddressWithSpace;
}

impl VmAddress for Address {
    fn accepts(space: Space) -> bool {
        space == Space::Ethereum
    }

    fn from_vm(address: AddressWithSpace, _: Network) -> Self {
        crate::primitive::address_from_cfx(address.address)
    }

    fn to_vm(self) -> AddressWithSpace {
        crate::primitive::address_to_cfx(self).with_evm_space()
    }
}
