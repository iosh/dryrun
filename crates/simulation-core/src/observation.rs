use alloy_primitives::{Address, B256};
use std::cell::Cell;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum AnalysisResource {
    Facts,
    StateReads,
    ReadCalls,
    CallOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("analysis {resource:?} limit {limit} exceeded")]
pub struct AnalysisLimitExceeded {
    pub resource: AnalysisResource,
    pub limit: usize,
}

#[derive(Debug)]
pub struct ReadBudget {
    limits: AnalysisLimits,
    state_reads: Cell<usize>,
    read_calls: Cell<usize>,
}

impl ReadBudget {
    pub fn new(limits: AnalysisLimits) -> Self {
        Self {
            limits,
            state_reads: Cell::new(0),
            read_calls: Cell::new(0),
        }
    }
    pub const fn limits(&self) -> &AnalysisLimits {
        &self.limits
    }
    pub fn state_read(&self) -> Result<(), AnalysisLimitExceeded> {
        Self::consume(
            &self.state_reads,
            self.limits.max_state_reads,
            AnalysisResource::StateReads,
        )
    }
    pub fn read_call(&self) -> Result<(), AnalysisLimitExceeded> {
        Self::consume(
            &self.read_calls,
            self.limits.max_read_calls,
            AnalysisResource::ReadCalls,
        )
    }
    pub fn check_output(&self, len: usize) -> Result<(), AnalysisLimitExceeded> {
        if len > self.limits.max_read_call_output_bytes {
            Err(AnalysisLimitExceeded {
                resource: AnalysisResource::CallOutput,
                limit: self.limits.max_read_call_output_bytes,
            })
        } else {
            Ok(())
        }
    }
    fn consume(
        counter: &Cell<usize>,
        limit: usize,
        resource: AnalysisResource,
    ) -> Result<(), AnalysisLimitExceeded> {
        let used = counter.get();
        if used >= limit {
            return Err(AnalysisLimitExceeded { resource, limit });
        }
        counter.set(used + 1);
        Ok(())
    }
}

/// Observation and read limits shared by all rules in one simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct AnalysisLimits {
    pub max_observed_facts: usize,
    pub max_state_reads: usize,
    pub max_read_calls: usize,
    pub read_call_gas_limit: u64,
    pub max_read_call_output_bytes: usize,
}

impl Default for AnalysisLimits {
    fn default() -> Self {
        Self {
            max_observed_facts: 100_000,
            max_state_reads: 1_024,
            max_read_calls: 64,
            read_call_gas_limit: 5_000_000,
            max_read_call_output_bytes: 256 * 1_024,
        }
    }
}

/// Logs matching this filter retain their state for later analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LogFilter<A = Address> {
    pub address: Option<A>,
    pub topic0: B256,
}

impl<A: Copy + Eq> LogFilter<A> {
    pub fn matches(&self, address: A, topic0: B256) -> bool {
        self.topic0 == topic0 && self.address.is_none_or(|expected| expected == address)
    }
}
