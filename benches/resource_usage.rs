use std::fs;
use std::thread;
use std::time::{Duration, Instant};

fn read_cpu_ticks(pid: u32) -> Option<u64> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;

    let end_name = stat.rfind(')')?;
    let fields: Vec<&str> = stat[end_name + 2..].split_whitespace().collect();

    let utime = fields.get(11)?.parse::<u64>().ok()?;
    let stime = fields.get(12)?.parse::<u64>().ok()?;

    Some(utime + stime)
}

fn read_rss_kb(pid: u32) -> Option<u64> {
    let status = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;

    for line in status.lines() {
        if let Some(value) = line.strip_prefix("VmRSS:") {
            return value.split_whitespace().next()?.parse().ok();
        }
    }

    None
}

fn main() {
    let pid = std::process::id();

    let start_ticks = read_cpu_ticks(pid);
    let start_time = Instant::now();

    let duration = Duration::from_secs(30);
    let mut total_scans = 0u64;
    let mut total_processes = 0usize;

    while start_time.elapsed() < duration {
        let processes = rwatch::process::collect_processes();

        total_scans += 1;
        total_processes += processes.len();

        thread::sleep(Duration::from_secs(1));
    }

    let elapsed = start_time.elapsed();
    let end_ticks = read_cpu_ticks(pid);
    let rss_kb = read_rss_kb(pid);

    println!("=== RWatch Resource Benchmark ===");
    println!("PID: {pid}");
    println!("Elapsed time: {:.3} seconds", elapsed.as_secs_f64());
    println!("Total scans: {total_scans}");
    println!("Total processes collected: {total_processes}");

    match (start_ticks, end_ticks) {
        (Some(start), Some(end)) => {
            let ticks = end.saturating_sub(start);
            let cpu_seconds = (ticks as f64) / 100.0;

            let cpu_utilization = (cpu_seconds / elapsed.as_secs_f64()) * 100.0;

            println!("CPU ticks consumed: {ticks}");
            println!("CPU time: {cpu_seconds:.3} seconds");
            println!("CPU utilization: {cpu_utilization:.2}% of one CPU");
        }
        _ => println!("CPU measurements unavailable"),
    }

    match rss_kb {
        Some(kb) => println!("RSS memory: {:.2} MiB", (kb as f64) / 1024.0),
        None => println!("RSS memory: unavailable"),
    }

    if total_scans > 0 {
        println!(
            "Average processes per scan: {:.2}",
            (total_processes as f64) / (total_scans as f64)
        );
    }
}
