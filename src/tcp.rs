use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;

use crate::parse::{parse_endpoint, parse_ipv6_endpoint};

use crate::socket::SocketOwner;

#[derive(Debug, Clone, Serialize)]
pub struct TcpRow {
    pub local_ip: String,
    pub local_port: u16,
    pub remote_ip: String,
    pub remote_port: u16,
    pub state: TcpState,
    pub inode: u64,
    pub owners: Vec<crate::socket::SocketOwner>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TcpConnection {
    pub endpoint_a: TcpRow,
    pub endpoint_b: TcpRow,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum TcpState {
    Established,
    SynSent,
    SynRecv,
    FinWait1,
    FinWait2,
    TimeWait,
    Close,
    CloseWait,
    LastAck,
    Listen,
    Closing,
    Unknown,
}
impl fmt::Display for TcpState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = match self {
            TcpState::Established => "ESTABLISHED",
            TcpState::SynSent => "SYN_SENT",
            TcpState::SynRecv => "SYN_RECV",
            TcpState::FinWait1 => "FIN_WAIT1",
            TcpState::FinWait2 => "FIN_WAIT2",
            TcpState::TimeWait => "TIME_WAIT",
            TcpState::Close => "CLOSE",
            TcpState::CloseWait => "CLOSE_WAIT",
            TcpState::LastAck => "LAST_ACK",
            TcpState::Listen => "LISTEN",
            TcpState::Closing => "CLOSING",
            TcpState::Unknown => "UNKNOWN",
        };

        write!(f, "{}", state)
    }
}

pub fn read_tcp_rows(owners: &HashMap<u64, Vec<SocketOwner>>) -> Vec<TcpRow> {
    let content = fs::read_to_string("/proc/net/tcp").expect("Failed to read /proc/net/tcp");

    let mut tcp_rows = Vec::new();

    for line in content.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();

        let local = fields[1];
        let remote = fields[2];

        // Parse local endpoint
        let (local_ip, local_port) = parse_endpoint(local);

        // Parse remote endpoint
        let (remote_ip, remote_port) = parse_endpoint(remote);

        // Parse TCP state
        let state = parse_tcp_state(fields[3]);

        // Get socket inode
        let inode: u64 = fields[9].parse().expect("Invalid inode");

        // Find PID using socket inode
        let socket_owners = match owners.get(&inode) {
            Some(owners) => owners.clone(),
            None => Vec::new(),
        };
        // Create TcpRow
        tcp_rows.push(TcpRow {
            local_ip,
            local_port,
            remote_ip,
            remote_port,
            state,
            inode,
            owners: socket_owners,
        });
    }

    tcp_rows
}
pub fn read_tcp6_rows(owners: &HashMap<u64, Vec<SocketOwner>>) -> Vec<TcpRow> {
    let content = fs::read_to_string("/proc/net/tcp6").expect("Failed to read /proc/net/tcp6");

    let mut tcp6_rows = Vec::new();

    for line in content.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();

        let (local_ip, local_port) = parse_ipv6_endpoint(fields[1]);

        let (remote_ip, remote_port) = parse_ipv6_endpoint(fields[2]);

        let state = parse_tcp_state(fields[3]);

        let inode: u64 = fields[9].parse().expect("Invalid inode");
        let socket_owners = match owners.get(&inode) {
            Some(owners) => owners.clone(),
            None => Vec::new(),
        };

        tcp6_rows.push(TcpRow {
            local_ip,
            local_port,
            remote_ip,
            remote_port,
            state,
            inode,
            owners: socket_owners,
        });
    }

    tcp6_rows
}

pub fn find_tcp_connections(rows: &[TcpRow]) -> Vec<TcpConnection> {
    let mut seen_connections: HashSet<String> = HashSet::new();
    let mut connections: Vec<TcpConnection> = Vec::new();

    for i in 0..rows.len() {
        for j in i + 1..rows.len() {
            if rows[i].state == TcpState::Established
                && rows[j].state == TcpState::Established
                && is_reverse(&rows[i], &rows[j])
            {
                let key = connection_key(
                    &rows[i].local_ip,
                    rows[i].local_port,
                    &rows[i].remote_ip,
                    rows[i].remote_port,
                );

                // Only store the connection once
                if seen_connections.insert(key) {
                    connections.push(TcpConnection {
                        endpoint_a: rows[i].clone(),
                        endpoint_b: rows[j].clone(),
                    });
                }
            }
        }
    }

    connections
}
fn connection_key(local_ip: &str, local_port: u16, remote_ip: &str, remote_port: u16) -> String {
    let first = format!("{}:{}", local_ip, local_port);
    let second = format!("{}:{}", remote_ip, remote_port);

    if first < second {
        format!("{} <-> {}", first, second)
    } else {
        format!("{} <-> {}", second, first)
    }
}

fn is_reverse(a: &TcpRow, b: &TcpRow) -> bool {
    a.local_ip == b.remote_ip
        && a.local_port == b.remote_port
        && a.remote_ip == b.local_ip
        && a.remote_port == b.local_port
}

