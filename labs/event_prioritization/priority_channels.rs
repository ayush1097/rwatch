use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};

#[derive(Debug)]
enum CriticalEvent {
    ProcessStarted { pid: u32 },
    ProcessExited { pid: u32 },
}

#[derive(Debug)]
enum MetricEvent {
    Cpu(f64),
}

#[tokio::main]
async fn main() {
    let (critical_tx, mut critical_rx) = mpsc::channel::<CriticalEvent>(10);

    let (metrics_tx, mut metrics_rx) = mpsc::channel::<MetricEvent>(100);

    // Continuously produce routine metrics.
    let metrics_producer = tokio::spawn(async move {
        for i in 0..100 {
            if metrics_tx.send(MetricEvent::Cpu(i as f64)).await.is_err() {
                break;
            }

            sleep(Duration::from_millis(2)).await;
        }
    });

    // Give the metrics producer time to start.
    sleep(Duration::from_millis(10)).await;

    // Inject a critical event while metrics are arriving.
    critical_tx
        .send(CriticalEvent::ProcessStarted { pid: 4242 })
        .await
        .unwrap();
    critical_tx
        .send(CriticalEvent::ProcessExited { pid: 4242 })
        .await
        .unwrap();

    let mut metrics_processed = 0;

    while metrics_processed < 100 {
        tokio::select! {
            biased;

            Some(event) = critical_rx.recv() => {
                match event {
                    CriticalEvent::ProcessStarted { pid } => {
                        println!("CRITICAL: process started pid={pid}");
                    }
                    CriticalEvent::ProcessExited { pid } => {
                        println!("CRITICAL: process exited pid={pid}");
                    }
                }
            }

            Some(event) = metrics_rx.recv() => {
                match event {
                    MetricEvent::Cpu(usage) => println!("METRIC: CPU {usage}"),
                }
                metrics_processed += 1;
            }

            else => break,
        }
    }

    metrics_producer.await.unwrap();
}
