use std::fs;
const SECTOR_SIZE: u64 = 512;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Instant;
#[derive(Debug, Serialize)]
pub struct DiskStats {
    pub device: String,
    pub reads_completed: u64,
    pub sectors_read: u64,
    pub writes_completed: u64,
    pub sectors_written: u64,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DiskRate {
    pub device: String,
    pub read_rate: f64,
    pub write_rate: f64,
    pub read_iops: f64,
    pub write_iops: f64,
}
pub struct DiskMonitor {
    previous: Vec<DiskStats>,
    last_sample: Instant,
}
impl DiskMonitor {
    pub fn new() -> Self {
        Self {
            previous: read_disk_stats(),
            last_sample: Instant::now(),
        }
    }

    pub fn sample(&mut self) -> Vec<DiskRate> {
        let current = read_disk_stats();

        let elapsed = self.last_sample.elapsed().as_secs_f64();

        let rates = calculate_disk_rates(&self.previous, &current, elapsed);

        self.previous = current;

        self.last_sample = Instant::now();

        rates
    }
}
impl Default for DiskMonitor {
    fn default() -> Self {
        Self::new()
    }
}
impl DiskStats {
    pub fn bytes_read(&self) -> u64 {
        self.sectors_read * SECTOR_SIZE
    }

    pub fn bytes_written(&self) -> u64 {
        self.sectors_written * SECTOR_SIZE
    }

