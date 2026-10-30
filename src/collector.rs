use crate::cgroup;
use crate::cpu::CpuMonitor;
use crate::disk::{self, DiskMonitor};
use crate::memory;
use crate::network::{self, NetworkMonitor};
use crate::process;
use crate::socket;
use crate::state;
use crate::tcp;
use crate::udp;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub enum MetricEvent {
    Cpu(f64),
    Memory(memory::MemoryInfo),
    Disk(Vec<disk::DiskRate>),
    Network(Vec<network::NetworkRate>),
    Processes(Vec<process::ProcessInfo>),
    Cgroups(Vec<cgroup::Cgroup>),
    Tcp {
        rows: Vec<tcp::TcpRow>,
        connections: Vec<tcp::TcpConnection>,
    },
    Udp(Vec<udp::UdpRow>),
}

#[derive(Debug, Clone)]
pub struct CollectorIntervals {
    pub cpu: Duration,
    pub memory: Duration,
    pub disk: Duration,
    pub network: Duration,
    pub process: Duration,
    pub cgroup: Duration,
    pub tcp: Duration,
    pub udp: Duration,
}

impl Default for CollectorIntervals {
    fn default() -> Self {
        Self {
            cpu: Duration::from_secs(2),
            memory: Duration::from_secs(2),
            disk: Duration::from_secs(2),
            network: Duration::from_secs(2),
            process: Duration::from_secs(2),
            cgroup: Duration::from_secs(2),
            tcp: Duration::from_secs(2),
            udp: Duration::from_secs(2),
        }
    }
}

pub struct CollectorHandles {
    pub cpu: JoinHandle<()>,
    pub memory: JoinHandle<()>,
    pub disk: JoinHandle<()>,
    pub network: JoinHandle<()>,
    pub process: JoinHandle<()>,
    pub cgroup: JoinHandle<()>,
    pub tcp: JoinHandle<()>,
    pub udp: JoinHandle<()>,
}

impl CollectorHandles {
    pub async fn join_all(self) {
        let _ = self.cpu.await;
        let _ = self.memory.await;
        let _ = self.disk.await;
        let _ = self.network.await;
        let _ = self.process.await;
        let _ = self.cgroup.await;
        let _ = self.tcp.await;
        let _ = self.udp.await;
    }
}

pub async fn update_state_with_event(state: &RwLock<state::RWatchState>, event: MetricEvent) {
    match event {
        MetricEvent::Cpu(usage) => {
            state.write().await.cpu = Some(usage);
        }
        MetricEvent::Memory(memory_info) => {
            state.write().await.memory = Some(memory_info);
        }
        MetricEvent::Disk(rates) => {
            state.write().await.disk = Some(rates);
        }
        MetricEvent::Network(rates) => {
            state.write().await.network = Some(rates);
        }
        MetricEvent::Processes(processes) => {
            state.write().await.processes = Some(processes);
        }
        MetricEvent::Cgroups(cgroups) => {
            state.write().await.cgroups = Some(cgroups);
        }
        MetricEvent::Tcp { rows, connections } => {
            let mut state_guard = state.write().await;
            state_guard.tcp = Some(rows);
            state_guard.tcp_connections = Some(connections);
        }
        MetricEvent::Udp(rows) => {
            state.write().await.udp = Some(rows);
        }
    }
}

pub async fn coordinate_metrics(
    mut receiver: mpsc::Receiver<MetricEvent>,
    state: Arc<RwLock<state::RWatchState>>,
    shutdown: CancellationToken,
) {
    loop {
        tokio::select! {
            Some(event) = receiver.recv() => {
                tracing::debug!("metric event received");
                update_state_with_event(&state, event).await;
            }
            _ = shutdown.cancelled() => {
                tracing::info!("coordinator shutting down");
                while let Ok(event) = receiver.try_recv() {
                    update_state_with_event(&state, event).await;
                }
                break;
            }
            else => {
                break;
            }
        }
    }
}

