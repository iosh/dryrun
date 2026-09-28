use std::cell::Cell;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{CodedError, ErrorCode};

/// Per-request resource limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Limits {
    /// Remote state items fetched by one request, execution included.
    pub max_state_reads: usize,
    /// Read-only contract calls made while deriving changes.
    pub max_read_calls: usize,
    /// Gas limit of each read-only call.
    pub read_call_gas: u64,
    /// Largest accepted output of a read-only call.
    pub max_read_call_output: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_state_reads: 10_000,
            max_read_calls: 256,
            read_call_gas: 5_000_000,
            max_read_call_output: 64 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Resource {
    StateReads,
    ReadCalls,
    ReadCallOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("{resource:?} limit of {limit} exceeded")]
pub struct LimitExceeded {
    pub resource: Resource,
    pub limit: usize,
}

impl CodedError for LimitExceeded {
    fn code(&self) -> ErrorCode {
        ErrorCode::LimitExceeded
    }
}

/// Counts resource use of one request against its [`Limits`].
#[derive(Debug)]
pub struct ReadBudget {
    limits: Limits,
    state_reads: Cell<usize>,
    read_calls: Cell<usize>,
}

impl ReadBudget {
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            state_reads: Cell::new(0),
            read_calls: Cell::new(0),
        }
    }

    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    pub fn record_state_read(&self) -> Result<(), LimitExceeded> {
        consume(
            &self.state_reads,
            self.limits.max_state_reads,
            Resource::StateReads,
        )
    }

    pub fn record_read_call(&self) -> Result<(), LimitExceeded> {
        consume(
            &self.read_calls,
            self.limits.max_read_calls,
            Resource::ReadCalls,
        )
    }

    pub fn check_read_call_output(&self, len: usize) -> Result<(), LimitExceeded> {
        let limit = self.limits.max_read_call_output;
        if len > limit {
            return Err(LimitExceeded {
                resource: Resource::ReadCallOutput,
                limit,
            });
        }
        Ok(())
    }
}

fn consume(used: &Cell<usize>, limit: usize, resource: Resource) -> Result<(), LimitExceeded> {
    if used.get() >= limit {
        return Err(LimitExceeded { resource, limit });
    }
    used.set(used.get() + 1);
    Ok(())
}
