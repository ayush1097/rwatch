use std::time::Instant;

use rwatch::process::collect_processes;

fn main() {
    let start = Instant::now();

    let processes = collect_processes();

    let elapsed = start.elapsed();

    println!("Processes collected: {}", processes.len());
    println!("Total scan time: {:?}", elapsed);

    if !processes.is_empty() {
        let average_time = elapsed.as_secs_f64() * 1000.0 / processes.len() as f64;

        println!("Average scan time per process: {:.4} ms", average_time);
    }
}