fn parse_tcp_state(state: &str) -> TcpState {
    match state {
        "01" => TcpState::Established,
        "02" => TcpState::SynSent,
        "03" => TcpState::SynRecv,
        "04" => TcpState::FinWait1,
        "05" => TcpState::FinWait2,
        "06" => TcpState::TimeWait,
        "07" => TcpState::Close,
        "08" => TcpState::CloseWait,
        "09" => TcpState::LastAck,
        "0A" => TcpState::Listen,
        "0B" => TcpState::Closing,
        _ => TcpState::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tcp_states() {
        assert_eq!(parse_tcp_state("01"), TcpState::Established);
        assert_eq!(parse_tcp_state("02"), TcpState::SynSent);
        assert_eq!(parse_tcp_state("0A"), TcpState::Listen);
        assert_eq!(parse_tcp_state("0B"), TcpState::Closing);
    }

    #[test]
    fn unknown_tcp_state() {
        assert_eq!(parse_tcp_state("FF"), TcpState::Unknown);
        assert_eq!(parse_tcp_state("00"), TcpState::Unknown);
    }

    #[test]
    fn parses_all_tcp_states() {
        assert_eq!(parse_tcp_state("01"), TcpState::Established);
        assert_eq!(parse_tcp_state("02"), TcpState::SynSent);
        assert_eq!(parse_tcp_state("03"), TcpState::SynRecv);
        assert_eq!(parse_tcp_state("04"), TcpState::FinWait1);
        assert_eq!(parse_tcp_state("05"), TcpState::FinWait2);
        assert_eq!(parse_tcp_state("06"), TcpState::TimeWait);
        assert_eq!(parse_tcp_state("07"), TcpState::Close);
        assert_eq!(parse_tcp_state("08"), TcpState::CloseWait);
        assert_eq!(parse_tcp_state("09"), TcpState::LastAck);
        assert_eq!(parse_tcp_state("0A"), TcpState::Listen);
        assert_eq!(parse_tcp_state("0B"), TcpState::Closing);
    }

    #[test]
    fn formats_all_tcp_states_display() {
        assert_eq!(TcpState::Established.to_string(), "ESTABLISHED");
        assert_eq!(TcpState::SynSent.to_string(), "SYN_SENT");
        assert_eq!(TcpState::SynRecv.to_string(), "SYN_RECV");
        assert_eq!(TcpState::FinWait1.to_string(), "FIN_WAIT1");
        assert_eq!(TcpState::FinWait2.to_string(), "FIN_WAIT2");
        assert_eq!(TcpState::TimeWait.to_string(), "TIME_WAIT");
        assert_eq!(TcpState::Close.to_string(), "CLOSE");
        assert_eq!(TcpState::CloseWait.to_string(), "CLOSE_WAIT");
        assert_eq!(TcpState::LastAck.to_string(), "LAST_ACK");
        assert_eq!(TcpState::Listen.to_string(), "LISTEN");
        assert_eq!(TcpState::Closing.to_string(), "CLOSING");
        assert_eq!(TcpState::Unknown.to_string(), "UNKNOWN");
    }

    #[test]
    fn connection_key_ordering() {
        let key1 = connection_key("10.0.0.1", 8080, "10.0.0.2", 9090);
        let key2 = connection_key("10.0.0.2", 9090, "10.0.0.1", 8080);
        assert_eq!(key1, key2);
        assert_eq!(key1, "10.0.0.1:8080 <-> 10.0.0.2:9090");
    }

    #[test]
    fn is_reverse_checks() {
        let row_a = TcpRow {
            local_ip: "127.0.0.1".to_string(),
            local_port: 8080,
            remote_ip: "127.0.0.1".to_string(),
            remote_port: 9090,
            state: TcpState::Established,
            inode: 1,
            owners: Vec::new(),
        };

        let row_b = TcpRow {
            local_ip: "127.0.0.1".to_string(),
            local_port: 9090,
            remote_ip: "127.0.0.1".to_string(),
            remote_port: 8080,
            state: TcpState::Established,
            inode: 2,
            owners: Vec::new(),
        };

        let row_mismatched = TcpRow {
            local_ip: "127.0.0.1".to_string(),
            local_port: 9091,
            remote_ip: "127.0.0.1".to_string(),
            remote_port: 8080,
            state: TcpState::Established,
            inode: 3,
            owners: Vec::new(),
        };

        assert!(is_reverse(&row_a, &row_b));
        assert!(is_reverse(&row_b, &row_a));
        assert!(!is_reverse(&row_a, &row_mismatched));
    }

    #[test]
    fn find_tcp_connections_pairs_established_sockets() {
        let row_a = TcpRow {
            local_ip: "10.0.0.1".to_string(),
            local_port: 1234,
            remote_ip: "10.0.0.2".to_string(),
            remote_port: 5678,
            state: TcpState::Established,
            inode: 100,
            owners: Vec::new(),
        };

        let row_b = TcpRow {
            local_ip: "10.0.0.2".to_string(),
            local_port: 5678,
            remote_ip: "10.0.0.1".to_string(),
            remote_port: 1234,
            state: TcpState::Established,
            inode: 101,
            owners: Vec::new(),
        };

        let row_c = TcpRow {
            local_ip: "0.0.0.0".to_string(),
            local_port: 80,
            remote_ip: "0.0.0.0".to_string(),
            remote_port: 0,
            state: TcpState::Listen,
            inode: 102,
            owners: Vec::new(),
        };

        let rows = vec![row_a, row_b, row_c];
        let conns = find_tcp_connections(&rows);

        assert_eq!(conns.len(), 1);
        assert_eq!(conns[0].endpoint_a.inode, 100);
        assert_eq!(conns[0].endpoint_b.inode, 101);
    }
}