    pub fn format_bytes(bytes: u64) -> String {
        // existing code
        const KB: f64 = 1024.0;
        const MB: f64 = KB * 1024.0;
        const GB: f64 = MB * 1024.0;

        let bytes = bytes as f64;

        if bytes >= GB {
            format!("{:.2} GB", bytes / GB)
        } else if bytes >= MB {
            format!("{:.2} MB", bytes / MB)
        } else if bytes >= KB {
            format!("{:.2} KB", bytes / KB)
        } else {
            format!("{} bytes", bytes)
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
            format!("{:.2} bytes/s", bytes_per_second)
        }
    }
}
pub fn read_disk_stats() -> Vec<DiskStats> {
    let content = fs::read_to_string("/proc/diskstats").expect("Failed to read /proc/diskstats");

    let mut disks = Vec::new();

    for line in content.lines() {
        if let Some(disk) = parse_disk_line(line)
            && is_real_disk(&disk.device)
        {
            disks.push(disk);
        }
    }

    disks
}
pub fn parse_disk_line(line: &str) -> Option<DiskStats> {
    let fields: Vec<&str> = line.split_whitespace().collect();

    if fields.len() < 10 {
        return None;
    }

    Some(DiskStats {
        device: fields[2].to_string(),
        reads_completed: fields[3].parse().ok()?,
        sectors_read: fields[5].parse().ok()?,
        writes_completed: fields[7].parse().ok()?,
        sectors_written: fields[9].parse().ok()?,
    })
}
pub fn calculate_disk_rates(
    first: &[DiskStats],
    second: &[DiskStats],
    elapsed: f64,
) -> Vec<DiskRate> {
    let mut second_map = HashMap::new();

    for disk in second {
        second_map.insert(&disk.device, disk);
    }

    let mut rates = Vec::new();

    for old_disk in first {
        if let Some(new_disk) = second_map.get(&old_disk.device) {
            let sectors_read = new_disk.sectors_read.saturating_sub(old_disk.sectors_read);

            let sectors_written = new_disk
                .sectors_written
                .saturating_sub(old_disk.sectors_written);
            let reads_completed = new_disk
                .reads_completed
                .saturating_sub(old_disk.reads_completed);

            let writes_completed = new_disk
                .writes_completed
                .saturating_sub(old_disk.writes_completed);

            let bytes_read = sectors_read * SECTOR_SIZE;
            let bytes_written = sectors_written * SECTOR_SIZE;

            let read_rate = (bytes_read as f64) / elapsed;
            let write_rate = (bytes_written as f64) / elapsed;
            let read_iops = (reads_completed as f64) / elapsed;

            let write_iops = (writes_completed as f64) / elapsed;

            rates.push(DiskRate {
                device: old_disk.device.clone(),
                read_rate,
                write_rate,
                read_iops,
                write_iops,
            });
        }
    }

    rates
}
fn is_real_disk(device: &str) -> bool {
    device.starts_with("sd")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_disk_line() {
        let line = "8 0 sda 100 0 5000 20 200 0 10000 30";

        let result = parse_disk_line(line).unwrap();

        assert_eq!(result.device, "sda");
        assert_eq!(result.reads_completed, 100);
        assert_eq!(result.sectors_read, 5000);
        assert_eq!(result.writes_completed, 200);
        assert_eq!(result.sectors_written, 10000);
    }

    #[test]
    fn rejects_short_disk_line() {
        let line = "8 0 sda 100";

        assert!(parse_disk_line(line).is_none());
    }

    #[test]
    fn rejects_disk_line_with_invalid_numbers() {
        let line = "8 0 sda 100 0 abc 20 200 0 10000 30";
        assert!(parse_disk_line(line).is_none());

        let line2 = "8 0 sda abc 0 5000 20 200 0 10000 30";
        assert!(parse_disk_line(line2).is_none());
    }

    #[test]
    fn is_real_disk_filters_correctly() {
        assert!(is_real_disk("sda"));
        assert!(is_real_disk("sdb1"));
        assert!(!is_real_disk("nvme0n1"));
        assert!(!is_real_disk("loop0"));
        assert!(!is_real_disk("dm-0"));
        assert!(!is_real_disk("ram0"));
    }

    #[test]
    fn calculates_bytes_read_and_written() {
        let stats = DiskStats {
            device: "sda".to_string(),
            reads_completed: 10,
            sectors_read: 20,
            writes_completed: 30,
            sectors_written: 40,
        };
        assert_eq!(stats.bytes_read(), 20 * SECTOR_SIZE);
        assert_eq!(stats.bytes_written(), 40 * SECTOR_SIZE);
    }

    #[test]
    fn formats_bytes_correctly() {
        assert_eq!(DiskStats::format_bytes(500), "500 bytes");
        assert_eq!(DiskStats::format_bytes(1024), "1.00 KB");
        assert_eq!(DiskStats::format_bytes(2048), "2.00 KB");
        assert_eq!(DiskStats::format_bytes(1024 * 1024 * 5), "5.00 MB");
        assert_eq!(DiskStats::format_bytes(1024 * 1024 * 1024 * 2), "2.00 GB");
    }

    #[test]
    fn formats_rate_correctly() {
        assert_eq!(DiskStats::format_rate(500.0), "500.00 bytes/s");
        assert_eq!(DiskStats::format_rate(1024.0), "1.00 KB/s");
        assert_eq!(DiskStats::format_rate(1024.0 * 1024.0 * 2.5), "2.50 MB/s");
        assert_eq!(
            DiskStats::format_rate(1024.0 * 1024.0 * 1024.0 * 3.0),
            "3.00 GB/s"
        );
    }

    #[test]
    fn calculates_disk_rates_normal() {
        let first = vec![DiskStats {
            device: "sda".to_string(),
            reads_completed: 100,
            sectors_read: 2000,
            writes_completed: 50,
            sectors_written: 4000,
        }];

        let second = vec![DiskStats {
            device: "sda".to_string(),
            reads_completed: 120,
            sectors_read: 2400,
            writes_completed: 60,
            sectors_written: 4800,
        }];

        let rates = calculate_disk_rates(&first, &second, 2.0);
        assert_eq!(rates.len(), 1);
        let rate = &rates[0];
        assert_eq!(rate.device, "sda");
        assert_eq!(rate.read_iops, 10.0); // (120 - 100) / 2.0
        assert_eq!(rate.write_iops, 5.0); // (60 - 50) / 2.0
        assert_eq!(rate.read_rate, 400.0 * 512.0 / 2.0);
        assert_eq!(rate.write_rate, 800.0 * 512.0 / 2.0);
    }

    #[test]
    fn calculates_disk_rates_saturating_and_missing_devices() {
        let first = vec![
            DiskStats {
                device: "sda".to_string(),
                reads_completed: 500,
                sectors_read: 5000,
                writes_completed: 500,
                sectors_written: 5000,
            },
            DiskStats {
                device: "sdb".to_string(),
                reads_completed: 10,
                sectors_read: 100,
                writes_completed: 10,
                sectors_written: 100,
            },
        ];

        // Second only has "sda", with smaller numbers (e.g. counter reset)
        let second = vec![DiskStats {
            device: "sda".to_string(),
            reads_completed: 200,
            sectors_read: 2000,
            writes_completed: 200,
            sectors_written: 2000,
        }];

        let rates = calculate_disk_rates(&first, &second, 1.0);
        assert_eq!(rates.len(), 1);
        assert_eq!(rates[0].device, "sda");
        assert_eq!(rates[0].read_rate, 0.0);
        assert_eq!(rates[0].write_rate, 0.0);
        assert_eq!(rates[0].read_iops, 0.0);
        assert_eq!(rates[0].write_iops, 0.0);
    }
}
