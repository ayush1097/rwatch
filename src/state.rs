use crate::{cgroup, disk, memory, network, process, tcp, udp};
use serde::Serialize;
use std::fs::File;
use std::io::Write;
#[derive(Serialize)]
pub struct RWatchState {
    pub cpu: Option<f64>,
    pub memory: Option<memory::MemoryInfo>,
    pub disk: Option<Vec<disk::DiskRate>>,
    pub network: Option<Vec<network::NetworkRate>>,
    pub processes: Option<Vec<process::ProcessInfo>>,
    pub cgroups: Option<Vec<cgroup::Cgroup>>,
    pub tcp: Option<Vec<tcp::TcpRow>>,
    pub tcp_connections: Option<Vec<tcp::TcpConnection>>,
    pub udp: Option<Vec<udp::UdpRow>>,
}

impl RWatchState {
    pub fn export_json(&self, path: &str) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;

        let mut file = File::create(path)?;
        file.write_all(json.as_bytes())?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_empty_state_to_valid_json() {
        let state = RWatchState {
            cpu: None,
            memory: None,
            disk: None,
            network: None,
            processes: None,
            cgroups: None,
            tcp: None,
            tcp_connections: None,
            udp: None,
        };

        let json = serde_json::to_string(&state).expect("Serialization should succeed");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("Should be valid JSON");
        assert!(parsed["cpu"].is_null());
        assert!(parsed["memory"].is_null());
        assert!(parsed["disk"].is_null());
    }

    #[test]
    fn exports_json_to_file_and_reads_back() {
        let state = RWatchState {
            cpu: Some(42.5),
            memory: None,
            disk: Some(vec![disk::DiskRate {
                device: "sda".to_string(),
                read_rate: 100.0,
                write_rate: 200.0,
                read_iops: 5.0,
                write_iops: 10.0,
            }]),
            network: None,
            processes: None,
            cgroups: None,
            tcp: None,
            tcp_connections: None,
            udp: None,
        };

        let temp_path = std::env::temp_dir().join("test_rwatch_state.json");
        let temp_str = temp_path.to_str().unwrap();

        state
            .export_json(temp_str)
            .expect("export_json should succeed");

        let contents = std::fs::read_to_string(temp_str).expect("file should be readable");
        let parsed: serde_json::Value = serde_json::from_str(&contents).expect("json should parse");
        assert_eq!(parsed["cpu"], 42.5);
        assert_eq!(parsed["disk"][0]["device"], "sda");

        let _ = std::fs::remove_file(temp_path);
    }
}
