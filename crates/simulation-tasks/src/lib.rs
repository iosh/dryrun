use std::{
    future::Future,
    num::NonZeroUsize,
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use thiserror::Error;
use tokio::{sync::Semaphore, task::JoinError, time::timeout};
use tokio_util::task::TaskTracker;

/// A bounded set of owned simulation attempts.
#[derive(Debug, Clone)]
pub struct SimulationTaskSet {
    inner: Arc<Inner>,
}

/// A failure produced by the task set rather than by a simulation attempt.
#[derive(Debug, Error)]
pub enum SimulationTaskError {
    #[error("simulation task set is closed")]
    Closed,

    #[error("timed out waiting for simulation capacity")]
    AdmissionTimedOut,

    #[error("simulation task failed")]
    TaskFailed {
        #[source]
        source: JoinError,
    },
}

#[derive(Debug)]
struct Inner {
    admission: Mutex<AdmissionState>,
    permits: Arc<Semaphore>,
    tasks: TaskTracker,
    admission_timeout: Duration,
}

#[derive(Debug)]
struct AdmissionState {
    closed: bool,
}

impl SimulationTaskSet {
    /// Creates a task set with one admission timeout shared by all attempts.
    pub fn new(max_concurrent: NonZeroUsize, admission_timeout: Duration) -> Self {
        Self {
            inner: Arc::new(Inner {
                admission: Mutex::new(AdmissionState { closed: false }),
                permits: Arc::new(Semaphore::new(max_concurrent.get())),
                tasks: TaskTracker::new(),
                admission_timeout,
            }),
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
        let permit = match timeout(
            self.inner.admission_timeout,
            self.inner.permits.clone().acquire_owned(),
        )
        .await
        {
            Ok(Ok(permit)) => permit,
            Ok(Err(_)) => return Err(SimulationTaskError::Closed),
            Err(_) => return Err(SimulationTaskError::AdmissionTimedOut),
        };

        let task = {
            let admission = self.inner.lock_admission();
            if admission.closed {
                return Err(SimulationTaskError::Closed);
            }

            self.inner.tasks.spawn(async move {
                let _permit = permit;
                start_attempt().await
            })
        };

        task.await
            .map_err(|source| SimulationTaskError::TaskFailed { source })
    }

    /// Prevents new attempts from starting without stopping active attempts.
    pub fn close(&self) {
        let mut admission = self.inner.lock_admission();
        if admission.closed {
            return;
        }

        admission.closed = true;
        self.inner.permits.close();
        self.inner.tasks.close();
    }

    /// Waits until the task set is closed and every started attempt has exited.
    pub async fn wait(&self) {
        self.inner.tasks.wait().await;
    }
}

impl Inner {
    fn lock_admission(&self) -> MutexGuard<'_, AdmissionState> {
        match self.admission.lock() {
            Ok(admission) => admission,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[cfg(test)]
mod tests;
