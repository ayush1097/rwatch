use std::hint::black_box;
use std::time::Instant;

use rwatch::parse::{format_endpoint, parse_endpoint, parse_ipv6_endpoint};

const ITERATIONS: u64 = 1_000_000;

fn main() {
    // IPv4 parsing
    let start = Instant::now();

    for _ in 0..ITERATIONS {
        black_box(parse_endpoint(black_box("0100007F:1F90")));
    }

    let ipv4_elapsed = start.elapsed();

    // IPv6 parsing
    let start = Instant::now();

    for _ in 0..ITERATIONS {
        black_box(parse_ipv6_endpoint(black_box(
            "00000000000000000000000001000000:1F90",
        )));
    }

    let ipv6_elapsed = start.elapsed();

    // Endpoint formatting
    let start = Instant::now();

    for _ in 0..ITERATIONS {
        black_box(format_endpoint(black_box("127.0.0.1"), black_box(8080)));
    }

    let format_elapsed = start.elapsed();

    println!("=== Network Parser Benchmark ===");
    println!("Iterations per operation: {ITERATIONS}");

    println!("\nIPv4 parsing:");
    print_results(ipv4_elapsed, ITERATIONS);

    println!("\nIPv6 parsing:");
    print_results(ipv6_elapsed, ITERATIONS);

    println!("\nEndpoint formatting:");
    print_results(format_elapsed, ITERATIONS);
}

fn print_results(elapsed: std::time::Duration, iterations: u64) {
    let total_seconds = elapsed.as_secs_f64();
    let ns_per_operation = total_seconds * 1_000_000_000.0 / iterations as f64;
    let operations_per_second = iterations as f64 / total_seconds;

    println!("Total time: {elapsed:?}");
    println!("Time per operation: {ns_per_operation:.2} ns");
    println!("Operations per second: {operations_per_second:.0}");
}
