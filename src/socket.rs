use serde::Serialize;
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone, Serialize)]
pub struct SocketOwner {
    pub pid: u32,
    pub fd: u32,
    pub process: String,
}

pub fn get_process_name(pid: u32) -> Option<String> {
    let path = format!("/proc/{}/comm", pid);

    match fs::read_to_string(path) {
        Ok(name) => Some(name.trim().to_string()),
        Err(_) => None,
    }
}

pub fn build_socket_owners() -> HashMap<u64, Vec<SocketOwner>> {
    let mut owners = HashMap::new();

    let proc_entries = match fs::read_dir("/proc") {
        Ok(entries) => entries,
        Err(_) => {
            return owners;
        }
    };

    for entry in proc_entries.flatten() {
        let file_name = entry.file_name();
        let pid_str = file_name.to_string_lossy();

        // Only inspect numeric /proc directories
        if !pid_str.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let pid: u32 = match pid_str.parse() {
            Ok(pid) => pid,
            Err(_) => {
                continue;
            }
        };

        let process = match get_process_name(pid) {
            Some(name) => name,
            None => {
                continue;
            }
        };

        let fd_path = format!("/proc/{}/fd", pid);

        let fd_entries = match fs::read_dir(&fd_path) {
            Ok(entries) => entries,
            Err(_) => {
                continue;
            }
        };

        for fd_entry in fd_entries.flatten() {
            let fd: u32 = match fd_entry.file_name().to_string_lossy().parse() {
                Ok(fd) => fd,
                Err(_) => {
                    continue;
                }
            };

            let link = match fs::read_link(fd_entry.path()) {
                Ok(link) => link,
                Err(_) => {
                    continue;
                }
            };

            let link = link.to_string_lossy();

            let Some(inode) = parse_socket_inode(&link) else {
                continue;
            };

            owners
                .entry(inode)
                .or_insert_with(Vec::new)
                .push(SocketOwner {
                    pid,
                    fd,
                    process: process.clone(),
                });
        }
    }

    owners
}

pub fn parse_socket_inode(link: &str) -> Option<u64> {
    let inode_str = link
        .strip_prefix("socket:[")
        .and_then(|value| value.strip_suffix(']'))?;

    inode_str.parse::<u64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_socket_inodes() {
        assert_eq!(parse_socket_inode("socket:[12345]"), Some(12345));
        assert_eq!(parse_socket_inode("socket:[0]"), Some(0));
        assert_eq!(
            parse_socket_inode("socket:[18446744073709551615]"),
            Some(u64::MAX)
        );
    }

    #[test]
    fn rejects_non_socket_or_malformed_links() {
        assert_eq!(parse_socket_inode("pipe:[12345]"), None);
        assert_eq!(parse_socket_inode("/dev/null"), None);
        assert_eq!(parse_socket_inode("socket:[]"), None);
        assert_eq!(parse_socket_inode("socket:[abc]"), None);
        assert_eq!(parse_socket_inode("socket:[12345"), None);
        assert_eq!(parse_socket_inode("socket:12345]"), None);
    }
}