pub fn spawn_cpu_collector(
    sender: mpsc::Sender<MetricEvent>,
    shutdown: CancellationToken,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!(collector = "cpu", "collector started");
        let mut cpu_monitor = match CpuMonitor::new() {
            Ok(monitor) => monitor,
            Err(error) => {
                tracing::error!(?error, "failed to initialize CPU monitor");
                return;
            }
        };

        let mut ticker = tokio::time::interval(interval);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let start = Instant::now();
                    match cpu_monitor.sample() {
                        Ok(usage) => {
                            let duration = start.elapsed();
                            tracing::debug!(
                                collector = "cpu",
                                usage = usage,
                                duration_us = duration.as_micros(),
                                "sample collected"
                            );
                            if sender.send(MetricEvent::Cpu(usage)).await.is_err() {
                                break;
                            }
                        }
                        Err(error) => {
                            tracing::error!(
                                collector = "cpu",
                                error = ?error,
                                "CPU sample failed"
                            );
                        }
                    }
                }
                _ = shutdown.cancelled() => {
                    tracing::info!(collector = "cpu", "collector shutting down");
                    break;
                }
            }
        }
    })
}

pub fn spawn_memory_collector(
    sender: mpsc::Sender<MetricEvent>,
    shutdown: CancellationToken,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!(collector = "memory", "collector started");
        let mut ticker = tokio::time::interval(interval);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let start = Instant::now();
                    match memory::read_memory_info() {
                        Ok(memory_info) => {
                            let duration = start.elapsed();
                            tracing::debug!(
                                collector = "memory",
                                duration_us = duration.as_micros(),
                                "sample collected"
                            );
                            if sender.send(MetricEvent::Memory(memory_info)).await.is_err() {
                                break;
                            }
                        }
                        Err(error) => {
                            tracing::error!(
                                collector = "memory",
                                error = ?error,
                                "memory sample failed"
                            );
                        }
                    }
                }
                _ = shutdown.cancelled() => {
                    tracing::info!(collector = "memory", "collector shutting down");
                    break;
                }
            }
        }
    })
}

pub fn spawn_disk_collector(
    sender: mpsc::Sender<MetricEvent>,
    shutdown: CancellationToken,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!(collector = "disk", "collector started");
        let mut disk_monitor = DiskMonitor::new();
        let mut ticker = tokio::time::interval(interval);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let start = Instant::now();
                    let rates = disk_monitor.sample();
                    let duration = start.elapsed();
                    tracing::debug!(
                        collector = "disk",
                        duration_us = duration.as_micros(),
                        "sample collected"
                    );
                    if sender.send(MetricEvent::Disk(rates)).await.is_err() {
                        break;
                    }
                }
                _ = shutdown.cancelled() => {
                    tracing::info!(collector = "disk", "collector shutting down");
                    break;
                }
            }
        }
    })
}

pub fn spawn_network_collector(
    sender: mpsc::Sender<MetricEvent>,
    shutdown: CancellationToken,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!(collector = "network", "collector started");
        let mut network_monitor = NetworkMonitor::new();
        let mut ticker = tokio::time::interval(interval);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let start = Instant::now();
                    let rates = network_monitor.sample();
                    let duration = start.elapsed();
                    tracing::debug!(
                        collector = "network",
                        duration_us = duration.as_micros(),
                        "sample collected"
                    );
                    if sender.send(MetricEvent::Network(rates)).await.is_err() {
                        break;
                    }
                }
                _ = shutdown.cancelled() => {
                    tracing::info!(collector = "network", "collector shutting down");
                    break;
                }
            }
        }
    })
}

pub fn spawn_process_collector(
    sender: mpsc::Sender<MetricEvent>,
    shutdown: CancellationToken,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!(collector = "process", "collector started");
        let mut process_monitor = process::ProcessMonitor::new();
        let mut ticker = tokio::time::interval(interval);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let start = Instant::now();
                    let processes = process_monitor.sample();
                    let duration = start.elapsed();
                    tracing::debug!(
                        collector = "process",
                        count = processes.len(),
                        duration_us = duration.as_micros(),
                        "process snapshot collected"
                    );
                    if sender.send(MetricEvent::Processes(processes)).await.is_err() {
                        break;
                    }
                }
                _ = shutdown.cancelled() => {
                    tracing::info!(collector = "process", "collector shutting down");
                    break;
                }
            }
        }
    })
}

