mod native;
mod standards;
mod wrapped_native;

use std::collections::HashSet;

use alloy::primitives::{Address, Log, U256};
use contract_standards::{Erc20Metadata, MetadataCall, StandardChange, metadata_calls};

use crate::{
    EthereumChainSpec, EvmChangesError, EvmExecutionEvent, EvmSimulationError,
    execution::EvmTransactionExecution, state::EvmStateView,
};

use self::{
    native::analyze_native_changes,
    standards::{DecodedStandardOccurrence, decode_standard_occurrences, load_metadata},
    wrapped_native::{WrappedNativeOccurrence, decode_wrapped_native_occurrences},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCurrency {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvmChange {
    NativeTransfer {
        from: Address,
        to: Address,
        raw_amount: U256,
        currency: NativeCurrency,
    },
    SelfDestructBurn {
        contract_address: Address,
        raw_amount: U256,
        currency: NativeCurrency,
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
struct ChangeOccurrence {
    event_index: usize,
    change: EvmChange,
}

impl ChangeOccurrence {
    const fn new(event_index: usize, change: EvmChange) -> Self {
        Self {
            event_index,
            change,
        }
    }
}

pub(crate) fn analyze_changes(
    output: &EvmTransactionExecution,
    after: &EvmStateView,
    chain_spec: &EthereumChainSpec,
) -> Result<Vec<EvmChange>, EvmSimulationError> {
    let events = output.events().to_vec();
    verify_committed_logs(&events, output.committed_logs())?;

    let currency = chain_spec.native_currency();
    let mut changes = analyze_native_changes(
        output.transition(),
        &events,
        output.caller(),
        output.beneficiary(),
        output.fee_settlement(),
        currency,
    )
    .map_err(EvmChangesError::from)?;

    let wrapped_native_occurrences = chain_spec
        .wrapped_native_token_address()
        .map(|address| decode_wrapped_native_occurrences(&events, address))
        .unwrap_or_default();
    let standard_occurrences = decode_standard_occurrences(&events);
    let calls = collect_metadata_calls(&standard_occurrences, &wrapped_native_occurrences);

    let metadata = load_metadata(after, calls)?;
    for occurrence in wrapped_native_occurrences {
        let change_metadata = metadata
            .erc20_metadata(&occurrence.contract_address())
            .map_err(EvmChangesError::from)?;
        changes.push(occurrence.into_change(change_metadata));
    }
    for occurrence in standard_occurrences {
        let change = occurrence
            .decoded_log
            .into_change(&metadata)
            .map_err(EvmChangesError::from)?;
        changes.push(ChangeOccurrence::new(
            occurrence.event_index,
            EvmChange::Standard(change),
        ));
    }

    changes.sort_by_key(|occurrence| occurrence.event_index);
    Ok(changes
        .into_iter()
        .map(|occurrence| occurrence.change)
        .collect())
}

fn collect_metadata_calls(
    standard_occurrences: &[DecodedStandardOccurrence],
    wrapped_native_occurrences: &[WrappedNativeOccurrence],
) -> Vec<MetadataCall<Address>> {
    let mut calls = Vec::new();

    for occurrence in standard_occurrences {
        calls.extend(
            metadata_calls(std::iter::once(&occurrence.decoded_log))
                .into_iter()
                .map(|call| (occurrence.event_index, call)),
        );
    }
    for occurrence in wrapped_native_occurrences {
        let event_index = occurrence.event_index();
        let contract_address = occurrence.contract_address();
        calls.extend([
            (event_index, MetadataCall::Name { contract_address }),
            (event_index, MetadataCall::Symbol { contract_address }),
            (event_index, MetadataCall::Decimals { contract_address }),
        ]);
    }

    calls.sort_by_key(|(event_index, _)| *event_index);
    let mut seen = HashSet::new();
    calls
        .into_iter()
        .filter_map(|(_, call)| seen.insert(call.clone()).then_some(call))
        .collect()
}

fn verify_committed_logs(
    events: &[EvmExecutionEvent],
    committed_logs: &[Log],
) -> Result<(), EvmChangesError> {
    let observed_logs = events.iter().filter_map(|event| {
        let EvmExecutionEvent::Log {
            address,
            topics,
            data,
        } = event
        else {
            return None;
        };

        Some((address, topics, data))
    });
    let observed_count = observed_logs.clone().count();

    if observed_count != committed_logs.len() {
        return Err(EvmChangesError::CommittedLogCountMismatch {
            observed_count,
            result_count: committed_logs.len(),
        });
    }

    for (index, ((address, topics, data), committed)) in
        observed_logs.zip(committed_logs).enumerate()
    {
        if *address != committed.address
            || topics.as_slice() != committed.data.topics()
            || data != &committed.data.data
        {
            return Err(EvmChangesError::CommittedLogMismatch { index });
        }
    }

    Ok(())
}
