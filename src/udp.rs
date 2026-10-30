use serde::Serialize;
use std::collections::HashMap;
use std::fs;

use crate::parse::{parse_endpoint, parse_ipv6_endpoint};

use crate::socket::SocketOwner;

#[derive(Debug, Clone, Serialize)]
pub struct UdpRow {
    pub local_ip: String,
    pub local_port: u16,
    pub remote_ip: String,
    pub remote_port: u16,
    pub uid: u32,
    pub inode: u64,
    pub rx_queue: u64,
    pub tx_queue: u64,
    pub drops: u64,
    pub owners: Vec<SocketOwner>,
}

pub fn read_udp_rows(owners: &HashMap<u64, Vec<SocketOwner>>) -> std::io::Result<Vec<UdpRow>> {
    let content = fs::read_to_string("/proc/net/udp")?;

    let mut udp_rows = Vec::new();

    for line in content.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 13 {
            tracing::warn!("skipping malformed UDP entry with insufficient fields");
            continue;
        }

        let (local_ip, local_port) = match parse_endpoint(fields[1]) {
            Ok(endpoint) => endpoint,
            Err(error) => {
                tracing::warn!(%error, "skipping UDP entry with invalid local endpoint");
                continue;
            }
        };

        let (remote_ip, remote_port) = match parse_endpoint(fields[2]) {
            Ok(endpoint) => endpoint,
            Err(error) => {
                tracing::warn!(%error, "skipping UDP entry with invalid remote endpoint");
                continue;
            }
        };

        let (tx_queue, rx_queue) = match parse_queue(fields[4]) {
            Ok(queue) => queue,
            Err(error) => {
                tracing::warn!(%error, "skipping UDP entry with invalid queue");
                continue;
            }
        };
        let uid: u32 = match fields[7].parse() {
            Ok(value) => value,
            Err(_) => {
                tracing::warn!("skipping UDP entry with invalid UID");
                continue;
            }
        };

        let inode: u64 = match fields[9].parse() {
            Ok(value) => value,
            Err(_) => {
                tracing::warn!("skipping UDP entry with invalid inode");
                continue;
            }
        };

        let drops: u64 = match fields[12].parse() {
            Ok(value) => value,
            Err(_) => {
                tracing::warn!("skipping UDP entry with invalid drop count");
                continue;
            }
        };

        let socket_owners = match owners.get(&inode) {
            Some(owners) => owners.clone(),
            None => Vec::new(),
        };

        udp_rows.push(UdpRow {
            local_ip,
            local_port,
            remote_ip,
            remote_port,
            uid,
            inode,
            rx_queue,
            tx_queue,
            drops,
            owners: socket_owners,
        });
    }

    Ok(udp_rows)
}

pub fn read_udp6_rows(owners: &HashMap<u64, Vec<SocketOwner>>) -> std::io::Result<Vec<UdpRow>> {
    let content = fs::read_to_string("/proc/net/udp6")?;

    let mut udp_rows = Vec::new();

    for line in content.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 13 {
            tracing::warn!("skipping malformed UDP6 entry with insufficient fields");
            continue;
        }

        let (local_ip, local_port) = match parse_ipv6_endpoint(fields[1]) {
            Ok(endpoint) => endpoint,
            Err(error) => {
                tracing::warn!(%error, "skipping UDP6 entry with invalid local endpoint");
                continue;
            }
        };

        let (remote_ip, remote_port) = match parse_ipv6_endpoint(fields[2]) {
            Ok(endpoint) => endpoint,
            Err(error) => {
                tracing::warn!(%error, "skipping UDP6 entry with invalid remote endpoint");
                continue;
            }
        };

        let (tx_queue, rx_queue) = match parse_queue(fields[4]) {
            Ok(queue) => queue,
            Err(error) => {
                tracing::warn!(%error, "skipping UDP entry with invalid queue");
                continue;
            }
        };
        let uid: u32 = match fields[7].parse() {
            Ok(value) => value,
            Err(_) => {
                tracing::warn!("skipping UDP6 entry with invalid UID");
                continue;
            }
        };

        let inode: u64 = match fields[9].parse() {
            Ok(value) => value,
            Err(_) => {
                tracing::warn!("skipping UDP6 entry with invalid inode");
                continue;
            }
        };

        let drops: u64 = match fields[12].parse() {
            Ok(value) => value,
            Err(_) => {
                tracing::warn!("skipping UDP6 entry with invalid drop count");
                continue;
            }
        };

        let socket_owners = match owners.get(&inode) {
            Some(owners) => owners.clone(),
            None => Vec::new(),
        };

        udp_rows.push(UdpRow {
            local_ip,
            local_port,
            remote_ip,
            remote_port,
            uid,
            inode,
            rx_queue,
            tx_queue,
            drops,
            owners: socket_owners,
        });
    }

    Ok(udp_rows)
}

fn parse_queue(queue: &str) -> Result<(u64, u64), &'static str> {
    let (tx, rx) = queue.split_once(':').ok_or("Invalid queue format")?;

    let tx_queue = u64::from_str_radix(tx, 16).map_err(|_| "Invalid TX queue")?;

    let rx_queue = u64::from_str_radix(rx, 16).map_err(|_| "Invalid RX queue")?;

    Ok((tx_queue, rx_queue))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_queue() {
        assert!(parse_queue("invalid").is_err());
        assert!(parse_queue("xyz:00000020").is_err());
        assert!(parse_queue("00000010:xyz").is_err());
    }
}
