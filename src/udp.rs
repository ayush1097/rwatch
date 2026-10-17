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

pub fn read_udp_rows(owners: &HashMap<u64, Vec<SocketOwner>>) -> Vec<UdpRow> {
    let content = fs::read_to_string("/proc/net/udp").expect("Failed to read /proc/net/udp");

    let mut udp_rows = Vec::new();

    for line in content.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();

        let (local_ip, local_port) = parse_endpoint(fields[1]);

        let (remote_ip, remote_port) = parse_endpoint(fields[2]);

        let (tx_queue, rx_queue) = parse_queue(fields[4]);

        let uid: u32 = fields[7].parse().expect("Invalid UID");

        let inode: u64 = fields[9].parse().expect("Invalid inode");

        let drops: u64 = fields[12].parse().expect("Invalid drops");

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

    udp_rows
}

pub fn read_udp6_rows(owners: &HashMap<u64, Vec<SocketOwner>>) -> Vec<UdpRow> {
    let content = fs::read_to_string("/proc/net/udp6").expect("Failed to read /proc/net/udp6");

    let mut udp_rows = Vec::new();

    for line in content.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();

        let (local_ip, local_port) = parse_ipv6_endpoint(fields[1]);

        let (remote_ip, remote_port) = parse_ipv6_endpoint(fields[2]);

        let (tx_queue, rx_queue) = parse_queue(fields[4]);

        let uid: u32 = fields[7].parse().expect("Invalid UID");

        let inode: u64 = fields[9].parse().expect("Invalid inode");

        let drops: u64 = fields[12].parse().expect("Invalid drops");

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

    udp_rows
}

fn parse_queue(queue: &str) -> (u64, u64) {
    let (tx, rx) = queue.split_once(':').expect("Invalid queue");

    let tx_queue = u64::from_str_radix(tx, 16).expect("Invalid TX queue");

    let rx_queue = u64::from_str_radix(rx, 16).expect("Invalid RX queue");

    (tx_queue, rx_queue)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_queue() {
        let (tx, rx) = parse_queue("00000010:00000020");

        assert_eq!(tx, 16);
        assert_eq!(rx, 32);
    }

    #[test]
    fn parses_queue_zeros_and_large() {
        let (tx, rx) = parse_queue("00000000:00000000");
        assert_eq!(tx, 0);
        assert_eq!(rx, 0);

        let (tx_max, rx_one) = parse_queue("FFFFFFFF:00000001");
        assert_eq!(tx_max, 4294967295);
        assert_eq!(rx_one, 1);
    }

    #[test]
    #[should_panic(expected = "Invalid queue")]
    fn parse_queue_panics_on_missing_colon() {
        parse_queue("00000010");
    }

    #[test]
    #[should_panic(expected = "Invalid TX queue")]
    fn parse_queue_panics_on_invalid_tx_hex() {
        parse_queue("INVALID!:00000020");
    }

    #[test]
    #[should_panic(expected = "Invalid RX queue")]
    fn parse_queue_panics_on_invalid_rx_hex() {
        parse_queue("00000010:INVALID!");
    }
}
