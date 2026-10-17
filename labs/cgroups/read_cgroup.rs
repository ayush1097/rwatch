use std::fs;
use std::path::Path;
use std::io;

struct Cgroup {
    path: String,
    pids: Vec<u32>,
    parent: Option<String>,
    memory_current: Option<u64>,
    memory_max: Option<u64>,

    cpu_usage_usec: u64,
    cpu_user_usec: u64,
    cpu_system_usec: u64,

    cpu_quota: Option<u64>,
    cpu_period_usec: Option<u64>,
    pids_current: u64,
    pids_max: Option<u64>,
}

fn try_read_u64<F>(reader: F) -> Option<u64> where F: FnOnce() -> io::Result<u64> {
    reader().ok()
}

fn read_cgroup(path: &str, parent: Option<String>) -> io::Result<Cgroup> {
    let pids = read_cgroup_pids(path)?;

    let memory_current = try_read_u64(|| read_memory_current(path));

    let memory_max = match read_memory_max(path) {
        Ok(value) => value,
        Err(_) => None,
    };

    let (cpu_quota, cpu_period_usec) = match read_cpu_max(path) {
        Ok(value) => value,
        Err(_) => (None, None),
    };

    let (cpu_usage_usec, cpu_user_usec, cpu_system_usec) = match read_cpu_stat(path) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("{} -> cpu.stat FAILED: {}", path, error);
            return Err(error);
        }
    };

    let (pids_current, pids_max) = match read_pids(path) {
        Ok(value) => value,
        Err(_) => (0, None),
    };

    Ok(Cgroup {
        path: path.to_string(),
        pids,
        parent,
        memory_current,
        memory_max,
        cpu_usage_usec,
        cpu_user_usec,
        cpu_system_usec,
        cpu_quota,
        cpu_period_usec,
        pids_current,
        pids_max,
    })
}
fn discover_cgroups() -> io::Result<(Vec<Cgroup>, usize, usize)> {
    let root = Path::new("/sys/fs/cgroup");

    let mut cgroups = Vec::new();
    let mut total_cgroups = 0;
    let mut sub_cgroups = 0;

    discover_recursive(root, "", &mut cgroups, &mut total_cgroups, &mut sub_cgroups)?;

    Ok((cgroups, total_cgroups, sub_cgroups))
}
fn discover_recursive(
    directory: &std::path::Path,
    cgroup_path: &str,
    cgroups: &mut Vec<Cgroup>,
    total_cgroups: &mut usize,
    sub_cgroups: &mut usize
) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();

        let current_path = if cgroup_path.is_empty() {
            format!("/{}", name)
        } else {
            format!("{}/{}", cgroup_path, name)
        };

        // Count this cgroup
        *total_cgroups += 1;

        // If it has a parent path, it is a sub-cgroup
        if !cgroup_path.is_empty() {
            *sub_cgroups += 1;
        }

        // Store the parent cgroup
        let parent = if cgroup_path.is_empty() { None } else { Some(cgroup_path.to_string()) };

        match read_cgroup(&current_path, parent) {
            Ok(cgroup) => {
                cgroups.push(cgroup);
            }

            Err(error) => {
                eprintln!("Skipping {}: {}", current_path, error);
            }
        }

        discover_recursive(&path, &current_path, cgroups, total_cgroups, sub_cgroups)?;
    }

    Ok(())
}

fn print_cgroup_tree(cgroups: &[Cgroup], parent: Option<&str>, depth: usize) {
    for cgroup in cgroups {
        let current_parent = cgroup.parent.as_deref();

        if current_parent != parent {
            continue;
        }

        let indent = "    ".repeat(depth);

        println!("{}├── {}", indent, cgroup.path);

        print_cgroup_tree(cgroups, Some(&cgroup.path), depth + 1);
    }
}

fn read_cgroup_pids(cgroup_path: &str) -> io::Result<Vec<u32>> {
    let path = format!("/sys/fs/cgroup{}/cgroup.procs", cgroup_path);

    let content = fs::read_to_string(path)?;

    let mut pids = Vec::new();

    for line in content.lines() {
        let pid = line
            .trim()
            .parse::<u32>()
            .map_err(|_| { io::Error::new(io::ErrorKind::InvalidData, "Invalid PID") })?;

        if pid != 0 {
            pids.push(pid);
        }
    }

    Ok(pids)
}


