mod native;
mod standards;
mod wrapped_native;

use std::collections::HashSet;

use alloy_primitives::{Address, U256};
use contract_standards::{Erc20Metadata, MetadataCall, StandardChange, metadata_calls};

use crate::execution::CommittedExecutionTrace;

use self::{
    native::{NativeAnalysis, NativeBalances},
    standards::{
        DecodedStandardOccurrence, decode_standard_occurrences,
        decode_standard_occurrences_in_scope, load_metadata,
    },
    wrapped_native::{
        WrappedNativeOccurrence, decode_wrapped_native_occurrences,
        decode_wrapped_native_occurrences_in_scope,
    },
};
use super::{EspaceChangesError, EspaceExecutedTransaction};

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
    ) -> Result<Vec<ChangeOccurrence>, contract_standards::MissingMetadataOutcome> {
        let mut changes = Vec::new();
        for occurrence in self.wrapped_native_occurrences {
            let change_metadata = metadata.erc20_metadata(&occurrence.contract_address())?;
            changes.push(occurrence.into_change(change_metadata));
        }
        for occurrence in self.standard_occurrences {
            let change = occurrence.decoded_log.into_change(metadata)?;
            changes.push(ChangeOccurrence::new(
                occurrence.position,
                EspaceChange::Standard(change),
            ));
        }
        Ok(changes)
    }
}

pub(crate) struct EspaceChangesAnalysis {
    successful: bool,
    currency: EspaceNativeCurrency,
    native: NativeAnalysis,
    standard_occurrences: Vec<DecodedStandardOccurrence>,
    wrapped_native_occurrences: Vec<WrappedNativeOccurrence>,
}

impl EspaceChangesAnalysis {
    pub(crate) fn from_execution(
        execution: &EspaceExecutedTransaction,
        wrapped_native_token: Address,
        currency: &EspaceNativeCurrency,
    ) -> Result<Self, EspaceChangesError> {
        let successful = execution.is_success();
        if !successful && !execution.committed_logs().is_empty() {
            return Err(EspaceChangesError::inconsistent_execution(
                "failed execution returned committed receipt logs",
            ));
        }

        let standard_occurrences = if successful {
            decode_standard_occurrences(execution.committed_logs())
        } else {
            Vec::new()
        };
        let wrapped_native_occurrences = if successful {
            decode_wrapped_native_occurrences(execution.committed_logs(), wrapped_native_token)
        } else {
            Vec::new()
        };

        Ok(Self {
            successful,
            currency: currency.clone(),
            native: NativeAnalysis::from_execution(execution)
                .map_err(|error| EspaceChangesError::resolver("native currency", error))?,
            standard_occurrences,
            wrapped_native_occurrences,
        })
    }

    pub(crate) fn read_native_balances(
        &self,
        state: &crate::espace::EspaceStateReader,
        operation: &'static str,
    ) -> Result<NativeBalances, EspaceChangesError> {
        self.native.read_balances(state, operation)
    }

    pub(crate) fn finish(
        self,
        state: &crate::espace::EspaceStateReader,
        before_balances: &NativeBalances,
        after_balances: &NativeBalances,
    ) -> Result<Vec<EspaceChange>, EspaceChangesError> {
        let Self {
            successful,
            currency,
            native,
            standard_occurrences,
            wrapped_native_occurrences,
        } = self;
        let mut changes = native
            .verify(before_balances, after_balances, &currency)
            .map_err(|error| EspaceChangesError::resolver("native currency", error))?;

        if !successful {
            return Ok(Vec::new());
        }

        let calls = collect_metadata_calls(&standard_occurrences, &wrapped_native_occurrences);
        let metadata = load_metadata(state, calls)?;

        for occurrence in wrapped_native_occurrences {
            let change_metadata = metadata.erc20_metadata(&occurrence.contract_address())?;
            changes.push(occurrence.into_change(change_metadata));
        }
        for occurrence in standard_occurrences {
            let change = occurrence.decoded_log.into_change(&metadata)?;
            changes.push(ChangeOccurrence::new(
                occurrence.position,
                EspaceChange::Standard(change),
            ));
        }

        changes.sort_by_key(|occurrence| occurrence.position);
        Ok(changes
            .into_iter()
            .map(|occurrence| occurrence.change)
            .collect())
    }
}

fn collect_metadata_calls(
    standard_occurrences: &[DecodedStandardOccurrence],
    wrapped_native_occurrences: &[WrappedNativeOccurrence],
) -> Vec<MetadataCall<Address>> {
    let calls = collect_metadata_call_occurrences(standard_occurrences, wrapped_native_occurrences);

    let mut seen = HashSet::new();
    calls
        .into_iter()
        .filter_map(|(_, call)| seen.insert(call.clone()).then_some(call))
        .collect()
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
