use std::collections::HashMap;
use std::fs;
use crate::parse::{
    format_endpoint,
    parse_endpoint,
    parse_ipv6_endpoint,
};
use crate::socket_owner::SocketOwner;

#[derive(Debug, Clone)]
pub struct UdpRow {
    local_ip: String,
    local_port: u16,
    remote_ip: String,
    remote_port: u16,
    uid: u32,
    inode: u64,
    rx_queue: u64,
    tx_queue: u64,
    drops: u64,
    owners: Vec<SocketOwner>,
}
pub fn read_udp_rows(
    owners: &HashMap<u64, Vec<SocketOwner>>,
) -> std::io::Result<Vec<UdpRow>> {
    let content = fs::read_to_string("/proc/net/udp")?;

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

    Ok(udp_rows)
}
// ==========================================
// read udp_rows_ipv6
// ==========================================
pub  fn read_udp6_rows(owners: &HashMap<u64, Vec<SocketOwner>>) -> std::io::Result<Vec<UdpRow>> {
    let content = fs::read_to_string("/proc/net/udp6")?;

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

    Ok(udp_rows)
}
pub  fn print_udp_row(row: &UdpRow) {
    println!(
        "Local    : {}",
        format_endpoint(&row.local_ip, row.local_port)
    );

    println!(
        "Remote   : {}",
        format_endpoint(&row.remote_ip, row.remote_port)
    );

    println!("UID      : {}", row.uid);
    println!("RX Queue : {}", row.rx_queue);
    println!("TX Queue : {}", row.tx_queue);
    println!("Inode    : {}", row.inode);
    println!("Drops    : {}", row.drops);
    println!("Owners:");
    for owner in &row.owners {
        println!(
            "  PID: {} | FD: {} | Process: {}",
            owner.pid, owner.fd, owner.process
        );
    }
    println!("------------------------");
}
fn parse_queue(queue: &str) -> (u64, u64) {
    let (tx, rx) = queue.split_once(':').expect("Invalid queue");

    let tx_queue = u64::from_str_radix(tx, 16).expect("Invalid TX queue");

    let rx_queue = u64::from_str_radix(rx, 16).expect("Invalid RX queue");

    (tx_queue, rx_queue)
}