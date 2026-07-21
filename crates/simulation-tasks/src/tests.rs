use std::{error::Error, num::NonZeroUsize, time::Duration};

use tokio::sync::oneshot;

use super::{SimulationTaskError, SimulationTaskSet};

const NO_ADMISSION_WAIT: Duration = Duration::ZERO;

#[tokio::test]
async fn queued_attempt_times_out_when_capacity_is_full() -> Result<(), Box<dyn Error>> {
    let tasks = SimulationTaskSet::new(NonZeroUsize::MIN, NO_ADMISSION_WAIT);
    let (started_tx, started_rx) = oneshot::channel();
    let (finish_tx, finish_rx) = oneshot::channel();

    let active_tasks = tasks.clone();
    let active = tokio::spawn(async move {
        active_tasks
            .run(move || async move {
                assert!(started_tx.send(()).is_ok());
                assert!(finish_rx.await.is_ok());
                7
            })
            .await
    });
    started_rx.await?;

    assert!(matches!(
        tasks.run(|| async { 9 }).await,
        Err(SimulationTaskError::AdmissionTimedOut)
    ));

    assert!(finish_tx.send(()).is_ok());
    assert_eq!(active.await??, 7);
    Ok(())
}

#[tokio::test]
async fn started_attempt_outlives_caller_and_delays_shutdown() -> Result<(), Box<dyn Error>> {
    let tasks = SimulationTaskSet::new(NonZeroUsize::MIN, NO_ADMISSION_WAIT);
    let (started_tx, started_rx) = oneshot::channel();
    let (finish_tx, finish_rx) = oneshot::channel();

    let caller_tasks = tasks.clone();
    let caller = tokio::spawn(async move {
        caller_tasks
            .run(move || async move {
                assert!(started_tx.send(()).is_ok());
                assert!(finish_rx.await.is_ok());
            })
            .await
    });
    started_rx.await?;

    caller.abort();
    let caller_result = caller.await;
    assert!(matches!(caller_result, Err(error) if error.is_cancelled()));

    assert!(matches!(
        tasks.run(|| async {}).await,
        Err(SimulationTaskError::AdmissionTimedOut)
    ));

    tasks.close();
    assert!(matches!(
        tasks.run(|| async {}).await,
        Err(SimulationTaskError::Closed)
    ));

    let waiting_tasks = tasks.clone();
    let waiting = tokio::spawn(async move { waiting_tasks.wait().await });
    tokio::task::yield_now().await;
    assert!(!waiting.is_finished());

    assert!(finish_tx.send(()).is_ok());
    waiting.await?;
    Ok(())
}
