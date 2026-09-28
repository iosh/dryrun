use std::{fmt::Debug, hash::Hash};

use alloy_primitives::Address;
use serde::Serialize;

/// An address as reported in the results of one execution space.
///
/// The VM always works on 20-byte addresses; a chain may attach more identity
/// to them (Conflux attaches the space and network).
pub trait ChainAddress: Copy + Ord + Hash + Debug + Serialize {
    /// The 20-byte address the VM operates on.
    fn raw(&self) -> Address;

    /// Another address in the same space as `self`.
    fn with_raw(&self, raw: Address) -> Self;
}

impl ChainAddress for Address {
    fn raw(&self) -> Address {
        *self
    }

    fn with_raw(&self, raw: Address) -> Self {
        raw
    }
}
