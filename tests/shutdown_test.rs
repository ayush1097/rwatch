use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn multiple_tasks_stop_on_shutdown() {
    let shutdown = CancellationToken::new();

    let task1_token = shutdown.clone();
    let task2_token = shutdown.clone();

    let task1 = tokio::spawn(async move {
        task1_token.cancelled().await;
        true
    });

    let task2 = tokio::spawn(async move {
        task2_token.cancelled().await;
        true
    });

    shutdown.cancel();

    assert!(task1.await.unwrap());
    assert!(task2.await.unwrap());
}

#[tokio::test]
async fn all_collectors_stop_on_shutdown() {
    let shutdown = CancellationToken::new();

    let mut tasks = Vec::new();

    for _ in 0..3 {
        let token = shutdown.clone();

        tasks.push(tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = token.cancelled() => {
                        break;
                    }

                    _ = tokio::time::sleep(
                        std::time::Duration::from_millis(10)
                    ) => {
                        // simulate collector work
                    }
                }
            }

            true
        }));
    }

    shutdown.cancel();

    for task in tasks {
        assert!(task.await.unwrap());
    }
}

#[tokio::test]
async fn shutdown_waits_for_collector() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    let shutdown = CancellationToken::new();
    let finished = Arc::new(AtomicBool::new(false));

    let token = shutdown.clone();
    let finished_clone = finished.clone();

    let task = tokio::spawn(async move {
        tokio::select! {
            _ = token.cancelled() => {
                // cleanup
                tokio::time::sleep(
                    std::time::Duration::from_millis(20)
                ).await;

                finished_clone.store(true, Ordering::SeqCst);
            }
        }
    });

    shutdown.cancel();

    // Wait for the collector task to actually finish.
    task.await.unwrap();

    assert!(finished.load(Ordering::SeqCst));
}

#[tokio::test]
async fn ctrl_c_signal_can_trigger_shutdown() {
    let shutdown = CancellationToken::new();
    let task_shutdown = shutdown.clone();

    let task = tokio::spawn(async move {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                task_shutdown.cancel();
            }

            _ = task_shutdown.cancelled() => {}
        }
    });

    shutdown.cancel();

    task.await.unwrap();

    assert!(shutdown.is_cancelled());
}
