use serde::Serialize;
use std::fs;
#[derive(Debug, Clone, Serialize)]
pub struct ResourceLimits {
    pub max_open_files: (Option<u64>, Option<u64>),
    pub max_processes: (Option<u64>, Option<u64>),
    pub max_locked_memory: (Option<u64>, Option<u64>),
    pub max_address_space: (Option<u64>, Option<u64>),
}

fn parse_limit(value: &str) -> Option<u64> {
    if value == "unlimited" {
        None
    } else {
        value.parse::<u64>().ok()
    }
}

pub fn parse_resource_limits(contents: &str) -> ResourceLimits {
    let mut limits = ResourceLimits {
        max_open_files: (None, None),
        max_processes: (None, None),
        max_locked_memory: (None, None),
        max_address_space: (None, None),
    };

    for line in contents.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();

        if parts.len() < 5 {
            continue;
        }

        if line.starts_with("Max open files") {
            limits.max_open_files = (parse_limit(parts[3]), parse_limit(parts[4]));
        } else if line.starts_with("Max processes") {
            limits.max_processes = (parse_limit(parts[2]), parse_limit(parts[3]));
        } else if line.starts_with("Max locked memory") {
            limits.max_locked_memory = (parse_limit(parts[3]), parse_limit(parts[4]));
        } else if line.starts_with("Max address space") {
            limits.max_address_space = (parse_limit(parts[3]), parse_limit(parts[4]));
        }
    }

    limits
}

pub fn read_resource_limits(pid: u32) -> ResourceLimits {
    let path = format!("/proc/{}/limits", pid);
    let contents = fs::read_to_string(path).unwrap_or_default();
    parse_resource_limits(&contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_numeric_limit() {
        assert_eq!(parse_limit("1048576"), Some(1048576));
        assert_eq!(parse_limit("0"), Some(0));
    }

    #[test]
    fn parses_unlimited_limit() {
        assert_eq!(parse_limit("unlimited"), None);
    }

    #[test]
    fn parses_invalid_limit() {
        assert_eq!(parse_limit("invalid"), None);
    }

    #[test]
    fn parses_full_resource_limits() {
        let sample = "\
Limit                     Soft Limit           Hard Limit           Units
Max processes             62828                62828                processes
Max open files            1024                 1048576              files
Max locked memory         65536                65536                bytes
Max address space         unlimited            unlimited            bytes
";
        let limits = parse_resource_limits(sample);
        assert_eq!(limits.max_processes, (Some(62828), Some(62828)));
        assert_eq!(limits.max_open_files, (Some(1024), Some(1048576)));
        assert_eq!(limits.max_locked_memory, (Some(65536), Some(65536)));
        assert_eq!(limits.max_address_space, (None, None));
    }

    #[test]
    fn handles_empty_and_short_lines() {
        let sample = "Short line\nAnother short\n";
        let limits = parse_resource_limits(sample);
        assert_eq!(limits.max_open_files, (None, None));
        assert_eq!(limits.max_processes, (None, None));
        assert_eq!(limits.max_locked_memory, (None, None));
        assert_eq!(limits.max_address_space, (None, None));
    }
}
