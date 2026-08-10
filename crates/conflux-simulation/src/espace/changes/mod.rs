mod native;
mod standards;
mod wrapped_native;

use std::collections::HashSet;

use alloy_primitives::{Address, U256};
use cfx_executor::{machine::Machine, state::State};
use contract_standards::{Erc20Metadata, MetadataCall, StandardChange, metadata_calls};

use crate::execution::{ConfluxExecutionOutput, Observation, PreparedTransactionExecution};

use self::{
    native::{NativeAnalysis, NativeBalances},
    standards::{DecodedStandardOccurrence, decode_standard_occurrences, load_metadata},
    wrapped_native::{WrappedNativeOccurrence, decode_wrapped_native_occurrences},
};
use super::{EspaceChangesError, EspaceCompleteTransaction, EspaceNativeChangeError};

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
        output: &ConfluxExecutionOutput,
        successful: bool,
        wrapped_native_token: Address,
        currency: &EspaceNativeCurrency,
    ) -> Result<Self, EspaceChangesError> {
        verify_committed_logs(&output.observations, &output.logs)?;
        if !successful && !output.logs.is_empty() {
            return Err(EspaceChangesError::invalid_execution_evidence(
                "failed execution returned committed receipt logs",
            ));
        }

        let standard_occurrences = if successful {
            decode_standard_occurrences(&output.observations)
        } else {
            Vec::new()
        };
        let wrapped_native_occurrences = if successful {
            decode_wrapped_native_occurrences(&output.observations, wrapped_native_token)
        } else {
            Vec::new()
        };

        Ok(Self {
            successful,
            currency: currency.clone(),
            native: NativeAnalysis::from_execution(output)?,
            standard_occurrences,
            wrapped_native_occurrences,
        })
    }

    pub(crate) fn read_native_balances(
        &self,
        state: &State,
        operation: &'static str,
    ) -> Result<NativeBalances, EspaceChangesError> {
        self.native.read_balances(state, operation)
    }

    pub(crate) fn finish(
        self,
        state: &mut State,
        machine: &Machine,
        prepared_execution: &PreparedTransactionExecution,
        transaction: &EspaceCompleteTransaction,
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
        let mut changes = native.verify(before_balances, after_balances, successful, &currency)?;

        if !successful {
            if !changes.is_empty() {
                return Err(EspaceNativeChangeError::BusinessEffectOnFailedExecution.into());
            }
            return Ok(Vec::new());
        }

        let calls = collect_metadata_calls(&standard_occurrences, &wrapped_native_occurrences);
        let metadata = load_metadata(state, machine, prepared_execution, transaction, calls)?;

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
    let mut seen = HashSet::new();
    calls
        .into_iter()
        .filter_map(|(_, call)| seen.insert(call.clone()).then_some(call))
        .collect()
}

fn verify_committed_logs(
    observations: &[Observation],
    committed_logs: &[primitives::LogEntry],
) -> Result<(), EspaceChangesError> {
    let observed_logs = observations.iter().filter_map(|observation| {
        let Observation::Log {
            space,
            address,
            topics,
            data,
            ..
        } = observation
        else {
            return None;
        };
        Some((space, address, topics, data))
    });
    let observed_count = observed_logs.clone().count();
    if observed_count != committed_logs.len() {
        return Err(EspaceChangesError::invalid_execution_evidence(format!(
            "observer recorded {observed_count} committed logs, executor returned {}",
            committed_logs.len()
        )));
    }

    for (index, ((space, address, topics, data), committed)) in
        observed_logs.zip(committed_logs).enumerate()
    {
        if *space != committed.space
            || *address != committed.address
            || topics != &committed.topics
            || data.as_slice() != committed.data.as_slice()
        {
            return Err(EspaceChangesError::invalid_execution_evidence(format!(
                "observer log {index} does not match the committed executor log"
            )));
        }
    }

    Ok(())
}