fn read_memory_current(cgroup_path: &str) -> io::Result<u64> {
    let path = format!("/sys/fs/cgroup{}/memory.current", cgroup_path);

    let value = fs::read_to_string(path)?;

    let memory = value
        .trim()
        .parse::<u64>()
        .map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "invalid memory.current value")
        })?;

    Ok(memory)
}
fn read_memory_max(cgroup_path: &str) -> io::Result<Option<u64>> {
    let path = format!("/sys/fs/cgroup{}/memory.max", cgroup_path);

    let value = fs::read_to_string(path)?;
    let value = value.trim();

    if value == "max" {
        return Ok(None);
    }

    let memory = value
        .parse::<u64>()
        .map_err(|_| { io::Error::new(io::ErrorKind::InvalidData, "invalid memory.max value") })?;

    Ok(Some(memory))
}
fn read_cpu_max(path: &str) -> io::Result<(Option<u64>, Option<u64>)> {
    let full_path = format!("/sys/fs/cgroup{}/cpu.max", path);

    let content = fs::read_to_string(full_path)?;
    let mut parts = content.split_whitespace();

    let quota = parts.next().unwrap();
    let period = parts.next().unwrap().parse::<u64>().ok();

    let quota = if quota == "max" { None } else { quota.parse::<u64>().ok() };

    Ok((quota, period))
}
fn read_cpu_stat(cgroup_path: &str) -> io::Result<(u64, u64, u64)> {
    let path = format!("/sys/fs/cgroup{}/cpu.stat", cgroup_path);

    let content = fs::read_to_string(path)?;

    let mut usage = None;
    let mut user = None;
    let mut system = None;

    for line in content.lines() {
        let mut parts = line.split_whitespace();

        let key = parts.next();
        let value = parts.next();

        match (key, value) {
            (Some("usage_usec"), Some(value)) => {
                usage = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| {
                            io::Error::new(io::ErrorKind::InvalidData, "invalid usage_usec")
                        })?
                );
            }

            (Some("user_usec"), Some(value)) => {
                user = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| {
                            io::Error::new(io::ErrorKind::InvalidData, "invalid user_usec")
                        })?
                );
            }

            (Some("system_usec"), Some(value)) => {
                system = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| {
                            io::Error::new(io::ErrorKind::InvalidData, "invalid system_usec")
                        })?
                );
            }

            _ => {}
        }
    }

    let usage = usage.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "usage_usec missing")
    })?;

    let user = user.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "user_usec missing")
    })?;

    let system = system.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "system_usec missing")
    })?;

    Ok((usage, user, system))
}
fn read_pids(cgroup_path: &str) -> io::Result<(u64, Option<u64>)> {
    let current_path = format!("/sys/fs/cgroup{}/pids.current", cgroup_path);

    let max_path = format!("/sys/fs/cgroup{}/pids.max", cgroup_path);

    let current = fs
        ::read_to_string(current_path)?
        .trim()
        .parse::<u64>()
        .map_err(|_| { io::Error::new(io::ErrorKind::InvalidData, "invalid pids.current") })?;

    let max_value = fs::read_to_string(max_path)?;
    let max_value = max_value.trim();

    let max = if max_value == "max" {
        None
    } else {
        Some(
            max_value
                .parse::<u64>()
                .map_err(|_| { io::Error::new(io::ErrorKind::InvalidData, "invalid pids.max") })?
        )
    };

    Ok((current, max))
}
fn main() -> io::Result<()> {
    let (cgroups, total_cgroups, sub_cgroups) = discover_cgroups()?;

    println!("Cgroup Summary");
    println!("------------------------");
    println!("Total cgroups: {}", total_cgroups);
    println!("Sub-cgroups: {}", sub_cgroups);
    println!("------------------------");

    println!("Cgroup Hierarchy");
    println!("------------------------");

    print_cgroup_tree(&cgroups, None, 0);

    println!("------------------------");

    for cgroup in cgroups {
        println!("Cgroup: {}", cgroup.path);
        println!("Parent: {:?}", cgroup.parent);
        println!("PIDs: {:?}", cgroup.pids);

        match cgroup.memory_current {
            Some(value) => println!("Memory current: {} bytes", value),
            None => println!("Memory current: unavailable"),
        }

        println!("Memory max: {:?}", cgroup.memory_max);

        println!(
            "CPU usage: {} μs ({:.3} sec)",
            cgroup.cpu_usage_usec,
            (cgroup.cpu_usage_usec as f64) / 1_000_000.0
        );

        println!(
            "CPU user: {} μs ({:.3} sec)",
            cgroup.cpu_user_usec,
            (cgroup.cpu_user_usec as f64) / 1_000_000.0
        );

        println!(
            "CPU system: {} μs ({:.3} sec)",
            cgroup.cpu_system_usec,
            (cgroup.cpu_system_usec as f64) / 1_000_000.0
        );

        println!("CPU quota: {:?}", cgroup.cpu_quota);
        println!("CPU period: {:?} μs", cgroup.cpu_period_usec);
        println!("PIDs current: {}", cgroup.pids_current);
        println!("PIDs max: {:?}", cgroup.pids_max);
        println!("------------------------");
    }

    Ok(())
}
//////rustc labs/cgroups/read_cgroup.rs -o /tmp/read_cgroup
////   /tmp/read_cgroup