use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;

#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub start_time_ticks: u64,
    pub state: String,
    pub parent_pid: u32,
    pub memory_kb: u64,
    pub threads: u32,
    pub utime: u64,
    pub stime: u64,
    pub cpu_usage: f64,
    pub namespaces: crate::namespace::ProcessNamespaces,
    pub limits: crate::limits::ResourceLimits,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessLifecycleEvent {
    Started { pid: u32, name: String },
    Exited { pid: u32, name: String },
}

pub fn detect_lifecycle_events(
    previous: &HashMap<u32, String>,
    current: &[ProcessInfo],
) -> Vec<ProcessLifecycleEvent> {
    let current_pids: HashSet<u32> = current.iter().map(|process| process.pid).collect();

    let mut events = Vec::new();

    // Detect newly observed processes.
    for process in current {
        if !previous.contains_key(&process.pid) {
            events.push(ProcessLifecycleEvent::Started {
                pid: process.pid,
                name: process.name.clone(),
            });
        }
    }

    // Detect processes that disappeared.
    for (&pid, name) in previous {
        if !current_pids.contains(&pid) {
            events.push(ProcessLifecycleEvent::Exited {
                pid,
                name: name.clone(),
            });
        }
    }

    events
}

pub struct ProcessMonitor {
    previous_ticks: HashMap<u32, u64>,
    previous_system_ticks: u64,
}

impl ProcessMonitor {
    pub fn new() -> Self {
        let processes = collect_processes();

        let mut previous_ticks = HashMap::new();

        for process in &processes {
            previous_ticks.insert(process.pid, process.utime + process.stime);
        }

        let previous_system_ticks = read_total_cpu_ticks().unwrap_or(0);

        Self {
            previous_ticks,
            previous_system_ticks,
        }
    }
    pub fn sample(&mut self) -> Vec<ProcessInfo> {
        let mut processes = collect_processes();

        let current_system_ticks = read_total_cpu_ticks().unwrap_or(0);

        let system_tick_delta = current_system_ticks.saturating_sub(self.previous_system_ticks);

        let cpu_count = read_cpu_count();

        if system_tick_delta > 0 {
            for process in &mut processes {
                let current_ticks = process.utime + process.stime;

                if let Some(previous_ticks) = self.previous_ticks.get(&process.pid) {
                    let process_tick_delta = current_ticks.saturating_sub(*previous_ticks);

                    process.cpu_usage = ((process_tick_delta as f64) / (system_tick_delta as f64))
                        * 100.0
                        * (cpu_count as f64);
                }
            }
        }

        self.previous_ticks.clear();

        for process in &processes {
            self.previous_ticks
                .insert(process.pid, process.utime + process.stime);
        }

        self.previous_system_ticks = current_system_ticks;

        processes
    }
}
impl Default for ProcessMonitor {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_process_stat(stat: &str) -> Option<(u64, u64, u64)> {
    let end_name = stat.rfind(')')?;
    let after_name = stat.get(end_name + 2..)?;

    let mut parts = after_name.split_whitespace();

    // after_name starts at Linux field 3.
    // nth(11) retrieves field 14 (utime).
    let utime = parts.nth(11)?.parse::<u64>().ok()?;

    // The next element is field 15 (stime).
    let stime = parts.next()?.parse::<u64>().ok()?;

    // Restart the iterator so indexing is relative to field 3.
    let mut parts = after_name.split_whitespace();

    // nth(19) retrieves field 22 (starttime).
    let start_time_ticks = parts.nth(19)?.parse::<u64>().ok()?;

    Some((utime, stime, start_time_ticks))
}

pub fn read_process(pid: u32) -> Option<ProcessInfo> {
    let comm_path = format!("/proc/{}/comm", pid);
    let name = fs::read_to_string(comm_path).ok()?.trim().to_string();

    let status_path = format!("/proc/{}/status", pid);
    let status = fs::read_to_string(status_path).ok()?;

    let mut state = String::new();
    let mut parent_pid = 0;
    let mut memory_kb = 0;
    let mut threads = 0;

    for line in status.lines() {
        if let Some(value) = line.strip_prefix("State:") {
            state = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("PPid:") {
            parent_pid = value.trim().parse().ok()?;
        } else if let Some(value) = line.strip_prefix("VmRSS:") {
            let value = value.trim();
            let memory = value.split_whitespace().next()?;
            memory_kb = memory.parse().ok()?;
        } else if let Some(value) = line.strip_prefix("Threads:") {
            threads = value.trim().parse().ok()?;
        }
    }

    let stat_path = format!("/proc/{}/stat", pid);
    let stat = fs::read_to_string(stat_path).ok()?;

    let (utime, stime, start_time_ticks) = parse_process_stat(&stat)?;

    Some(ProcessInfo {
        pid,
        name,
        start_time_ticks,
        state,
        parent_pid,
        memory_kb,
        threads,
        utime,
        stime,
        cpu_usage: 0.0,
        namespaces: crate::namespace::read_namespaces(pid),
        limits: crate::limits::read_resource_limits(pid),
    })
}

pub fn collect_processes() -> Vec<ProcessInfo> {
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

        if let Ok(pid) = file_name.parse::<u32>()
            && let Some(process) = read_process(pid)
        {
            processes.push(process);
        }
    }

    processes
}

pub fn read_total_cpu_ticks() -> Option<u64> {
    let stat = fs::read_to_string("/proc/stat").ok()?;

    let first_line = stat.lines().next()?;

    let mut parts = first_line.split_whitespace();

    parts.next()?;

    Some(parts.filter_map(|value| value.parse::<u64>().ok()).sum())
}

pub fn read_cpu_count() -> usize {
    let stat = match fs::read_to_string("/proc/stat") {
        Ok(stat) => stat,
        Err(_) => {
            return 0;
        }
    };

    stat.lines()
        .filter(|line| {
            let name = line.split_whitespace().next().unwrap_or("");

            name.starts_with("cpu")
                && name != "cpu"
                && name[3..].chars().all(|c| c.is_ascii_digit())
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cpu_times_and_start_time() {
        let mut fields = vec!["0"; 20];

        fields[0] = "S"; // Linux field 3: state
        fields[11] = "120"; // Linux field 14: utime
        fields[12] = "35"; // Linux field 15: stime
        fields[19] = "987654"; // Linux field 22: starttime

        let stat = format!("1234 (test process) {}", fields.join(" "));

        println!("Synthetic stat: {stat}");

        assert_eq!(parse_process_stat(&stat), Some((120, 35, 987654)));
    }

    #[test]
    fn rejects_incomplete_process_stat() {
        let stat = "1234 (test process) S 0 0";

        assert_eq!(parse_process_stat(stat), None);
    }
}
