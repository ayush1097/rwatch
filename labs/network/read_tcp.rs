mod socket_owner;
mod udp;
mod tcp;
mod parse;
fn main() {
    let socket_owners = socket_owner::build_socket_owners();

    // ==========================================
    // STEP 1: READ ALL TCP & UDP ROWS
    // ==========================================

    let tcp_rows = tcp::read_tcp_rows(&socket_owners);
    let udp_rows = udp::read_udp_rows(&socket_owners);

    println!("=== UDP SOCKETS ===");

    for row in &udp_rows {
        udp::print_udp_row(row);
    }

    let udp6_rows = udp::read_udp6_rows(&socket_owners);

    println!("=== UDP6 SOCKETS ===");

    for row in &udp6_rows {
        udp::print_udp_row(row);
    }
    // ==========================================
    // STEP 2: PRINT SPECIAL TCP SOCKETS
    // ==========================================
    tcp::print_tcp_state_summary(&tcp_rows);
    for row in &tcp_rows {
        match row.state {
            | tcp::TcpState::Listen
            | tcp::TcpState::TimeWait
            | tcp::TcpState::CloseWait
            | tcp::TcpState::FinWait1
            | tcp::TcpState::FinWait2 => {
                tcp::print_tcp_row(row);
            }
            _ => {}
        }
    }

    // ==========================================
    // STEP 3: FIND ESTABLISHED CONNECTIONS
    // ==========================================

    let connections = tcp::find_tcp_connections(&tcp_rows);

    // ==========================================
    // STEP 4: PRINT TCP CONNECTIONS
    // ==========================================

    for connection in &connections {
        tcp::print_tcp_connection(connection);
    }

    let tcp6_rows = tcp::read_tcp6_rows(&socket_owners);

    println!("=== TCP6 SOCKETS ===");

    for row in &tcp6_rows {
        tcp::print_tcp_row(row);
    }
}
