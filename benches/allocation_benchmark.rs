use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use rwatch::network::{NetworkStats, parse_network_line};

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };

        if !ptr.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }

        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };

        if !new_ptr.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }

        new_ptr
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

// Alternative implementation: no temporary Vec.
fn parse_network_line_optimized(line: &str) -> Option<NetworkStats> {
    let (interface, stats) = line.split_once(':')?;

    let mut fields = stats.split_whitespace();

    let rx_bytes = fields.next()?.parse().ok()?;
    let tx_bytes = fields.nth(7)?.parse().ok()?;

    Some(NetworkStats {
        interface: interface.trim().to_string(),
        rx_bytes,
        tx_bytes,
    })
}

fn benchmark(name: &str, parser: fn(&str) -> Option<NetworkStats>, input: &str) {
    const ITERATIONS: usize = 100_000;

    // Warm up before measuring.
    black_box(parser(input));

    let allocations_before = ALLOCATIONS.load(Ordering::Relaxed);
    let start = Instant::now();

    for _ in 0..ITERATIONS {
        black_box(parser(black_box(input)));
    }

    let elapsed = start.elapsed();
    let allocations_after = ALLOCATIONS.load(Ordering::Relaxed);

    let total_allocations = allocations_after - allocations_before;

    println!("\n=== {name} ===");
    println!("Iterations: {ITERATIONS}");
    println!("Elapsed: {elapsed:?}");
    println!("Total allocations: {total_allocations}");
    println!(
        "Allocations per operation: {:.2}",
        total_allocations as f64 / ITERATIONS as f64
    );
    println!(
        "Operations per second: {:.0}",
        ITERATIONS as f64 / elapsed.as_secs_f64()
    );
}

fn main() {
    let input = "eth0: 1000 10 20 30 40 50 60 70 2000 11 22 33 44 55 66 77";

    benchmark("Original parser", parse_network_line, input);
    benchmark("Optimized parser", parse_network_line_optimized, input);
}
