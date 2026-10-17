use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone)]
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

            let Some(inode_str) = link
                .strip_prefix("socket:[")
                .and_then(|value| value.strip_suffix(']'))
            else {
                continue;
            };

            let inode: u64 = match inode_str.parse() {
                Ok(inode) => inode,
                Err(_) => {
                    continue;
                }
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