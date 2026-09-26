mod collection;

use super::{ChangeOccurrence, EspaceNativeCurrency};
use crate::espace::{EspaceAnalysisError, EspaceChange, EspaceExecutedTransaction};
use alloy_primitives::{Address, U256};

pub(super) fn derive_changes(
    execution: &EspaceExecutedTransaction,
    currency: &EspaceNativeCurrency,
) -> Result<Vec<ChangeOccurrence>, EspaceAnalysisError> {
    let operations = collection::collect_native_operations(execution)?;
    Ok(operations
        .into_iter()
        .map(|operation| {
            let (position, change) = match operation {
                NativeOperation::AccountTransfer {
                    position,
                    from,
                    to,
                    amount,
                } => (
                    position,
                    EspaceChange::NativeTransfer {
                        from,
                        to,
                        raw_amount: amount,
                        currency: currency.clone(),
                    },
                ),
                NativeOperation::SelfDestructBurn {
                    position,
                    contract,
                    amount,
                } => (
                    position,
                    EspaceChange::SelfDestructBurn {
                        contract_address: contract,
                        raw_amount: amount,
                        currency: currency.clone(),
                    },
                ),
            };
            ChangeOccurrence::new(position, change)
        })
        .collect())
}

#[derive(Debug)]
enum NativeOperation {
    AccountTransfer {
        position: usize,
        from: Address,
        to: Address,
        amount: U256,
    },
    SelfDestructBurn {
        position: usize,
        contract: Address,
        amount: U256,
    },
}

impl NativeOperation {
    fn position(&self) -> usize {
        match self {
            Self::AccountTransfer { position, .. } | Self::SelfDestructBurn { position, .. } => {
                *position
            }
        }
    }
}
