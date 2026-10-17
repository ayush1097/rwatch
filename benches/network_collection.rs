use std::hint::black_box;
use std::time::Instant;

use rwatch::network::{calculate_network_rates, read_network_stats};

const ITERATIONS: u64 = 100_000;

fn main() {
    // Benchmark reading and parsing /proc/net/dev.
    let start = Instant::now();

    let mut total_interfaces = 0usize;

    for _ in 0..ITERATIONS {
        let networks = black_box(read_network_stats());
        total_interfaces += networks.len();
    }

    let collection_elapsed = start.elapsed();

    // Capture two samples for rate calculation.
    let first = read_network_stats();
    let second = read_network_stats();

    // Benchmark network-rate calculation separately.
    let start = Instant::now();

    for _ in 0..ITERATIONS {
        black_box(calculate_network_rates(
            black_box(&first),
            black_box(&second),
            black_box(1.0),
        ));
    }

    let calculation_elapsed = start.elapsed();

    println!("=== Network Collection Benchmark ===");
    println!("Iterations: {ITERATIONS}");
    println!("Total interface records: {total_interfaces}");

    println!("\nNetwork collection:");
    print_results(collection_elapsed, ITERATIONS);

    println!("\nNetwork-rate calculation:");
    print_results(calculation_elapsed, ITERATIONS);
}

fn print_results(elapsed: std::time::Duration, iterations: u64) {
    let seconds = elapsed.as_secs_f64();

    println!("Total time: {elapsed:?}");
    println!(
        "Average time per operation: {:.2} µs",
        seconds * 1_000_000.0 / iterations as f64
    );
    println!("Operations per second: {:.0}", iterations as f64 / seconds);
}
