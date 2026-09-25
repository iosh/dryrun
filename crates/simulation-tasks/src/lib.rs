use std::{future::Future, num::NonZeroUsize, sync::Arc, time::Duration};

use thiserror::Error;
use tokio::{sync::Semaphore, task::JoinError};
use tokio_util::task::TaskTracker;
use tracing::error;

/// A bounded set of owned simulation attempts.
#[derive(Debug, Clone)]
pub struct SimulationTaskSet {
    permits: Arc<Semaphore>,
    tasks: TaskTracker,
    response_timeout: Duration,
}

/// A failure produced by the task set rather than by a simulation attempt.
#[derive(Debug, Error)]
pub enum SimulationTaskError {
    #[error("simulation task set is closed")]
    Closed,

    #[error("simulation response deadline exceeded")]
    ResponseTimedOut,

    #[error("simulation task was cancelled")]
    TaskCancelled {
        #[source]
        source: JoinError,
    },

    #[error("simulation task panicked")]
    TaskPanicked {
        #[source]
        source: JoinError,
    },
}

impl SimulationTaskSet {
    /// Creates a task set with a fixed concurrency limit and response deadline.
    pub fn new(max_concurrent: NonZeroUsize, response_timeout: Duration) -> Self {
        Self {
            permits: Arc::new(Semaphore::new(max_concurrent.get())),
            tasks: TaskTracker::new(),
            response_timeout,
        }
    }

    /// Stops admitting attempts and wakes callers waiting for capacity.
    pub fn close(&self) {
        // Close admission before allowing drain to observe an empty tracker.
        self.permits.close();
        self.tasks.close();
    }

    /// Waits for all attempts admitted before closing to finish.
    pub async fn drain(&self) {
        self.tasks.wait().await;
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
        let permits = self.permits.clone();
        // Register before admission so close cannot miss a concurrent attempt.
        let task_token = self.tasks.token();

        tokio::time::timeout(self.response_timeout, async move {
            let permit = permits
                .acquire_owned()
                .await
                .map_err(|_| SimulationTaskError::Closed)?;
            let task = tokio::spawn(async move {
                let _task_token = task_token;
                let _permit = permit;
                match tokio::spawn(async move { start_attempt().await }).await {
                    Ok(output) => Ok(output),
                    Err(source) => Err(classify_join_error(source)),
                }
            });

            task.await.map_err(classify_join_error)
        })
        .await
        .map_err(|_| SimulationTaskError::ResponseTimedOut)??
    }
}

fn classify_join_error(source: JoinError) -> SimulationTaskError {
    if source.is_panic() {
        error!(error = ?source, "admitted simulation task panicked");
        SimulationTaskError::TaskPanicked { source }
    } else {
        error!(error = ?source, "admitted simulation task was cancelled");
        SimulationTaskError::TaskCancelled { source }
    }
}

impl simulation_core::error::ErrorInfo for SimulationTaskError {
    fn diagnostic(&self) -> simulation_core::error::Diagnostic {
        use simulation_core::error::ErrorCode;
        match self {
            Self::Closed => ErrorCode::ServiceClosed,
            Self::ResponseTimedOut => ErrorCode::ServiceTimeout,
            Self::TaskCancelled { .. } => ErrorCode::ServiceCancelled,
            Self::TaskPanicked { .. } => ErrorCode::Internal,
        }
        .diagnostic()
    }
}
