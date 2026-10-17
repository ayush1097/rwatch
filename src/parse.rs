pub fn parse_endpoint(endpoint: &str) -> (String, u16) {
    let (ip, port) = endpoint.split_once(':').expect("Invalid endpoint");

    let ip_number = u32::from_str_radix(ip, 16).expect("Invalid IP");

    let bytes = ip_number.to_le_bytes();

    let ip = format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3]);

    let port = u16::from_str_radix(port, 16).expect("Invalid port");

    (ip, port)
}
pub fn parse_ipv6_endpoint(endpoint: &str) -> (String, u16) {
    let (ip_hex, port_hex) = endpoint.split_once(':').expect("Invalid IPv6 endpoint");

    let port = u16::from_str_radix(port_hex, 16).expect("Invalid port");

    let mut bytes = [0u8; 16];

    for i in 0..16 {
        let byte = u8::from_str_radix(&ip_hex[i * 2..i * 2 + 2], 16).expect("Invalid IPv6 address");

        bytes[i] = byte;
    }

    // /proc/net/udp6 stores each 32-bit word in
    // little-endian byte order.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ipv4_endpoint() {
        let (ip, port) = parse_endpoint("0100007F:1F90");

        assert_eq!(ip, "127.0.0.1");
        assert_eq!(port, 8080);
    }

    #[test]
    fn parses_ipv6_endpoint() {
        let (ip, port) = parse_ipv6_endpoint("00000000000000000000000001000000:1F90");

        assert_eq!(ip, "::1");
        assert_eq!(port, 8080);
    }

    #[test]
    fn formats_ipv4_endpoint() {
        assert_eq!(format_endpoint("127.0.0.1", 8080), "127.0.0.1:8080");
        assert_eq!(format_endpoint("0.0.0.0", 80), "0.0.0.0:80");
        assert_eq!(
            format_endpoint("192.168.1.100", 65535),
            "192.168.1.100:65535"
        );
    }

    #[test]
    fn formats_ipv6_endpoint() {
        assert_eq!(format_endpoint("::1", 8080), "[::1]:8080");
        assert_eq!(format_endpoint("fe80::1", 443), "[fe80::1]:443");
        assert_eq!(
            format_endpoint("2001:db8::ff00:42:8329", 53),
            "[2001:db8::ff00:42:8329]:53"
        );
    }

    #[test]
    fn parses_ipv4_endpoint_zeros_and_max() {
        let (ip, port) = parse_endpoint("00000000:0000");
        assert_eq!(ip, "0.0.0.0");
        assert_eq!(port, 0);

        let (ip_max, port_max) = parse_endpoint("FFFFFFFF:FFFF");
        assert_eq!(ip_max, "255.255.255.255");
        assert_eq!(port_max, 65535);
    }

    #[test]
    fn parses_ipv4_endpoint_arbitrary() {
        // C0 A8 01 01 in little-endian order: 0101A8C0 -> 192.168.1.1:80 (hex 0050 = 80)
        let (ip, port) = parse_endpoint("0101A8C0:0050");
        assert_eq!(ip, "192.168.1.1");
        assert_eq!(port, 80);
    }

    #[test]
    fn parses_ipv6_endpoint_all_zeros() {
        let (ip, port) = parse_ipv6_endpoint("00000000000000000000000000000000:0016");
        assert_eq!(ip, "::");
        assert_eq!(port, 22);
    }

    #[test]
    #[should_panic(expected = "Invalid endpoint")]
    fn parse_endpoint_panics_on_missing_colon() {
        parse_endpoint("0100007F");
    }

    #[test]
    #[should_panic(expected = "Invalid IP")]
    fn parse_endpoint_panics_on_invalid_hex_ip() {
        parse_endpoint("NOTHEX!!:1F90");
    }

    #[test]
    #[should_panic(expected = "Invalid port")]
    fn parse_endpoint_panics_on_invalid_hex_port() {
        parse_endpoint("0100007F:ZZZZ");
    }

    #[test]
    #[should_panic(expected = "Invalid IPv6 endpoint")]
    fn parse_ipv6_endpoint_panics_on_missing_colon() {
        parse_ipv6_endpoint("00000000000000000000000001000000");
    }
}
