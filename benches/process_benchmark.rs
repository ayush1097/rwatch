use std::hint::black_box;
use std::time::{Duration, Instant};

use rwatch::process::collect_processes;

const ITERATIONS: usize = 100;

fn main() {
    let mut total_time = Duration::ZERO;
    let mut min_time = Duration::MAX;
    let mut max_time = Duration::ZERO;
    let mut total_processes = 0usize;

    for _ in 0..ITERATIONS {
        let start = Instant::now();

        let processes = black_box(collect_processes());

        let elapsed = start.elapsed();

        total_time += elapsed;
        min_time = min_time.min(elapsed);
        max_time = max_time.max(elapsed);
        total_processes += processes.len();

        black_box(processes);
    }

    let average_time = total_time / ITERATIONS as u32;
    let average_processes = total_processes as f64 / ITERATIONS as f64;

    println!("=== RWatch Process Benchmark ===");
    println!("Iterations: {ITERATIONS}");
    println!("Average processes per scan: {average_processes:.2}");
    println!("Total benchmark time: {total_time:?}");
    println!("Average scan time: {average_time:?}");
    println!("Minimum scan time: {min_time:?}");
    println!("Maximum scan time: {max_time:?}");
}
