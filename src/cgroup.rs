use serde::Serialize;
use std::fs;
use std::io;
use std::path::Path;
#[derive(Debug, Clone, Serialize)]

pub struct Cgroup {
    pub path: String,
    pub pids: Vec<u32>,
    pub parent: Option<String>,
    pub memory_current: Option<u64>,
    pub memory_max: Option<u64>,
    pub cpu_usage_usec: u64,
    pub cpu_user_usec: u64,
    pub cpu_system_usec: u64,
    pub cpu_quota: Option<u64>,
    pub cpu_period_usec: Option<u64>,
    pub pids_current: u64,
    pub pids_max: Option<u64>,
}

pub fn discover_cgroups() -> io::Result<(Vec<Cgroup>, usize, usize)> {
    let root = Path::new("/sys/fs/cgroup");

    let mut cgroups = Vec::new();
    let mut total_cgroups = 0;
    let mut sub_cgroups = 0;

    discover_recursive(root, "", &mut cgroups, &mut total_cgroups, &mut sub_cgroups)?;

    Ok((cgroups, total_cgroups, sub_cgroups))
}
fn read_cgroup(path: &str, parent: Option<String>) -> io::Result<Cgroup> {
    let pids = read_cgroup_pids(path)?;

    let memory_current = try_read_u64(|| read_memory_current(path));

    let memory_max = read_memory_max(path).unwrap_or_default();

    let (cpu_quota, cpu_period_usec) = read_cpu_max(path).unwrap_or_default();

    let (cpu_usage_usec, cpu_user_usec, cpu_system_usec) = match read_cpu_stat(path) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("{} -> cpu.stat FAILED: {}", path, error);
            return Err(error);
        }
    };

    let (pids_current, pids_max) = read_pids(path).unwrap_or_default();

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
fn discover_recursive(
    directory: &std::path::Path,
    cgroup_path: &str,
    cgroups: &mut Vec<Cgroup>,
    total_cgroups: &mut usize,
    sub_cgroups: &mut usize,
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
        let parent = if cgroup_path.is_empty() {
            None
        } else {
            Some(cgroup_path.to_string())
        };

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
pub fn parse_memory_current(value: &str) -> io::Result<u64> {
    value
        .trim()
        .parse::<u64>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid memory.current value"))
}

fn read_memory_current(cgroup_path: &str) -> io::Result<u64> {
    let path = format!("/sys/fs/cgroup{}/memory.current", cgroup_path);
    let value = fs::read_to_string(path)?;
    parse_memory_current(&value)
}

pub fn parse_memory_max(value: &str) -> io::Result<Option<u64>> {
    let value = value.trim();

    if value == "max" {
        return Ok(None);
    }

    let memory = value
        .parse::<u64>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid memory.max value"))?;

    Ok(Some(memory))
}

fn read_memory_max(cgroup_path: &str) -> io::Result<Option<u64>> {
    let path = format!("/sys/fs/cgroup{}/memory.max", cgroup_path);
    let value = fs::read_to_string(path)?;
    parse_memory_max(&value)
}

pub fn parse_cpu_max(content: &str) -> io::Result<(Option<u64>, Option<u64>)> {
    let mut parts = content.split_whitespace();

    let quota = parts
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing quota"))?;
    let period = parts.next().and_then(|p| p.parse::<u64>().ok());

    let quota = if quota == "max" {
        None
    } else {
        quota.parse::<u64>().ok()
    };

    Ok((quota, period))
}

fn read_cpu_max(path: &str) -> io::Result<(Option<u64>, Option<u64>)> {
    let full_path = format!("/sys/fs/cgroup{}/cpu.max", path);
    let content = fs::read_to_string(full_path)?;
    parse_cpu_max(&content)
}

pub fn parse_cpu_stat(content: &str) -> io::Result<(u64, u64, u64)> {
    let mut usage = None;
    let mut user = None;
    let mut system = None;

    for line in content.lines() {
        let mut parts = line.split_whitespace();

        let key = parts.next();
        let value = parts.next();

        match (key, value) {
            (Some("usage_usec"), Some(value)) => {
                usage = Some(value.parse::<u64>().map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "invalid usage_usec")
                })?);
            }

            (Some("user_usec"), Some(value)) => {
                user = Some(value.parse::<u64>().map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "invalid user_usec")
                })?);
            }

            (Some("system_usec"), Some(value)) => {
                system = Some(value.parse::<u64>().map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "invalid system_usec")
                })?);
            }

            _ => {}
        }
    }

    let usage =
        usage.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "usage_usec missing"))?;

    let user =
        user.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "user_usec missing"))?;

    let system =
        system.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "system_usec missing"))?;

    Ok((usage, user, system))
}

