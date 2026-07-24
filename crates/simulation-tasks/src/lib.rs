use std::{future::Future, num::NonZeroUsize, sync::Arc};

use thiserror::Error;
use tokio::{sync::Semaphore, task::JoinError};

/// A bounded set of owned simulation attempts.
#[derive(Debug, Clone)]
pub struct SimulationTaskSet {
    permits: Arc<Semaphore>,
}

/// A failure produced by the task set rather than by a simulation attempt.
#[derive(Debug, Error)]
pub enum SimulationTaskError {
    #[error("simulation task set is closed")]
    Closed,

    #[error("simulation task failed")]
    TaskFailed {
        #[source]
        source: JoinError,
    },
}

impl SimulationTaskSet {
    /// Creates a task set with a fixed concurrency limit.
    pub fn new(max_concurrent: NonZeroUsize) -> Self {
        Self {
            permits: Arc::new(Semaphore::new(max_concurrent.get())),
        }
    }

    /// Waits for capacity, then starts and awaits an owned attempt.
    ///
    /// Dropping the caller after the attempt starts does not stop the attempt or
    /// release its capacity early.
    pub async fn run<Start, Attempt, Output>(
        &self,
        start_attempt: Start,
    ) -> Result<Output, SimulationTaskError>
    where
        Start: FnOnce() -> Attempt + Send + 'static,
        Attempt: Future<Output = Output> + Send + 'static,
        Output: Send + 'static,
    {
        let permit = self
            .permits
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| SimulationTaskError::Closed)?;
        let task = tokio::spawn(async move {
            let _permit = permit;
            start_attempt().await
        });

        task.await
            .map_err(|source| SimulationTaskError::TaskFailed { source })
    }
}

#[cfg(test)]
mod tests;
