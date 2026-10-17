use std::collections::HashMap;
use std::fs;
use std::io::{ self, Write };
use std::thread;
use std::time::Duration;

struct ProcessInfo {
    pid: u32,
    name: String,
    state: String,
    parent_pid: u32,
    memory_kb: u64,
    threads: u32,
    utime: u64,
    stime: u64,
    cpu_usage: f64,
}
#[derive(Debug)]
enum NamespaceError {
    PermissionDenied,
    InvalidFormat,
    InvalidId,
    Io,
}
#[derive(Debug)]
struct ProcessNamespaces {
    pid: Result<u64, NamespaceError>,
    net: Result<u64, NamespaceError>,
    mnt: Result<u64, NamespaceError>,
    user: Result<u64, NamespaceError>,
}
fn read_namespaces(pid: u32) -> ProcessNamespaces {
    ProcessNamespaces {
        pid: read_namespace(pid, "pid"),
        net: read_namespace(pid, "net"),
        mnt: read_namespace(pid, "mnt"),
        user: read_namespace(pid, "user"),
    }
}
fn namespace_id(result: &Result<u64, NamespaceError>) -> String {
    match result {
        Ok(id) => id.to_string(),
        Err(NamespaceError::PermissionDenied) => "permission denied".to_string(),
        Err(_) => "unavailable".to_string(),
    }
}
fn read_namespace(pid: u32, namespace: &str) -> Result<u64, NamespaceError> {
    let path = format!("/proc/{}/ns/{}", pid, namespace);

    let target = fs::read_link(path).map_err(|e| {
        if e.kind() == io::ErrorKind::PermissionDenied {
            NamespaceError::PermissionDenied
        } else {
            NamespaceError::Io
        }
    })?;

    let target = target.to_string_lossy();

    let start = target.find('[').ok_or(NamespaceError::InvalidFormat)? + 1;

    let end = target.find(']').ok_or(NamespaceError::InvalidFormat)?;

    let id = target[start..end].parse::<u64>().map_err(|_| NamespaceError::InvalidId)?;

    Ok(id)
}
// =============================================
// Read information about one process
// =============================================
fn read_process(pid: u32) -> Option<ProcessInfo> {
    // -------------------------
    // Read Process Name
    // -------------------------
    let comm_path = format!("/proc/{}/comm", pid);
    let name = fs::read_to_string(comm_path).ok()?.trim().to_string();

    // -------------------------
    // Read Process Status
    // -------------------------
    let status_path = format!("/proc/{}/status", pid);
    let status = fs::read_to_string(status_path).ok()?;

    let mut state = String::new();
    let mut parent_pid = 0;
    let mut memory_kb = 0;
    let mut threads = 0;

    for line in status.lines() {
        if let Some(value) = line.strip_prefix("State:") {
            state = value.trim().to_string();
        }

        if let Some(value) = line.strip_prefix("PPid:") {
            parent_pid = value.trim().parse().ok()?;
        }

        if let Some(value) = line.strip_prefix("VmRSS:") {
            let value = value.trim();

            if let Some(memory) = value.split_whitespace().next() {
                memory_kb = memory.parse().ok()?;
            }
        }

        if let Some(value) = line.strip_prefix("Threads:") {
            threads = value.trim().parse().ok()?;
        }
    }

    // -------------------------
    // Read Process CPU Ticks
    // -------------------------
    let stat_path = format!("/proc/{}/stat", pid);
    let stat = fs::read_to_string(stat_path).ok()?;

    // Process names can contain spaces.
    // Find the closing ')'.
    let end_name = stat.rfind(')')?;

    // Everything after process name
    let after_name = &stat[end_name + 2..];

    let parts: Vec<&str> = after_name.split_whitespace().collect();

    // Original /proc/[pid]/stat:
    //
    // Field 14 = utime
    // Field 15 = stime
    //
    // After removing:
    // PID and process name
    //
    // utime = index 11
    // stime = index 12

    let utime: u64 = parts.get(11)?.parse().ok()?;
    let stime: u64 = parts.get(12)?.parse().ok()?;

    Some(ProcessInfo {
        pid,
        name,
        state,
        parent_pid,
        memory_kb,
        threads,
        utime,
        stime,
        cpu_usage: 0.0,
    })
}

