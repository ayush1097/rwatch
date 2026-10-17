use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    const CAPACITY: usize = 100;
    const TOTAL_EVENTS: usize = 1_000;

    let (sender, mut receiver) = mpsc::channel::<usize>(CAPACITY);

    let producer = tokio::spawn(async move {
        let mut total_send_time = Duration::ZERO;
        let mut slow_sends = 0;

        for event in 0..TOTAL_EVENTS {
            let start = Instant::now();

            if sender.send(event).await.is_err() {
                break;
            }

            let elapsed = start.elapsed();
            total_send_time += elapsed;

            // Count sends that took more than 1 ms.
            if elapsed > Duration::from_millis(1) {
                slow_sends += 1;
            }
        }

        (total_send_time, slow_sends)
    });

    let consumer = tokio::spawn(async move {
        let mut processed = 0;

        while let Some(_event) = receiver.recv().await {
            processed += 1;

            // Simulate a slow consumer.
            tokio::time::sleep(Duration::from_millis(10)).await;

            if processed == TOTAL_EVENTS {
                break;
            }
        }

        processed
    });

    let (total_send_time, slow_sends) = producer.await.unwrap();
    let processed = consumer.await.unwrap();

    println!("Channel capacity: {CAPACITY}");
    println!("Total events: {TOTAL_EVENTS}");
    println!("Events processed: {processed}");
    println!("Total time spent in send(): {total_send_time:?}");
    println!("Sends taking over 1 ms: {slow_sends}");
}