pub fn spawn_cgroup_collector(
    sender: mpsc::Sender<MetricEvent>,
    shutdown: CancellationToken,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!(collector = "cgroup", "collector started");
        let mut ticker = tokio::time::interval(interval);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    match cgroup::discover_cgroups() {
                        Ok((cgroups, total, sub)) => {
                            tracing::debug!(
                                collector = "cgroup",
                                count = cgroups.len(),
                                total_cgroups = total,
                                sub_cgroups = sub,
                                "cgroup snapshot collected"
                            );
                            if sender.send(MetricEvent::Cgroups(cgroups)).await.is_err() {
                                break;
                            }
                        }
                        Err(error) => {
                            tracing::error!(
                                collector = "cgroup",
                                error = ?error,
                                "cgroup collection failed"
                            );
                        }
                    }
                }
                _ = shutdown.cancelled() => {
                    tracing::info!(collector = "cgroup", "collector shutting down");
                    break;
                }
            }
        }
    })
}

pub fn spawn_tcp_collector(
    sender: mpsc::Sender<MetricEvent>,
    shutdown: CancellationToken,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!(collector = "tcp", "collector started");
        let mut ticker = tokio::time::interval(interval);
        loop {
            tokio::select! {
                            _ = ticker.tick() => {
                                let owners = socket::build_socket_owners();

            let mut rows = match tcp::read_tcp_rows(&owners) {
                Ok(rows) => rows,
                Err(error) => {
                    tracing::warn!(%error, "failed to read IPv4 TCP connections");
                    Vec::new()
                }
            };

            match tcp::read_tcp6_rows(&owners) {
                Ok(rows6) => rows.extend(rows6),
                Err(error) => {
                    tracing::warn!(%error, "failed to read IPv6 TCP connections");
                }
            }

            let connections = tcp::find_tcp_connections(&rows);

            if sender.send(MetricEvent::Tcp { rows, connections }).await.is_err() {
                break;
            }
                            }
                            _ = shutdown.cancelled() => {
                                tracing::info!(collector = "tcp", "collector shutting down");
                                break;
                            }
                        }
        }
    })
}

pub fn spawn_udp_collector(
    sender: mpsc::Sender<MetricEvent>,
    shutdown: CancellationToken,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!(collector = "udp", "collector started");
        let mut ticker = tokio::time::interval(interval);
        loop {
            tokio::select! {
                            _ = ticker.tick() => {
                                let owners = socket::build_socket_owners();

            let mut rows = match udp::read_udp_rows(&owners) {
                Ok(rows) => rows,
                Err(error) => {
                    tracing::warn!(%error, "failed to read IPv4 UDP sockets");
                    Vec::new()
                }
            };

            match udp::read_udp6_rows(&owners) {
                Ok(rows6) => rows.extend(rows6),
                Err(error) => {
                    tracing::warn!(%error, "failed to read IPv6 UDP sockets");
                }
            }

            if sender.send(MetricEvent::Udp(rows)).await.is_err() {
                break;
            }
                            }
                            _ = shutdown.cancelled() => {
                                tracing::info!(collector = "udp", "collector shutting down");
                                break;
                            }
                        }
        }
    })
}

pub fn spawn_all_collectors(
    sender: mpsc::Sender<MetricEvent>,
    shutdown: CancellationToken,
    intervals: CollectorIntervals,
) -> CollectorHandles {
    CollectorHandles {
        cpu: spawn_cpu_collector(sender.clone(), shutdown.clone(), intervals.cpu),
        memory: spawn_memory_collector(sender.clone(), shutdown.clone(), intervals.memory),
        disk: spawn_disk_collector(sender.clone(), shutdown.clone(), intervals.disk),
        network: spawn_network_collector(sender.clone(), shutdown.clone(), intervals.network),
        process: spawn_process_collector(sender.clone(), shutdown.clone(), intervals.process),
        cgroup: spawn_cgroup_collector(sender.clone(), shutdown.clone(), intervals.cgroup),
        tcp: spawn_tcp_collector(sender.clone(), shutdown.clone(), intervals.tcp),
        udp: spawn_udp_collector(sender, shutdown, intervals.udp),
    }
}
