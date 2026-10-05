use alloy::primitives::Address as HexAddress;
use cfx_types::{AddressWithSpace, Space};
use conflux_provider::{CoreAddress, Network};
use serde::{Serialize, Serializer};
use simulation_core::ChainAddress;

use crate::{address::VmAddress, primitive::*};

/// An account identity in a Core transaction, including nested eSpace execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Address {
    address: AddressWithSpace,
    network: Network,
}

impl Address {
    /// Creates an identity with an explicit execution space and network.
    pub fn new(address: HexAddress, space: Space, network: Network) -> Self {
        Self::from_vm(
            AddressWithSpace {
                address: address_to_cfx(address),
                space,
            },
            network,
        )
    }

    /// The 20-byte address, without its space or network.
    pub fn raw(&self) -> HexAddress {
        address_from_cfx(self.address.address)
    }

    /// The execution space; `Space::Ethereum` denotes eSpace.
    pub const fn space(&self) -> Space {
        self.address.space
    }

    /// The Conflux network this identity belongs to.
    pub const fn network(&self) -> Network {
        self.network
    }
}

impl ChainAddress for Address {
    fn raw(&self) -> HexAddress {
        Self::raw(self)
    }

    fn with_raw(&self, raw: HexAddress) -> Self {
        Self::new(raw, self.space(), self.network())
    }
}

impl VmAddress for Address {
    fn accepts(_: Space) -> bool {
        true
    }

    fn from_vm(address: AddressWithSpace, network: Network) -> Self {
        Self { address, network }
    }

    fn to_vm(self) -> AddressWithSpace {
        self.address
    }
}

impl Serialize for Address {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.address.space {
            Space::Ethereum => self.raw().serialize(serializer),
            Space::Native => CoreAddress::from_bytes(self.address.address.0, self.network)
                .map_err(serde::ser::Error::custom)?
                .serialize(serializer),
        }
    }
}
