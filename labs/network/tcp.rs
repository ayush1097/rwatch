use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;

use crate::parse::{
    format_endpoint,
    parse_endpoint,
    parse_ipv6_endpoint,
};

use crate::socket_owner::SocketOwner;

#[derive(Debug, Clone)]
pub struct TcpRow {
    local_ip: String,
    local_port: u16,
    remote_ip: String,
    remote_port: u16,
    pub state: TcpState,
    inode: u64,
    owners: Vec<SocketOwner>,
}
#[derive(Debug, Clone)]
pub struct TcpConnection {
    endpoint_a: TcpRow,
    endpoint_b: TcpRow,
}

#[derive(Debug, Clone, PartialEq)]
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
//=========================================
// read tcp6 rows
//=========================================
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
pub fn print_tcp_row(row: &TcpRow) {
    println!("=== TCP SOCKET ===");
    println!(
        "Local  : {}",
        format_endpoint(&row.local_ip, row.local_port)
    );

    println!(
        "Remote : {}",
        format_endpoint(&row.remote_ip, row.remote_port)
    );

    println!("State  : {}", row.state);
    println!("Inode  : {}", row.inode);
    println!("Owners:");
    for owner in &row.owners {
        println!(
            "  PID: {} | FD: {} | Process: {}",
            owner.pid, owner.fd, owner.process
        );
    }

    println!("------------------------");
}
pub fn print_tcp_connection(connection: &TcpConnection) {
    println!("=== TCP CONNECTION ===");

    println!(
        "{}:{} -> {}:{}",
        connection.endpoint_a.local_ip,
        connection.endpoint_a.local_port,
        connection.endpoint_a.remote_ip,
        connection.endpoint_a.remote_port
    );

    println!("Owners:");
    for owner in &connection.endpoint_a.owners {
        println!(
            "  PID: {} | FD: {} | Process: {}",
            owner.pid, owner.fd, owner.process
        );
    }
    println!("Inode   : {}", connection.endpoint_a.inode);

    println!("        <------>");

    println!(
        "{}:{} -> {}:{}",
        connection.endpoint_b.local_ip,
        connection.endpoint_b.local_port,
        connection.endpoint_b.remote_ip,
        connection.endpoint_b.remote_port
    );

    println!("Owners:");
    for owner in &connection.endpoint_b.owners {
        println!(
            "  PID: {} | FD: {} | Process: {}",
            owner.pid, owner.fd, owner.process
        );
    }
    println!("Inode   : {}", connection.endpoint_b.inode);

    println!("------------------------");
}
pub fn print_tcp_state_summary(rows: &[TcpRow]) {
    let mut established = 0;
    let mut syn_sent = 0;
    let mut syn_recv = 0;
    let mut fin_wait1 = 0;
    let mut fin_wait2 = 0;
    let mut time_wait = 0;
    let mut close = 0;
    let mut close_wait = 0;
    let mut last_ack = 0;
    let mut listen = 0;
    let mut closing = 0;
    let mut unknown = 0;

    for row in rows {
        match row.state {
            TcpState::Established => {
                established += 1;
            }
            TcpState::SynSent => {
                syn_sent += 1;
            }
            TcpState::SynRecv => {
                syn_recv += 1;
            }
            TcpState::FinWait1 => {
                fin_wait1 += 1;
            }
            TcpState::FinWait2 => {
                fin_wait2 += 1;
            }
            TcpState::TimeWait => {
                time_wait += 1;
            }
            TcpState::Close => {
                close += 1;
            }
            TcpState::CloseWait => {
                close_wait += 1;
            }
            TcpState::LastAck => {
                last_ack += 1;
            }
            TcpState::Listen => {
                listen += 1;
            }
            TcpState::Closing => {
                closing += 1;
            }
            TcpState::Unknown => {
                unknown += 1;
            }
        }
    }

    println!("=== TCP STATE SUMMARY ===");

    println!("ESTABLISHED : {}", established);
    println!("SYN_SENT    : {}", syn_sent);
    println!("SYN_RECV    : {}", syn_recv);
    println!("FIN_WAIT1   : {}", fin_wait1);
    println!("FIN_WAIT2   : {}", fin_wait2);
    println!("TIME_WAIT   : {}", time_wait);
    println!("CLOSE       : {}", close);
    println!("CLOSE_WAIT  : {}", close_wait);
    println!("LAST_ACK    : {}", last_ack);
    println!("LISTEN      : {}", listen);
    println!("CLOSING     : {}", closing);
    println!("UNKNOWN     : {}", unknown);

    println!("------------------------");
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
