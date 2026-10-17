use std::fmt;
use std::fs;

use serde::Serialize;
#[derive(Debug)]
pub enum MemoryError {
    Io(std::io::Error),

    MissingField(&'static str),

    InvalidValue { key: String, value: String },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MemoryInfo {
    pub mem_total: u64,
    pub mem_free: u64,
    pub mem_available: u64,
}

impl MemoryInfo {
    fn kb_to_gb(kb: u64) -> f64 {
        kb as f64 / 1024.0 / 1024.0
    }

    pub fn total_gb(&self) -> f64 {
        Self::kb_to_gb(self.mem_total)
    }

    pub fn free_gb(&self) -> f64 {
        Self::kb_to_gb(self.mem_free)
    }

    pub fn available_gb(&self) -> f64 {
        Self::kb_to_gb(self.mem_available)
    }

    pub fn used_memory(&self) -> u64 {
        self.mem_total - self.mem_available
    }

    pub fn used_gb(&self) -> f64 {
        Self::kb_to_gb(self.used_memory())
    }
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryError::Io(error) => {
                write!(f, "I/O error: {}", error)
            }

            MemoryError::MissingField(field) => {
                write!(f, "Missing required field: {}", field)
            }

            MemoryError::InvalidValue { key, value } => {
                write!(f, "Invalid value for {}: {}", key, value)
            }
        }
    }
}

fn parse_memory_info(contents: &str) -> Result<MemoryInfo, MemoryError> {
    let mut mem_total: Option<u64> = None;
    let mut mem_free: Option<u64> = None;
    let mut mem_available: Option<u64> = None;

    for line in contents.lines() {
        let mut parts = line.split_whitespace();

        let key = match parts.next() {
            Some(key) => key,
            None => continue,
        };

        let value_str = match parts.next() {
            Some(value) => value,
            None => continue,
        };

        let value: u64 = value_str.parse().map_err(|_| MemoryError::InvalidValue {
            key: key.to_string(),
            value: value_str.to_string(),
        })?;

        match key {
            "MemTotal:" => mem_total = Some(value),
            "MemFree:" => mem_free = Some(value),
            "MemAvailable:" => mem_available = Some(value),
            _ => {}
        }
    }

    let mem_total = mem_total.ok_or(MemoryError::MissingField("MemTotal"))?;

    let mem_free = mem_free.ok_or(MemoryError::MissingField("MemFree"))?;

    let mem_available = mem_available.ok_or(MemoryError::MissingField("MemAvailable"))?;

    Ok(MemoryInfo {
        mem_total,
        mem_free,
        mem_available,
    })
}

pub fn read_memory_info() -> Result<MemoryInfo, MemoryError> {
    let contents = fs::read_to_string("/proc/meminfo").map_err(MemoryError::Io)?;

    parse_memory_info(&contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_memory_info() {
        let input = "\
MemTotal:       1000 kB
MemFree:         400 kB
MemAvailable:    600 kB
";

        let memory_info = parse_memory_info(input).unwrap();

        assert_eq!(memory_info.mem_total, 1000);
        assert_eq!(memory_info.mem_free, 400);
        assert_eq!(memory_info.mem_available, 600);
        assert_eq!(memory_info.used_memory(), 400);
    }

    #[test]
    fn fails_when_mem_available_is_missing() {
        let input = "\
MemTotal:       1000 kB
MemFree:         400 kB
";

        let result = parse_memory_info(input);

        assert!(matches!(
            result,
            Err(MemoryError::MissingField("MemAvailable"))
        ));
    }

    #[test]
    fn fails_when_memory_value_is_not_a_number() {
        let input = "\
MemTotal:       abc kB
MemFree:        400 kB
MemAvailable:   600 kB
";

        let result = parse_memory_info(input);

        assert!(matches!(result, Err(MemoryError::InvalidValue { .. })));
    }

    #[test]
    fn fails_when_mem_total_is_missing() {
        let input = "\
MemFree:         400 kB
MemAvailable:    600 kB
";
        let result = parse_memory_info(input);
        assert!(matches!(result, Err(MemoryError::MissingField("MemTotal"))));
    }

    #[test]
    fn fails_when_mem_free_is_missing() {
        let input = "\
MemTotal:       1000 kB
MemAvailable:    600 kB
";
        let result = parse_memory_info(input);
        assert!(matches!(result, Err(MemoryError::MissingField("MemFree"))));
    }

    #[test]
    fn fails_when_empty_meminfo() {
        let input = "";
        let result = parse_memory_info(input);
        assert!(matches!(result, Err(MemoryError::MissingField(_))));
    }

    #[test]
    fn parses_memory_info_ignoring_irrelevant_fields() {
        let input = "\
MemTotal:       16777216 kB
MemFree:         8388608 kB
MemAvailable:   12582912 kB
Buffers:          500000 kB
Cached:          2000000 kB
SwapCached:            0 kB
Active:          4000000 kB
";
        let mem = parse_memory_info(input).expect("Should parse with extra fields");
        assert_eq!(mem.mem_total, 16777216);
        assert_eq!(mem.mem_free, 8388608);
        assert_eq!(mem.mem_available, 12582912);
        assert_eq!(mem.used_memory(), 16777216 - 12582912);
    }

    #[test]
    fn calculates_memory_in_gb() {
        let mem = MemoryInfo {
            mem_total: 1024 * 1024 * 16,     // 16 GB in kB
            mem_free: 1024 * 1024 * 4,       // 4 GB in kB
            mem_available: 1024 * 1024 * 12, // 12 GB in kB
        };
        assert_eq!(mem.total_gb(), 16.0);
        assert_eq!(mem.free_gb(), 4.0);
        assert_eq!(mem.available_gb(), 12.0);
        assert_eq!(mem.used_gb(), 4.0);
    }

    #[test]
    fn formats_memory_error_display() {
        let missing = MemoryError::MissingField("MemTotal");
        assert_eq!(format!("{}", missing), "Missing required field: MemTotal");

        let invalid = MemoryError::InvalidValue {
            key: "MemTotal".to_string(),
            value: "xyz".to_string(),
        };
        assert_eq!(format!("{}", invalid), "Invalid value for MemTotal: xyz");

        let io_err = MemoryError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert!(format!("{}", io_err).contains("I/O error"));
    }
}
