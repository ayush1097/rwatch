#![allow(dead_code)]

mod api;
mod cgroup;
mod config;
mod cpu;
mod disk;
mod limits;
mod memory;
mod namespace;
mod network;
mod parse;
mod process;
mod signals;
mod socket;
mod state;
mod tcp;
mod udp;
mod ui;
use cpu::CpuMonitor;
use disk::DiskMonitor;
use network::NetworkMonitor;

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
enum MetricEvent {
    Cpu(f64),
    Memory(memory::MemoryInfo),
    Disk(Vec<disk::DiskRate>),
    Network(Vec<network::NetworkRate>),
    Processes(Vec<process::ProcessInfo>),

    ProcessStarted {
        pid: u32,
        name: String,
    },
    ProcessExited {
        pid: u32,
        name: String,
    },

    Cgroups(Vec<cgroup::Cgroup>),

    Tcp {
        rows: Vec<tcp::TcpRow>,
        connections: Vec<tcp::TcpConnection>,
    },

    Udp(Vec<udp::UdpRow>),
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "RWatch - Linux system monitoring tool

USAGE:
    rwatch [OPTIONS]
    rwatch snapshot --json

OPTIONS:
    -h, --help       Print help information
    -V, --version    Print version information

COMMANDS:
    snapshot --json  Collect a system snapshot and output JSON

Run without arguments to start the interactive TUI."
        );
        return;
    }

    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("rwatch {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if args.len() > 1 && !(args.len() == 3 && args[1] == "snapshot" && args[2] == "--json") {
        eprintln!("Error: invalid command or arguments.\n");
        eprintln!("Usage:");
        eprintln!("  rwatch");
        eprintln!("  rwatch snapshot --json");
        eprintln!("  rwatch --help");
        eprintln!("  rwatch --version");
        std::process::exit(2);
    }

    let config = match config::Config::load() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Error: failed to load rwatch.toml: {error}");
            std::process::exit(1);
        }
    };

    let process_interval = match config.process_duration() {
        Ok(duration) => duration,
        Err(error) => {
            eprintln!("Error: invalid process_interval in rwatch.toml: {error}");
            std::process::exit(1);
        }
    };

    let network_interval = match config.network_duration() {
        Ok(duration) => duration,
        Err(error) => {
            eprintln!("Error: invalid network_interval in rwatch.toml: {error}");
            std::process::exit(1);
        }
    };

    let refresh_interval = match config.refresh_duration() {
        Ok(duration) => duration,
        Err(error) => {
            eprintln!("Error: invalid refresh_interval in rwatch.toml: {error}");
            std::process::exit(1);
        }
    };

    if args.len() >= 3 && args[1] == "snapshot" && args[2] == "--json" {
        let memory = match memory::read_memory_info() {
            Ok(info) => Some(info),
            Err(error) => {
                eprintln!("Warning: failed to collect memory information: {error}");
                None
            }
        };
        let cpu = match CpuMonitor::new() {
            Ok(mut monitor) => match monitor.sample() {
                Ok(usage) => Some(usage),
                Err(error) => {
                    eprintln!("Warning: failed to collect CPU usage: {error:?}");
                    None
                }
            },
            Err(error) => {
                eprintln!("Warning: failed to initialize CPU monitor: {error:?}");
                None
            }
        };
        let mut disk_monitor = DiskMonitor::new();
        let disk = disk_monitor.sample();

        let mut network_monitor = NetworkMonitor::new();
        let network = network_monitor.sample();

        let mut process_monitor = process::ProcessMonitor::new();
        let processes = process_monitor.sample();

        let cgroups = match cgroup::discover_cgroups() {
            Ok((cgroups, _, _)) => cgroups,
            Err(error) => {
                eprintln!("Warning: failed to discover cgroups: {error:?}");
                Vec::new()
            }
        };

        let owners = socket::build_socket_owners();

        let mut tcp_rows = match tcp::read_tcp_rows(&owners) {
            Ok(rows) => rows,
            Err(error) => {
                eprintln!("Warning: failed to read TCP connections: {error}");
                Vec::new()
            }
        };

        match tcp::read_tcp6_rows(&owners) {
            Ok(rows) => tcp_rows.extend(rows),
            Err(error) => {
                eprintln!("Warning: failed to read IPv6 TCP connections: {error}");
            }
        }

        let tcp_connections = tcp::find_tcp_connections(&tcp_rows);

        let mut udp_rows = match udp::read_udp_rows(&owners) {
            Ok(rows) => rows,
            Err(error) => {
                eprintln!("Warning: failed to read IPv4 UDP sockets: {error}");
                Vec::new()
            }
        };

        match udp::read_udp6_rows(&owners) {
            Ok(rows) => udp_rows.extend(rows),
            Err(error) => {
                eprintln!("Warning: failed to read IPv6 UDP sockets: {error}");
            }
        }

        let state = state::RWatchState {
            cpu,
            memory,
            disk: Some(disk),
            network: Some(network),
            processes: Some(processes),
            cgroups: Some(cgroups),
            tcp: Some(tcp_rows),
            tcp_connections: Some(tcp_connections),
            udp: Some(udp_rows),
        };

        println!("{}", serde_json::to_string_pretty(&state).unwrap());
        return;
    }

    let (sender, mut receiver) = mpsc::channel(100);
    let (critical_sender, mut critical_receiver) = mpsc::channel::<MetricEvent>(32);

    let shutdown = CancellationToken::new();

    let shutdown_signal = shutdown.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for Ctrl+C");

        shutdown_signal.cancel();
    });
    let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| config.log_level.clone());

    tracing_subscriber::fmt().with_env_filter(log_level).init();

    let state = Arc::new(RwLock::new(state::RWatchState {
        cpu: None,
        memory: None,
        disk: None,
        network: None,
        processes: None,
        cgroups: None,
        tcp: None,
        tcp_connections: None,
        udp: None,
    }));

    let ui_state = state.clone();
    let ui_shutdown = shutdown.clone();

    let _ui_handle = tokio::spawn(async move {
        if let Err(error) = ui::run(ui_state, refresh_interval, ui_shutdown).await {
            tracing::error!(?error, "TUI exited with error");
        }
    });
    let api_state = state.clone();

    let _api_handle = tokio::spawn(async move {
        tracing::info!("Starting HTTP API server");

        api::run_server(api_state).await;

        tracing::info!("HTTP API server stopped");
    });
    tokio::time::sleep(Duration::from_secs(1)).await;

    /////CPU//////////
    let cpu_sender = sender.clone();
    let cpu_shutdown = shutdown.clone();
    let cpu_handle = tokio::spawn(async move {
        tracing::info!(collector = "cpu", "collector started");

        let mut cpu_monitor = match CpuMonitor::new() {
            Ok(monitor) => monitor,
            Err(error) => {
                println!("CPU Error: {:?}", error);
                return;
            }
        };

        let mut interval = tokio::time::interval(Duration::from_secs(2));

        loop {
            tokio::select! {

                                            _ = cpu_shutdown.cancelled() => {
                tracing::info!(
                    collector = "cpu",
                    "collector shutting down"
                );
                break;
            }
                                            _ = interval.tick() => {
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


                                                        cpu_sender
                                                            .send(MetricEvent::Cpu(usage))
                                                            .await
                                                            .unwrap();

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


                                        }
        }
    });
    //////////////////////
    //////MEMORY//////////
    /////////////////////
    let memory_sender = sender.clone();
    let memory_shutdown = shutdown.clone();
    let memory_handle = tokio::spawn(async move {
        tracing::info!(collector = "memory", "collector started");

        let mut interval = tokio::time::interval(Duration::from_secs(2));

        loop {
            tokio::select! {
                                   _ = interval.tick() => {
                        let start = Instant::now();

                        match memory::read_memory_info() {
                            Ok(memory_info) => {
                                let duration = start.elapsed();

                                tracing::debug!(
                                    collector = "memory",
                                    duration_us = duration.as_micros(),
            "sample collected"
                                );

                                memory_sender
                                    .send(MetricEvent::Memory(memory_info))
                                    .await
                                    .unwrap();
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

                                    _ = memory_shutdown.cancelled() => {
                                        tracing::info!(
                                            collector = "memory",
                                            "collector shutting down"
                                        );
                                        break;
                                    }
                                }
        }
    });
    //////////////////
    //// disk //////////
    /////////////////
    let disk_sender = sender.clone();
    let disk_shutdown = shutdown.clone();

    let disk_handle = tokio::spawn(async move {
        tracing::info!(collector = "disk", "collector started");

        let mut disk_monitor = DiskMonitor::new();
        let mut interval = tokio::time::interval(Duration::from_secs(2));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let start = Instant::now();

                    let rates = disk_monitor.sample();

                    let duration = start.elapsed();

                    tracing::debug!(
                        collector = "disk",
                        duration_us = duration.as_micros(),
                        "sample collected"
                    );

                    disk_sender
                        .send(MetricEvent::Disk(rates))
                        .await
                        .unwrap();
                }

                _ = disk_shutdown.cancelled() => {
                    tracing::info!(
                        collector = "disk",
                        "collector shutting down"
                    );
                    break;
                }
            }
        }
    });
    ////////////////////
    ////NETWORK//////////
    ////////////////////
    let network_sender = sender.clone();
    let network_shutdown = shutdown.clone();

    let network_handle = tokio::spawn(async move {
        tracing::info!(collector = "network", "collector started");
        let mut network_monitor = NetworkMonitor::new();
        let mut interval = tokio::time::interval(network_interval);

        loop {
            tokio::select! {
                                    _ = interval.tick() => {
                                        let start = Instant::now();

            let rates = network_monitor.sample();

            let duration = start.elapsed();
                                        tracing::debug!(
                        collector = "network",
                        duration_us = duration.as_micros(),
            "sample collected"
                    );
                                        network_sender
                                            .send(MetricEvent::Network(rates))
                                            .await
                                            .unwrap();
                                    }

                                    _ = network_shutdown.cancelled() => {
                                        tracing::info!(collector = "network", "collector shutting down");
                                        break;
                                    }
                                }
        }
    });
    //////////////////
    ////Processes//////
    /////////////////
    let process_sender = sender.clone();
    let critical_process_sender = critical_sender.clone();

    let process_shutdown = shutdown.clone();

    let process_handle = tokio::spawn(async move {
        tracing::info!(collector = "process", "collector started");

        let mut process_monitor = process::ProcessMonitor::new();
        let baseline = process_monitor.sample();

        let mut previous_processes: std::collections::HashMap<u32, String> = baseline
            .into_iter()
            .map(|process| (process.pid, process.name))
            .collect();

        let mut interval = tokio::time::interval(process_interval);

        loop {
            tokio::select! {
                            _ = interval.tick() => {
                                let start = Instant::now();

                                let processes = process_monitor.sample();

                                let lifecycle_events =
                process::detect_lifecycle_events(&previous_processes, &processes);

            for event in lifecycle_events {
                let event = match event {
                    process::ProcessLifecycleEvent::Started { pid, name } => {
                        MetricEvent::ProcessStarted { pid, name }
                    }
                    process::ProcessLifecycleEvent::Exited { pid, name } => {
                        MetricEvent::ProcessExited { pid, name }
                    }
                };

                if critical_process_sender.send(event).await.is_err() {
                    break;
                }
            }

            previous_processes = processes
                .iter()
                .map(|process| (process.pid, process.name.clone()))
                .collect();


                                let duration = start.elapsed();

                                tracing::debug!(
                                    collector = "process",
                                    count = processes.len(),
                                    duration_us = duration.as_micros(),
                                    "process snapshot collected"
                                );

                                if process_sender
                                    .send(MetricEvent::Processes(processes))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }

                            _ = process_shutdown.cancelled() => {
                                tracing::info!(
                                    collector = "process",
                                    "collector shutting down"
                                );
                                break;
                            }
                        }
        }
    });

    ////////////////////
    //// Cgroup////////
    //////////////////
    let cgroup_sender = sender.clone();
    let cgroup_shutdown = shutdown.clone();

    let cgroup_handle = tokio::spawn(async move {
        tracing::info!(collector = "cgroup", "collector started");

        let mut interval = tokio::time::interval(Duration::from_secs(2));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    match cgroup::discover_cgroups() {
                        Ok((cgroups, total, sub)) => {
                            tracing::debug!(
                                collector = "cgroup",
                                count = cgroups.len(),
                                total_cgroups = total,
                                sub_cgroups = sub,
                                "cgroup snapshot collected"
                            );

                            if cgroup_sender
                                .send(MetricEvent::Cgroups(cgroups))
                                .await
                                .is_err()
                            {
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

                _ = cgroup_shutdown.cancelled() => {
                    tracing::info!(
                        collector = "cgroup",
                        "collector shutting down"
                    );
                    break;
                }
            }
        }
    });
    ////////////////////////
    //// TCP Connections//////
    ////////////////////////
    let tcp_sender = sender.clone();
    let tcp_shutdown = shutdown.clone();

    let tcp_handle = tokio::spawn(async move {
        tracing::info!(collector = "tcp", "collector started");

        let mut interval = tokio::time::interval(Duration::from_secs(2));

        loop {
            tokio::select! {
                            _ = interval.tick() => {
                                let owners = socket::build_socket_owners();

            let rows = match tcp::read_tcp_rows(&owners) {
                Ok(rows) => rows,
                Err(error) => {
                    tracing::warn!(%error, "failed to read TCP connections for TUI");
                    Vec::new()
                }
            };

            let connections = tcp::find_tcp_connections(&rows);

            if tcp_sender
                .send(MetricEvent::Tcp {
                    rows,
                    connections,
                })
                .await
                .is_err()
            {
                break;
            }
                            }

                            _ = tcp_shutdown.cancelled() => {
                                tracing::info!(collector = "tcp", "collector shutting down");
                                break;
                            }
                        }
        }
    });
    ///////////////////////
    //// UDP Connections///
    ///////////////////////
    let udp_sender = sender.clone();
    let udp_shutdown = shutdown.clone();

    let udp_handle = tokio::spawn(async move {
        tracing::info!(collector = "udp", "collector started");

        let mut interval = tokio::time::interval(Duration::from_secs(2));

        loop {
            tokio::select! {
                            _ = interval.tick() => {
                                let owners = socket::build_socket_owners();

            let mut rows = match udp::read_udp_rows(&owners) {
                Ok(rows) => rows,
                Err(error) => {
                    tracing::warn!(%error, "failed to read IPv4 UDP sockets for TUI");
                    Vec::new()
                }
            };

            match udp::read_udp6_rows(&owners) {
                Ok(rows6) => rows.extend(rows6),
                Err(error) => {
                    tracing::warn!(%error, "failed to read IPv6 UDP sockets for TUI");
                }
            }

            if udp_sender
                .send(MetricEvent::Udp(rows))
                .await
                .is_err()
            {
                break;
            }
                            }

                            _ = udp_shutdown.cancelled() => {
                                tracing::info!(
                                    collector = "udp",
                                    "collector shutting down"
                                );
                                break;
                            }
                        }
        }
    });

    // 3. Receive memory data
    loop {
        tokio::select! {
            biased;

            Some(event) = critical_receiver.recv() => {
                match event {
                    MetricEvent::ProcessStarted { pid, name } => {
                        tracing::info!(
                            event = "process_started",
                            pid,
                            process_name = %name,
                            "Process started"
                        );
                    }

                    MetricEvent::ProcessExited { pid, name } => {
                        tracing::info!(
                            event = "process_exited",
                            pid,
                            process_name = %name,
                            "Process exited"
                        );
                    }

                    _ => {
                        tracing::warn!(
                            "Unexpected non-lifecycle event on critical channel"
                        );
                    }
                }
            }

            Some(event) = receiver.recv() => {
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
                        let mut state = state.write().await;
                        state.tcp = Some(rows);
                        state.tcp_connections = Some(connections);
                    }

                    MetricEvent::Udp(rows) => {
                        state.write().await.udp = Some(rows);
                    }

                    MetricEvent::ProcessStarted { .. }
                    | MetricEvent::ProcessExited { .. } => {
                        tracing::warn!(
                            "Lifecycle event unexpectedly received on metrics channel"
                        );
                    }
                }
            }

            _ = shutdown.cancelled() => {
                tracing::info!("shutdown signal received");
                break;
            }
        }
    }
    let _ = cpu_handle.await;
    let _ = memory_handle.await;
    let _ = disk_handle.await;
    let _ = network_handle.await;
    let _ = process_handle.await;
    let _ = cgroup_handle.await;
    let _ = tcp_handle.await;
    let _ = udp_handle.await;
    println!("RWatch shutdown complete");
}
