use alloy_primitives::{Address, B256, keccak256};

use super::{AnalysisError, view::TokenView};

/// An implementation review establishes that the shared standard algorithms
/// account for all effects of the supported calls, including unlogged writes.
pub trait ReviewedStandardImplementation: Send + Sync + 'static {
    fn code_hash(&self) -> B256;

    fn matches(&self, view: &dyn TokenView, contract: Address) -> Result<bool, AnalysisError> {
        Ok(keccak256(view.initial().code(contract)?) == self.code_hash())
    }

    /// Called after matching, before standard event and state verification.
    fn verify_support(&self, view: &dyn TokenView, contract: Address) -> Result<(), AnalysisError>;
}

pub(super) fn address_mapping_slot(address: Address, slot: B256) -> B256 {
    let mut input = [0; 64];
    input[..32].copy_from_slice(address.into_word().as_slice());
    input[32..].copy_from_slice(slot.as_slice());
    keccak256(input)
}