// =============================================
// Collect all processes
// =============================================
fn collect_processes() -> Vec<ProcessInfo> {
    let mut processes = Vec::new();

    let entries = match fs::read_dir("/proc") {
        Ok(entries) => entries,
        Err(_) => {
            return processes;
        }
    };

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();

        // Only numeric directories are PIDs
        if let Ok(pid) = file_name.parse::<u32>() {
            if let Some(process) = read_process(pid) {
                processes.push(process);
            }
        }
    }

    processes
}

// =============================================
// Read total system CPU ticks
// =============================================
fn read_total_cpu_ticks() -> Option<u64> {
    let stat = fs::read_to_string("/proc/stat").ok()?;

    // First line:
    //
    // cpu  user nice system idle ...
    let first_line = stat.lines().next()?;

    let mut parts = first_line.split_whitespace();

    // Skip "cpu"
    parts.next()?;

    let total: u64 = parts.filter_map(|value| value.parse::<u64>().ok()).sum();

    Some(total)
}

// =============================================
// Count logical CPUs
// =============================================
fn read_cpu_count() -> usize {
    let stat = match fs::read_to_string("/proc/stat") {
        Ok(stat) => stat,
        Err(_) => {
            return 0;
        }
    };

    stat.lines()
        .filter(|line| {
            let name = line.split_whitespace().next().unwrap_or("");

            name.starts_with("cpu") &&
                name != "cpu" &&
                name[3..].chars().all(|c| c.is_ascii_digit())
        })
        .count()
}

// =============================================
// Main
// =============================================
fn main() {
    let mut previous_processes = collect_processes();
    let mut previous_ticks: HashMap<u32, u64> = HashMap::new();

    for process in &previous_processes {
        previous_ticks.insert(process.pid, process.utime + process.stime);
    }

    let mut previous_system_ticks = read_total_cpu_ticks().unwrap_or(0);

    loop {
        // Wait before collecting next snapshot
        thread::sleep(Duration::from_secs(1));

        // Collect current processes
        let mut current_processes = collect_processes();

        // Collect current system CPU ticks
        let current_system_ticks = read_total_cpu_ticks().unwrap_or(0);

        let system_tick_delta = current_system_ticks.saturating_sub(previous_system_ticks);

        // Calculate CPU usage
        if system_tick_delta > 0 {
            for process in &mut current_processes {
                let current_ticks = process.utime + process.stime;

                if let Some(previous) = previous_ticks.get(&process.pid) {
                    let process_tick_delta = current_ticks.saturating_sub(*previous);

                    process.cpu_usage =
                        ((process_tick_delta as f64) / (system_tick_delta as f64)) *
                        100.0 *
                        (read_cpu_count() as f64);
                }
            }
        }

        // Sort by CPU usage
        current_processes.sort_by(|a, b| { b.cpu_usage.partial_cmp(&a.cpu_usage).unwrap() });

        // Clear terminal
        print!("\x1B[2J\x1B[1;1H");
        io::stdout().flush().unwrap();

        // Display system information
        println!("RWatch - Process Monitor");
        println!(
            "------------------------------------------------------------------------------------------------"
        );

        println!(
            "Processes: {} | Logical CPUs: {} | System Tick Delta: {}",
            current_processes.len(),
            read_cpu_count(),
            system_tick_delta
        );

        println!();

        println!(
            "{:<8} {:<22} {:<16} {:<8} {:<10} {:<12} {:<8}",
            "PID",
            "NAME",
            "STATE",
            "PPID",
            "CPU %",
            "MEMORY",
            "THREADS"
        );

        println!(
            "------------------------------------------------------------------------------------------------"
        );

        // Display top 10 processes
        for process in current_processes.iter().take(10) {
            println!(
                "{:<8} {:<22} {:<16} {:<8} {:<10.2} {:<12} {:<8}",
                process.pid,
                process.name,
                process.state,
                process.parent_pid,
                process.cpu_usage,
                format!("{} KB", process.memory_kb),
                process.threads
            );

        }

        // Create next previous snapshot
        previous_ticks.clear();

        for process in &current_processes {
            previous_ticks.insert(process.pid, process.utime + process.stime);
        }

        previous_system_ticks = current_system_ticks;
        previous_processes = current_processes;
    }
}
