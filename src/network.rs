use serde::Serialize;
use std::fs;
use std::time::Instant;
#[derive(Debug, Serialize)]
pub struct NetworkStats {
    pub interface: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NetworkRate {
    pub interface: String,
    pub download_rate: f64,
    pub upload_rate: f64,
}

pub struct NetworkMonitor {
    previous: Vec<NetworkStats>,
    last_sample: Instant,
}
impl NetworkMonitor {
    pub fn new() -> Self {
        Self {
            previous: read_network_stats(),
            last_sample: Instant::now(),
        }
    }
    pub fn sample(&mut self) -> Vec<NetworkRate> {
        let current = read_network_stats();

        let elapsed = self.last_sample.elapsed().as_secs_f64();

        let rates = calculate_network_rates(&self.previous, &current, elapsed);

        self.previous = current;

        self.last_sample = Instant::now();

        rates
    }
}
impl Default for NetworkMonitor {
    fn default() -> Self {
        Self::new()
    }
}
pub fn format_rate(bytes_per_second: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    if bytes_per_second >= GB {
        format!("{:.2} GB/s", bytes_per_second / GB)
    } else if bytes_per_second >= MB {
        format!("{:.2} MB/s", bytes_per_second / MB)
    } else if bytes_per_second >= KB {
        format!("{:.2} KB/s", bytes_per_second / KB)
    } else {
        format!("{:.2} B/s", bytes_per_second)
    }
}
pub fn read_network_stats() -> Vec<NetworkStats> {
    let content = fs::read_to_string("/proc/net/dev").expect("Failed to read /proc/net/dev");

    let mut networks = Vec::new();

    for line in content.lines().skip(2) {
        if let Some(network) = parse_network_line(line) {
            networks.push(network);
        }
    }

    networks
}

pub fn parse_network_line(line: &str) -> Option<NetworkStats> {
    let (interface, stats) = line.split_once(':')?;

    let fields: Vec<&str> = stats.split_whitespace().collect();

    if fields.len() < 9 {
        return None;
    }

    Some(NetworkStats {
        interface: interface.trim().to_string(),
        rx_bytes: fields[0].parse().ok()?,
        tx_bytes: fields[8].parse().ok()?,
    })
}

pub fn calculate_network_rates(
    first: &[NetworkStats],
    second: &[NetworkStats],
    elapsed: f64,
) -> Vec<NetworkRate> {
    let mut rates = Vec::new();

    for old_network in first {
        if old_network.interface == "lo" {
            continue;
        }

        let new_network = second
            .iter()
            .find(|new_network| new_network.interface == old_network.interface);

        if let Some(new_network) = new_network {
            let rx_difference = new_network.rx_bytes.saturating_sub(old_network.rx_bytes);

            let tx_difference = new_network.tx_bytes.saturating_sub(old_network.tx_bytes);

            let download_rate = rx_difference as f64 / elapsed;

            let upload_rate = tx_difference as f64 / elapsed;

            rates.push(NetworkRate {
                interface: old_network.interface.clone(),
                download_rate,
                upload_rate,
            });
        }
    }

    rates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_network_line() {
        let line = "eth0: 1000 10 20 30 40 50 60 70 2000 11 22 33 44 55 66 77";

        let result = parse_network_line(line).unwrap();

        assert_eq!(result.interface, "eth0");
        assert_eq!(result.rx_bytes, 1000);
        assert_eq!(result.tx_bytes, 2000);
    }

    #[test]
    fn rejects_invalid_network_line() {
        let line = "eth0: 1000 10";

        assert!(parse_network_line(line).is_none());
    }

    #[test]
    fn rejects_network_line_without_colon() {
        let line = "eth0 1000 10 20 30 40 50 60 70 2000";
        assert!(parse_network_line(line).is_none());
    }

    #[test]
    fn rejects_network_line_with_invalid_numbers() {
        let line_bad_rx = "eth0: abc 10 20 30 40 50 60 70 2000";
        assert!(parse_network_line(line_bad_rx).is_none());

        let line_bad_tx = "eth0: 1000 10 20 30 40 50 60 70 xyz";
        assert!(parse_network_line(line_bad_tx).is_none());
    }

    #[test]
    fn parses_network_line_with_surrounding_whitespace() {
        let line = "  wlan0:  500 10 20 30 40 50 60 70 600 11 22 33 44 55 66 77 ";
        let result = parse_network_line(line).unwrap();
        assert_eq!(result.interface, "wlan0");
        assert_eq!(result.rx_bytes, 500);
        assert_eq!(result.tx_bytes, 600);
    }

    #[test]
    fn formats_network_rate() {
        assert_eq!(format_rate(100.0), "100.00 B/s");
        assert_eq!(format_rate(1024.0), "1.00 KB/s");
        assert_eq!(format_rate(1024.0 * 1024.0 * 5.0), "5.00 MB/s");
        assert_eq!(format_rate(1024.0 * 1024.0 * 1024.0 * 1.5), "1.50 GB/s");
    }

    #[test]
    fn calculates_network_rates_normal() {
        let first = vec![NetworkStats {
            interface: "eth0".to_string(),
            rx_bytes: 1000,
            tx_bytes: 2000,
        }];

        let second = vec![NetworkStats {
            interface: "eth0".to_string(),
            rx_bytes: 3000,
            tx_bytes: 4000,
        }];

        let rates = calculate_network_rates(&first, &second, 2.0);
        assert_eq!(rates.len(), 1);
        assert_eq!(rates[0].interface, "eth0");
        assert_eq!(rates[0].download_rate, 1000.0);
        assert_eq!(rates[0].upload_rate, 1000.0);
    }

    #[test]
    fn calculates_network_rates_ignores_lo_and_handles_missing() {
        let first = vec![
            NetworkStats {
                interface: "lo".to_string(),
                rx_bytes: 5000,
                tx_bytes: 5000,
            },
            NetworkStats {
                interface: "eth1".to_string(),
                rx_bytes: 2000,
                tx_bytes: 2000,
            },
        ];

        let second = vec![
            NetworkStats {
                interface: "lo".to_string(),
                rx_bytes: 10000,
                tx_bytes: 10000,
            },
            // eth1 is missing from second
        ];

        let rates = calculate_network_rates(&first, &second, 1.0);
        assert_eq!(rates.len(), 0);
    }

    #[test]
    fn calculates_network_rates_saturating_sub() {
        let first = vec![NetworkStats {
            interface: "eth0".to_string(),
            rx_bytes: 5000,
            tx_bytes: 5000,
        }];

        let second = vec![NetworkStats {
            interface: "eth0".to_string(),
            rx_bytes: 1000,
            tx_bytes: 2000,
        }];

        let rates = calculate_network_rates(&first, &second, 1.0);
        assert_eq!(rates.len(), 1);
        assert_eq!(rates[0].download_rate, 0.0);
        assert_eq!(rates[0].upload_rate, 0.0);
    }
}
