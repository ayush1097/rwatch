pub fn parse_endpoint(endpoint: &str) -> Result<(String, u16), &'static str> {
    let (ip, port) = endpoint.split_once(':').ok_or("Invalid endpoint")?;

    let ip_number = u32::from_str_radix(ip, 16).map_err(|_| "Invalid IP")?;

    let bytes = ip_number.to_le_bytes();

    let ip = format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3]);

    let port = u16::from_str_radix(port, 16).map_err(|_| "Invalid port")?;

    Ok((ip, port))
}

pub fn parse_ipv6_endpoint(endpoint: &str) -> Result<(String, u16), &'static str> {
    let (ip_hex, port_hex) = endpoint.split_once(':').ok_or("Invalid IPv6 endpoint")?;

    if ip_hex.len() != 32 {
        return Err("Invalid IPv6 address length");
    }

    let port = u16::from_str_radix(port_hex, 16).map_err(|_| "Invalid port")?;

    let mut bytes = [0u8; 16];

    for (i, byte) in bytes.iter_mut().enumerate() {
        let start = i * 2;
        let end = start + 2;

        *byte = u8::from_str_radix(&ip_hex[start..end], 16).map_err(|_| "Invalid IPv6 address")?;
    }

    // /proc/net/tcp6 and /proc/net/udp6 store each 32-bit
    // address word in little-endian byte order.
    for chunk in bytes.chunks_exact_mut(4) {
        chunk.reverse();
    }

    let ip = std::net::Ipv6Addr::from(bytes);

    Ok((ip.to_string(), port))
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
        let (ip, port) = parse_endpoint("0100007F:1F90").unwrap();
        assert_eq!(ip, "127.0.0.1");
        assert_eq!(port, 8080);
    }

    #[test]
    fn parses_ipv6_endpoint() {
        let (ip, port) = parse_ipv6_endpoint("00000000000000000000000001000000:1F90").unwrap();
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
        let (ip, port) = parse_endpoint("00000000:0000").unwrap();
        assert_eq!(ip, "0.0.0.0");
        assert_eq!(port, 0);

        let (ip_max, port_max) = parse_endpoint("FFFFFFFF:FFFF").unwrap();
        assert_eq!(ip_max, "255.255.255.255");
        assert_eq!(port_max, 65535);
    }

    #[test]
    fn parses_ipv4_endpoint_arbitrary() {
        // C0 A8 01 01 in little-endian order: 0101A8C0 -> 192.168.1.1:80 (hex 0050 = 80)
        let (ip, port) = parse_endpoint("0101A8C0:0050").unwrap();
        assert_eq!(ip, "192.168.1.1");
        assert_eq!(port, 80);
    }

    #[test]
    fn parses_ipv6_endpoint_all_zeros() {
        let (ip, port) = parse_ipv6_endpoint("00000000000000000000000000000000:0016").unwrap();
        assert_eq!(ip, "::");
        assert_eq!(port, 22);
    }

    #[test]
    fn rejects_ipv4_endpoint_without_colon() {
        assert!(parse_endpoint("0100007F").is_err());
    }

    #[test]
    fn rejects_ipv4_endpoint_with_invalid_hex_ip() {
        assert!(parse_endpoint("NOTHEX!!:1F90").is_err());
    }

    #[test]
    fn rejects_ipv4_endpoint_with_invalid_hex_port() {
        assert!(parse_endpoint("0100007F:ZZZZ").is_err());
    }

    #[test]
    fn rejects_ipv6_endpoint_without_colon() {
        assert!(parse_ipv6_endpoint("00000000000000000000000001000000").is_err());
    }

    #[test]
    fn rejects_ipv6_endpoint_with_invalid_length() {
        assert!(parse_ipv6_endpoint("0102:1F90").is_err());
    }

    #[test]
    fn rejects_ipv6_endpoint_with_invalid_hex() {
        assert!(parse_ipv6_endpoint("0000000000000000000000000000000Z:1F90").is_err());
    }
}
