pub fn parse_endpoint(endpoint: &str) -> (String, u16) {
    let (ip, port) = endpoint.split_once(':').expect("Invalid endpoint");

    let ip_number = u32::from_str_radix(ip, 16).expect("Invalid IP");

    let bytes = ip_number.to_le_bytes();

    let ip = format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3]);

    let port = u16::from_str_radix(port, 16).expect("Invalid port");

    (ip, port)
}

pub fn parse_ipv6_endpoint(endpoint: &str) -> (String, u16) {
    let (ip_hex, port_hex) =
        endpoint.split_once(':').expect("Invalid IPv6 endpoint");

    let port =
        u16::from_str_radix(port_hex, 16).expect("Invalid port");

    let mut bytes = [0u8; 16];

    for i in 0..16 {
        let byte =
            u8::from_str_radix(
                &ip_hex[i * 2..i * 2 + 2],
                16,
            )
            .expect("Invalid IPv6 address");

        bytes[i] = byte;
    }

    for chunk in bytes.chunks_exact_mut(4) {
        chunk.reverse();
    }

    let ip = std::net::Ipv6Addr::from(bytes);

    (ip.to_string(), port)
}

pub fn format_endpoint(ip: &str, port: u16) -> String {
    if ip.contains(':') {
        format!("[{}]:{}", ip, port)
    } else {
        format!("{}:{}", ip, port)
    }
}