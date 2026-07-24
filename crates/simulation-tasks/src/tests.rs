use std::{error::Error, num::NonZeroUsize};

use tokio::sync::oneshot;

use super::SimulationTaskSet;

#[tokio::test]
async fn started_attempt_outlives_caller_and_keeps_capacity() -> Result<(), Box<dyn Error>> {
    let tasks = SimulationTaskSet::new(NonZeroUsize::MIN);
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
    assert_eq!(tasks.permits.available_permits(), 0);

    caller.abort();
    let caller_result = caller.await;
    assert!(matches!(caller_result, Err(error) if error.is_cancelled()));
    assert_eq!(tasks.permits.available_permits(), 0);

    assert!(finish_tx.send(()).is_ok());
    assert_eq!(tasks.run(|| async { 9 }).await?, 9);
    Ok(())
}
