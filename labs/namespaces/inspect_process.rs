use std::fs;
use std::io;

#[derive(Debug, PartialEq)]
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

    target[start..end].parse::<u64>().map_err(|_| NamespaceError::InvalidId)
}

fn read_namespaces(pid: u32) -> ProcessNamespaces {
    ProcessNamespaces {
        pid: read_namespace(pid, "pid"),
        net: read_namespace(pid, "net"),
        mnt: read_namespace(pid, "mnt"),
        user: read_namespace(pid, "user"),
    }
}

fn print_namespace(name: &str, result: &Result<u64, NamespaceError>) {
    match result {
        Ok(id) => println!("{:<5} namespace: {} ({})", name, id, namespace_description(name)),

        Err(NamespaceError::PermissionDenied) => {
            println!("{:<5} namespace: unavailable (permission denied)", name)
        }

        Err(error) => { println!("{:<5} namespace: unavailable ({:?})", name, error) }
    }
}
fn namespace_description(name: &str) -> &str {
    match name {
        "PID" => "process ID isolation",
        "NET" => "network isolation",
        "MNT" => "mount/filesystem isolation",
        "USER" => "user/UID isolation",
        _ => "unknown",
    }
}
 fn compare_namespaces(
    first: &ProcessNamespaces,
    second: &ProcessNamespaces,
) {
    println!(
        "PID namespace:  {}",
        if first.pid == second.pid {
            "SAME"
        } else {
            "DIFFERENT"
        }
    );

    println!(
        "NET namespace:  {}",
        if first.net == second.net {
            "SAME"
        } else {
            "DIFFERENT"
        }
    );

    println!(
        "MNT namespace:  {}",
        if first.mnt == second.mnt {
            "SAME"
        } else {
            "DIFFERENT"
        }
    );

    println!(
        "USER namespace: {}",
        if first.user == second.user {
            "SAME"
        } else {
            "DIFFERENT"
        }
    );
}
fn main() {
    let mut args = std::env::args().skip(1);

    let first_pid = args
        .next()
        .expect("usage: inspect_process <pid1> <pid2>")
        .parse::<u32>()
        .expect("PID must be a number");

    let second_pid = args
        .next()
        .expect("usage: inspect_process <pid1> <pid2>")
        .parse::<u32>()
        .expect("PID must be a number");

    let first = read_namespaces(first_pid);
    let second = read_namespaces(second_pid);

    println!("Process 1: {}", first_pid);
    println!("Process 2: {}", second_pid);
    println!("------------------------");

    compare_namespaces(&first, &second);
}