use std::fs;

#[derive(Debug)]
struct ResourceLimits {
    max_open_files: (Option<u64>, Option<u64>),
    max_processes: (Option<u64>, Option<u64>),
    max_locked_memory: (Option<u64>, Option<u64>),
    max_address_space: (Option<u64>, Option<u64>),
}

fn parse_limit(value: &str) -> Option<u64> {
    if value == "unlimited" { None } else { value.parse::<u64>().ok() }
}

fn read_resource_limits(pid: u32) -> ResourceLimits {
    let path = format!("/proc/{}/limits", pid);

    let contents = fs::read_to_string(path).expect("failed to read limits");

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
fn print_limit(name: &str, limit: (Option<u64>, Option<u64>), unit: &str) {
    println!("{}", name);

    match limit.0 {
        Some(value) => println!("  Soft: {} {}", value, unit),
        None => println!("  Soft: unlimited"),
    }

    match limit.1 {
        Some(value) => println!("  Hard: {} {}", value, unit),
        None => println!("  Hard: unlimited"),
    }

    println!();
}

fn main() {
    let pid: u32 = std::env
        ::args()
        .nth(1)
        .expect("usage: read_limits <pid>")
        .parse()
        .expect("invalid pid");

    let limits = read_resource_limits(pid);

    println!("Resource Limits");
    println!("------------------------");

    print_limit("Open files:", limits.max_open_files, "files");
    print_limit("Processes:", limits.max_processes, "processes");
    print_limit("Locked memory:", limits.max_locked_memory, "bytes");
    print_limit("Address space:", limits.max_address_space, "bytes");
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_numeric_limit() {
        assert_eq!(parse_limit("1048576"), Some(1048576));
    }

    #[test]
    fn parses_unlimited_limit() {
        assert_eq!(parse_limit("unlimited"), None);
    }
}