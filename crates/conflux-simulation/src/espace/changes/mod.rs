mod native;
mod standards;
mod wrapped_native;

use alloy_primitives::{Address, U256};
use contract_standards::{Erc20Metadata, MetadataCall, StandardChange, metadata_calls};

use crate::{
    execution::{CommittedExecutionTrace, LogCheckpoint},
    primitive::b256_to_cfx,
};

use self::{
    standards::{DecodedStandardOccurrence, decode_standard_occurrences_in_scope},
    wrapped_native::{WrappedNativeOccurrence, decode_wrapped_native_occurrences_in_scope},
};
use super::{EspaceChangesError, EspaceExecutedTransaction, EspaceStateAccess};

pub(crate) use standards::{
    IsolatedReadCallError, MetadataReadError, ReadCallOutcome, execute_isolated_read_call,
    execute_read_call,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceNativeCurrency {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspaceChange {
    NativeTransfer {
        from: Address,
        to: Address,
        raw_amount: U256,
        currency: EspaceNativeCurrency,
    },
    SelfDestructBurn {
        contract_address: Address,
        raw_amount: U256,
        currency: EspaceNativeCurrency,
    },
    WrappedNativeDeposit {
        contract_address: Address,
        account: Address,
        raw_amount: U256,
        metadata: Erc20Metadata,
    },
    WrappedNativeWithdrawal {
        contract_address: Address,
        account: Address,
        raw_amount: U256,
        metadata: Erc20Metadata,
    },
    Standard(StandardChange<Address>),
}

#[derive(Debug)]
pub(crate) struct ChangeOccurrence {
    position: usize,
    change: EspaceChange,
}

impl ChangeOccurrence {
    pub(crate) const fn new(position: usize, change: EspaceChange) -> Self {
        Self { position, change }
    }

    pub(crate) fn into_parts(self) -> (usize, EspaceChange) {
        (self.position, self.change)
    }
}

pub(crate) struct NestedEspaceEffects {
    standard_occurrences: Vec<DecodedStandardOccurrence>,
    wrapped_native_occurrences: Vec<WrappedNativeOccurrence>,
}

impl NestedEspaceEffects {
    pub(crate) fn from_trace(
        trace: &CommittedExecutionTrace,
        root_frame_ids: &[crate::execution::FrameId],
        wrapped_native_token: Address,
    ) -> Self {
        let includes_frame = |frame_id| {
            root_frame_ids
                .iter()
                .any(|root_id| trace.frame_is_within(frame_id, *root_id))
        };
        Self {
            standard_occurrences: decode_standard_occurrences_in_scope(trace, includes_frame),
            wrapped_native_occurrences: decode_wrapped_native_occurrences_in_scope(
                trace,
                wrapped_native_token,
                includes_frame,
            ),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.standard_occurrences.is_empty() && self.wrapped_native_occurrences.is_empty()
    }

    pub(crate) fn metadata_call_occurrences(&self) -> Vec<(usize, MetadataCall<Address>)> {
        collect_metadata_call_occurrences(
            &self.standard_occurrences,
            &self.wrapped_native_occurrences,
        )
    }

    pub(crate) fn into_changes(
        self,
        metadata: &contract_standards::MetadataValues<Address>,
    ) -> Vec<ChangeOccurrence> {
        let mut changes = Vec::new();
        for occurrence in self.wrapped_native_occurrences {
            let change_metadata = metadata
                .erc20_metadata(&occurrence.contract_address())
                .unwrap_or_else(|_| {
                    unreachable!("nested eSpace metadata collection records every outcome")
                });
            changes.push(occurrence.into_change(change_metadata));
        }
        for occurrence in self.standard_occurrences {
            let change = occurrence
                .decoded_log
                .into_change(metadata)
                .unwrap_or_else(|_| {
                    unreachable!("nested eSpace metadata collection records every outcome")
                });
            changes.push(ChangeOccurrence::new(
                occurrence.position,
                EspaceChange::Standard(change),
            ));
        }
        changes
    }
}

pub(crate) fn log_checkpoints(wrapped_native_token: Address) -> Vec<LogCheckpoint> {
    contract_standards::supported_event_topics()
        .iter()
        .map(|topic0| LogCheckpoint {
            space: cfx_types::Space::Ethereum,
            address: None,
            topic0: b256_to_cfx(*topic0),
        })
        .chain(wrapped_native::log_checkpoints(wrapped_native_token))
        .collect()
}

pub(crate) fn derive_changes(
    execution: &EspaceExecutedTransaction,
    state: &EspaceStateAccess,
    wrapped_native_token: Address,
    currency: &EspaceNativeCurrency,
) -> Result<Vec<EspaceChange>, EspaceChangesError> {
    let successful = execution.is_success();
    if !successful && !execution.committed_logs().is_empty() {
        return Err(EspaceChangesError::inconsistent_execution(
            "failed execution returned committed receipt logs",
        ));
    }
    let mut changes = native::derive_changes(execution, state, currency)?;

    if !successful {
        return Ok(Vec::new());
    }
    changes.extend(standards::derive_changes(
        execution,
        state,
        wrapped_native_token,
    )?);

    changes.sort_by_key(|occurrence| occurrence.position);
    Ok(changes
        .into_iter()
        .map(|occurrence| occurrence.change)
        .collect())
}

fn collect_metadata_call_occurrences(
    standard_occurrences: &[DecodedStandardOccurrence],
    wrapped_native_occurrences: &[WrappedNativeOccurrence],
) -> Vec<(usize, MetadataCall<Address>)> {
    let mut calls = Vec::new();

    for occurrence in standard_occurrences {
        calls.extend(
            metadata_calls(std::iter::once(&occurrence.decoded_log))
                .into_iter()
                .map(|call| (occurrence.position, call)),
        );
    }
    for occurrence in wrapped_native_occurrences {
        let position = occurrence.position();
        let contract_address = occurrence.contract_address();
        calls.extend([
            (position, MetadataCall::Name { contract_address }),
            (position, MetadataCall::Symbol { contract_address }),
            (position, MetadataCall::Decimals { contract_address }),
        ]);
    }

    calls.sort_by_key(|(position, _)| *position);
    calls
}