fn read_cpu_stat(cgroup_path: &str) -> io::Result<(u64, u64, u64)> {
    let path = format!("/sys/fs/cgroup{}/cpu.stat", cgroup_path);
    let content = fs::read_to_string(path)?;
    parse_cpu_stat(&content)
}

pub fn parse_pids(current_str: &str, max_str: &str) -> io::Result<(u64, Option<u64>)> {
    let current = current_str
        .trim()
        .parse::<u64>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid pids.current"))?;

    let max_value = max_str.trim();

    let max = if max_value == "max" {
        None
    } else {
        Some(
            max_value
                .parse::<u64>()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid pids.max"))?,
        )
    };

    Ok((current, max))
}

fn read_pids(cgroup_path: &str) -> io::Result<(u64, Option<u64>)> {
    let current_path = format!("/sys/fs/cgroup{}/pids.current", cgroup_path);
    let max_path = format!("/sys/fs/cgroup{}/pids.max", cgroup_path);

    let current = fs::read_to_string(current_path)?;
    let max_value = fs::read_to_string(max_path)?;

    parse_pids(&current, &max_value)
}

pub fn parse_cgroup_procs(content: &str) -> io::Result<Vec<u32>> {
    let mut pids = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let pid = trimmed
            .parse::<u32>()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid PID"))?;

        if pid != 0 {
            pids.push(pid);
        }
    }

    Ok(pids)
}

fn read_cgroup_pids(cgroup_path: &str) -> io::Result<Vec<u32>> {
    let path = format!("/sys/fs/cgroup{}/cgroup.procs", cgroup_path);
    let content = fs::read_to_string(path)?;
    parse_cgroup_procs(&content)
}

fn try_read_u64<F>(reader: F) -> Option<u64>
where
    F: FnOnce() -> io::Result<u64>,
{
    reader().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_memory_current_values() {
        assert_eq!(parse_memory_current("10485760\n").unwrap(), 10485760);
        assert_eq!(parse_memory_current("0").unwrap(), 0);
        assert!(parse_memory_current("invalid").is_err());
    }

    #[test]
    fn parses_memory_max_values() {
        assert_eq!(parse_memory_max("max\n").unwrap(), None);
        assert_eq!(parse_memory_max("52428800").unwrap(), Some(52428800));
        assert!(parse_memory_max("bad_max").is_err());
    }

    #[test]
    fn parses_cpu_max_unlimited_and_quota() {
        let (quota, period) = parse_cpu_max("max 100000").unwrap();
        assert_eq!(quota, None);
        assert_eq!(period, Some(100000));

        let (quota2, period2) = parse_cpu_max("20000 100000").unwrap();
        assert_eq!(quota2, Some(20000));
        assert_eq!(period2, Some(100000));

        assert!(parse_cpu_max("").is_err());
    }

    #[test]
    fn parses_cpu_stat_data() {
        let content = "\
usage_usec 987654
user_usec 500000
system_usec 487654
nr_periods 100
nr_throttled 0
throttled_usec 0
";
        let (usage, user, system) = parse_cpu_stat(content).unwrap();
        assert_eq!(usage, 987654);
        assert_eq!(user, 500000);
        assert_eq!(system, 487654);
    }

    #[test]
    fn parses_cpu_stat_fails_on_missing_fields() {
        let content = "usage_usec 100\nuser_usec 50\n";
        assert!(parse_cpu_stat(content).is_err());
    }

    #[test]
    fn parses_pids_limits() {
        let (cur, max) = parse_pids("42\n", "max\n").unwrap();
        assert_eq!(cur, 42);
        assert_eq!(max, None);

        let (cur2, max2) = parse_pids("10\n", "500\n").unwrap();
        assert_eq!(cur2, 10);
        assert_eq!(max2, Some(500));
    }

    #[test]
    fn parses_cgroup_procs_list() {
        let content = "1001\n0\n1002\n\n1003\n";
        let pids = parse_cgroup_procs(content).unwrap();
        assert_eq!(pids, vec![1001, 1002, 1003]);
    }
}
