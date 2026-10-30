use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use nix::sys::signal::Signal;
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph, Row, Table},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingSignal {
    Terminate,
    ForceKill,
}

use tokio::sync::RwLock;

use crate::state::RWatchState;
struct TerminalCleanup;

impl Drop for TerminalCleanup {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Overview,
    Processes,
    ProcessDetail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessSort {
    Cpu,
    Memory,
}
fn format_limit(value: Option<u64>) -> String {
    match value {
        Some(value) => value.to_string(),
        None => "Unlimited".to_string(),
    }
}
fn format_namespace(value: &crate::namespace::ProcessNamespaces, name: &str) -> String {
    match name {
        "PID" => match &value.pid {
            Ok(id) => id.to_string(),
            Err(error) => format!("{:?}", error),
        },

        "NET" => match &value.net {
            Ok(id) => id.to_string(),
            Err(error) => format!("{:?}", error),
        },

        "MNT" => match &value.mnt {
            Ok(id) => id.to_string(),
            Err(error) => format!("{:?}", error),
        },

        "USER" => match &value.user {
            Ok(id) => id.to_string(),
            Err(error) => format!("{:?}", error),
        },

        _ => "Unknown".to_string(),
    }
}
pub async fn run(
    state: Arc<RwLock<RWatchState>>,
    refresh_interval: Duration,
    shutdown: CancellationToken,
) -> Result<(), io::Error> {
    let mut stdout = io::stdout();

    enable_raw_mode()?;
    let _cleanup = TerminalCleanup;

    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);

    let mut terminal = Terminal::new(backend)?;

    let mut screen = Screen::Overview;
    let mut process_sort = ProcessSort::Cpu;
    let mut selected_process: usize = 0;
    let mut process_list_selection: Option<(u32, u64)> = None;
    let mut selected_process_identity: Option<(u32, u64)> = None;
    let mut status_message: Option<String> = None;
    let mut pending_signal: Option<PendingSignal> = None;
    loop {
        // ---------------------------------------------------------
        // Read state
        // ---------------------------------------------------------

        let (cpu, memory, disk, network, tcp, udp, cgroups, processes) = {
            let state = state.read().await;

            let cpu = state.cpu;

            let memory = state.memory.as_ref().map(|memory| {
                (
                    memory.total_gb(),
                    memory.free_gb(),
                    memory.available_gb(),
                    memory.used_gb(),
                )
            });

            let disk = state.disk.as_ref().map(|disks| {
                disks
                    .iter()
                    .map(|disk| {
                        (
                            disk.device.clone(),
                            disk.read_rate,
                            disk.write_rate,
                            disk.read_iops,
                            disk.write_iops,
                        )
                    })
                    .collect::<Vec<_>>()
            });

            let network = state.network.as_ref().map(|networks| {
                networks
                    .iter()
                    .map(|network| {
                        (
                            network.interface.clone(),
                            network.download_rate,
                            network.upload_rate,
                        )
                    })
                    .collect::<Vec<_>>()
            });

            let tcp = state.tcp.as_ref().map(|rows| {
                rows.iter()
                    .map(|row| {
                        (
                            row.local_ip.clone(),
                            row.local_port,
                            row.remote_ip.clone(),
                            row.remote_port,
                            row.state.to_string(),
                            row.inode,
                            row.owners
                                .iter()
                                .map(|owner| (owner.pid, owner.fd, owner.process.clone()))
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Vec<_>>()
            });

            let udp = state.udp.as_ref().map(|rows| {
                rows.iter()
                    .map(|row| {
                        (
                            row.local_ip.clone(),
                            row.local_port,
                            row.remote_ip.clone(),
                            row.remote_port,
                            row.uid,
                            row.inode,
                            row.rx_queue,
                            row.tx_queue,
                            row.drops,
                            row.owners
                                .iter()
                                .map(|owner| (owner.pid, owner.fd, owner.process.clone()))
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Vec<_>>()
            });

            let cgroups = state.cgroups.as_ref().map(|groups| {
                groups
                    .iter()
                    .map(|cgroup| {
                        (
                            cgroup.path.clone(),
                            cgroup.pids.len(),
                            cgroup.memory_current,
                            cgroup.memory_max,
                            cgroup.cpu_usage_usec,
                            cgroup.cpu_quota,
                            cgroup.cpu_period_usec,
                            cgroup.pids_current,
                            cgroup.pids_max,
                        )
                    })
                    .collect::<Vec<_>>()
            });

            let processes = state.processes.as_ref().map(|processes| {
                processes
                    .iter()
                    .map(|process| {
                        (
                            process.pid,
                            process.name.clone(),
                            process.state.clone(),
                            process.parent_pid,
                            process.memory_kb,
                            process.threads,
                            process.cpu_usage,
                            process.namespaces.clone(),
                            process.limits.clone(),
                            process.start_time_ticks,
                        )
                    })
                    .collect::<Vec<_>>()
            });

            (cpu, memory, disk, network, tcp, udp, cgroups, processes)
        };

        // ---------------------------------------------------------
        // Keyboard input
        // ---------------------------------------------------------

        if event::poll(Duration::from_millis(0))?
            && let Event::Key(key) = event::read()?
        {
            match key.code {
                KeyCode::Char('o') => {
                    screen = Screen::Overview;
                    selected_process_identity = None;
                }

                KeyCode::Char('p') => {
                    screen = Screen::Processes;
                    selected_process = 0;
                    selected_process_identity = None;
                }

                KeyCode::Char('c') => {
                    process_sort = ProcessSort::Cpu;
                    selected_process = 0;
                }

                KeyCode::Char('m') => {
                    process_sort = ProcessSort::Memory;
                    selected_process = 0;
                }

                KeyCode::Up => {
                    if screen == Screen::Processes {
                        selected_process = selected_process.saturating_sub(1);
                        process_list_selection = None;
                    }
                }

                KeyCode::Down => {
                    if screen == Screen::Processes {
                        selected_process = selected_process.saturating_add(1);
                        process_list_selection = None;
                    }
                }

                KeyCode::Char('k') => {
                    if screen == Screen::ProcessDetail && selected_process_identity.is_some() {
                        pending_signal = Some(PendingSignal::ForceKill);
                        status_message = None;
                    }
                }

                KeyCode::Char('y') => {
                    if let (Some(action), Some((pid, start_time_ticks))) =
                        (pending_signal, selected_process_identity)
                    {
                        pending_signal = None;

                        let signal = match action {
                            PendingSignal::Terminate => Signal::SIGTERM,
                            PendingSignal::ForceKill => Signal::SIGKILL,
                        };

                        let signal_name = match action {
                            PendingSignal::Terminate => "SIGTERM",
                            PendingSignal::ForceKill => "SIGKILL",
                        };

                        match crate::signals::send_signal(pid, start_time_ticks, signal) {
                            Ok(()) => {
                                status_message =
                                    Some(format!("{} sent to PID {}", signal_name, pid));
                            }
                            Err(error) => {
                                status_message = Some(format!(
                                    "Failed to send {} to PID {}: {:?}",
                                    signal_name, pid, error
                                ));
                            }
                        }
                    }
                }

                KeyCode::Char('n') => {
                    if pending_signal.is_some() {
                        pending_signal = None;
                        status_message = Some("Signal cancelled".to_string());
                    }
                }

                KeyCode::Char('t') => {
                    if screen == Screen::ProcessDetail && selected_process_identity.is_some() {
                        pending_signal = Some(PendingSignal::Terminate);
                        status_message = None;
                    }
                }

                // NEW
                KeyCode::Enter => {
                    if screen == Screen::Processes
                        && let Some(processes) = processes.as_ref()
                    {
                        let mut sorted_processes = processes.clone();

                        match process_sort {
                            ProcessSort::Cpu => {
                                sorted_processes.sort_by(|a, b| {
                                    b.6.partial_cmp(&a.6).unwrap_or(std::cmp::Ordering::Equal)
                                });
                            }

                            ProcessSort::Memory => {
                                sorted_processes.sort_by(|a, b| b.4.cmp(&a.4));
                            }
                        }

                        if let Some(process) = sorted_processes.get(selected_process) {
                            selected_process_identity = Some((process.0, process.9));

                            screen = Screen::ProcessDetail;
                        }
                    }
                }

                // NEW
                KeyCode::Esc => {
                    if pending_signal.is_some() {
                        pending_signal = None;
                        status_message = Some("Signal cancelled".to_string());
                    } else if screen == Screen::ProcessDetail {
                        screen = Screen::Processes;
                        selected_process_identity = None;
                        status_message = None;
                    }
                }

                KeyCode::Char('q') => {
                    break;
                }

                _ => {}
            }
        }

        // ---------------------------------------------------------
        // Draw UI
        // ---------------------------------------------------------

        terminal.draw(|frame| {
            let areas = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(1)])
                .split(frame.area());
            let content_area = areas[0];

            match screen {
                // =================================================
                // OVERVIEW SCREEN
                // =================================================
                Screen::Overview => {
                    let area = content_area;

                    if area.height < 33 || area.width < 80 {
                        let message = Paragraph::new(
                            "Terminal too small for the Overview dashboard.\n\
         Increase the terminal size or use the Processes screen (p).\n\
         Press q to quit.",
                        )
                        .block(Block::default().title(" RWatch ").borders(Borders::ALL));

                        frame.render_widget(message, area);
                    } else {
                        let rows = Layout::default()
                            .direction(Direction::Vertical)
                            .constraints([
                                Constraint::Length(8), // CPU + Memory
                                Constraint::Min(5),    // Disk
                                Constraint::Min(5),    // Network
                                Constraint::Min(5),    // TCP
                                Constraint::Min(5),    // UDP
                                Constraint::Min(5),    // Cgroups
                            ])
                            .split(area);

                        // -------------------------------------------------
                        // CPU + Memory
                        // -------------------------------------------------

                        let top = Layout::default()
                            .direction(Direction::Horizontal)
                            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                            .split(rows[0]);

                        // ---------------- CPU ----------------

                        let cpu_text = match cpu {
                            Some(value) => {
                                format!("CPU: {:.2}%", value)
                            }

                            None => "CPU: collecting...".to_string(),
                        };

                        let cpu_widget = Paragraph::new(cpu_text)
                            .block(Block::default().title(" CPU ").borders(Borders::ALL));

                        // ---------------- Memory ----------------

                        let memory_text = match memory {
                            Some((total, free, available, used)) => {
                                format!(
                                    "Total: {:.2} GB\n\
                                 Free: {:.2} GB\n\
                                 Available: {:.2} GB\n\
                                 Used: {:.2} GB",
                                    total, free, available, used
                                )
                            }

                            None => "Memory: collecting...".to_string(),
                        };

                        let memory_widget = Paragraph::new(memory_text)
                            .block(Block::default().title(" Memory ").borders(Borders::ALL));

                        frame.render_widget(cpu_widget, top[0]);
                        frame.render_widget(memory_widget, top[1]);

                        // -------------------------------------------------
                        // Disk
                        // -------------------------------------------------

                        let disk_text = match disk {
                            Some(disks) => {
                                let mut text = String::new();

                                for disk in disks.iter().take(10) {
                                    let (device, read_rate, write_rate, read_iops, write_iops) =
                                        disk;

                                    text.push_str(&format!(
                                        "{}  Read: {:.2} B/s  Write: {:.2} B/s  \
                                     Read IOPS: {:.2}  Write IOPS: {:.2}\n",
                                        device, read_rate, write_rate, read_iops, write_iops
                                    ));
                                }

                                if text.is_empty() {
                                    "No disks found.".to_string()
                                } else {
                                    text
                                }
                            }

                            None => "Disk: collecting...".to_string(),
                        };

                        let disk_widget = Paragraph::new(disk_text)
                            .block(Block::default().title(" Disk ").borders(Borders::ALL));

                        frame.render_widget(disk_widget, rows[1]);

                        // -------------------------------------------------
                        // Network
                        // -------------------------------------------------

                        let network_text = match network {
                            Some(networks) => {
                                let mut text = String::new();

                                for (interface, download_rate, upload_rate) in
                                    networks.iter().take(10)
                                {
                                    text.push_str(&format!(
                                        "{}  Download: {}  Upload: {}\n",
                                        interface,
                                        crate::network::format_rate(*download_rate),
                                        crate::network::format_rate(*upload_rate)
                                    ));
                                }

                                if text.is_empty() {
                                    "No network interfaces found.".to_string()
                                } else {
                                    text
                                }
                            }

                            None => "Network: collecting...".to_string(),
                        };

                        let network_widget = Paragraph::new(network_text)
                            .block(Block::default().title(" Network ").borders(Borders::ALL));

                        frame.render_widget(network_widget, rows[2]);

                        // -------------------------------------------------
                        // TCP
                        // -------------------------------------------------

                        let tcp_text = match tcp {
                            Some(rows) => {
                                let mut text = String::new();

                                for (
                                    local_ip,
                                    local_port,
                                    remote_ip,
                                    remote_port,
                                    state,
                                    inode,
                                    owners,
                                ) in rows.iter().take(10)
                                {
                                    text.push_str(&format!(
                                        "{}:{} -> {}:{}  {}  inode:{}",
                                        local_ip, local_port, remote_ip, remote_port, state, inode
                                    ));

                                    for (pid, fd, process) in owners {
                                        text.push_str(&format!(
                                            "  owner: {}(pid={}, fd={})",
                                            process, pid, fd
                                        ));
                                    }

                                    text.push('\n');
                                }

                                if text.is_empty() {
                                    "No TCP sockets found.".to_string()
                                } else {
                                    text
                                }
                            }

                            None => "TCP: collecting...".to_string(),
                        };

                        let tcp_widget = Paragraph::new(tcp_text).block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(" TCP / Sockets "),
                        );

                        frame.render_widget(tcp_widget, rows[3]);

                        // -------------------------------------------------
                        // UDP
                        // -------------------------------------------------

                        let udp_text = match udp {
                            Some(rows) => {
                                let mut text = String::new();

                                for (
                                    local_ip,
                                    local_port,
                                    remote_ip,
                                    remote_port,
                                    uid,
                                    _inode,
                                    rx_queue,
                                    tx_queue,
                                    drops,
                                    owners,
                                ) in rows.iter().take(10)
                                {
                                    text.push_str(&format!(
                                        "{}:{} -> {}:{}  UID:{}  RX:{}  TX:{}  Drops:{}",
                                        local_ip,
                                        local_port,
                                        remote_ip,
                                        remote_port,
                                        uid,
                                        rx_queue,
                                        tx_queue,
                                        drops
                                    ));

                                    for (pid, fd, process) in owners {
                                        text.push_str(&format!(
                                            "  owner: {}(pid={}, fd={})",
                                            process, pid, fd
                                        ));
                                    }

                                    text.push('\n');
                                }

                                if text.is_empty() {
                                    "No UDP sockets found.".to_string()
                                } else {
                                    text
                                }
                            }

                            None => "UDP: collecting...".to_string(),
                        };

                        let udp_widget = Paragraph::new(udp_text).block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(" UDP / Sockets "),
                        );

                        frame.render_widget(udp_widget, rows[4]);

                        // -------------------------------------------------
                        // Cgroups
                        // -------------------------------------------------

                        let cgroup_text = match cgroups {
                            Some(groups) => {
                                let mut text = String::new();

                                for (
                                    path,
                                    pid_count,
                                    memory_current,
                                    memory_max,
                                    cpu_usage_usec,
                                    cpu_quota,
                                    cpu_period,
                                    pids_current,
                                    pids_max,
                                ) in groups.iter().take(10)
                                {
                                    text.push_str(&format!(
                                        "{}  PIDs:{}  Memory:{:?}/{:?}  \
                                     CPU:{}us  CPU quota:{:?}/{:?}  \
                                     PIDs:{}/{:?}\n",
                                        path,
                                        pid_count,
                                        memory_current,
                                        memory_max,
                                        cpu_usage_usec,
                                        cpu_quota,
                                        cpu_period,
                                        pids_current,
                                        pids_max
                                    ));
                                }

                                if text.is_empty() {
                                    "No cgroups found.".to_string()
                                } else {
                                    text
                                }
                            }

                            None => "Cgroups: collecting...".to_string(),
                        };

                        let cgroup_widget = Paragraph::new(cgroup_text)
                            .block(Block::default().borders(Borders::ALL).title(" Cgroups "));

                        frame.render_widget(cgroup_widget, rows[5]);
                    }
                }
                // =================================================
                // PROCESSES SCREEN
                // =================================================
                Screen::Processes => {
                    let area = content_area;

                    let process_rows = match processes {
                        Some(mut processes) => {
                            match process_sort {
                                ProcessSort::Cpu => {
                                    processes.sort_by(|a, b| {
                                        b.6.partial_cmp(&a.6).unwrap_or(std::cmp::Ordering::Equal)
                                    });
                                }

                                ProcessSort::Memory => {
                                    processes.sort_by(|a, b| b.4.cmp(&a.4));
                                }
                            }

                            // Keep selection inside the available process list.
                            // Preserve the selected process when sorting changes row order.
                            let visible_count = processes.len().min(50);

                            if let Some(identity) = process_list_selection
                                && let Some(index) = processes
                                    .iter()
                                    .take(visible_count)
                                    .position(|process| (process.0, process.9) == identity)
                            {
                                selected_process = index;
                            }
                            if visible_count == 0 {
                                selected_process = 0;
                                process_list_selection = None;
                            } else {
                                selected_process = selected_process.min(visible_count - 1);

                                let process = &processes[selected_process];
                                process_list_selection = Some((process.0, process.9));
                            }
                            processes
                                .iter()
                                .take(50)
                                .enumerate()
                                .map(|(index, process)| {
                                    let marker = if index == selected_process { ">" } else { " " };

                                    Row::new(vec![
                                        format!("{} {}", marker, process.0),
                                        process.1.clone(),
                                        format!("{:.2}%", process.6),
                                        format!("{} KB", process.4),
                                        process.5.to_string(),
                                        process.2.clone(),
                                    ])
                                })
                                .collect::<Vec<_>>()
                        }

                        None => Vec::new(),
                    };

                    let process_table = Table::new(
                        process_rows,
                        [
                            Constraint::Length(8),  // PID
                            Constraint::Length(24), // Name
                            Constraint::Length(10), // CPU
                            Constraint::Length(15), // Memory
                            Constraint::Length(10), // Threads
                            Constraint::Length(15), // State
                        ],
                    )
                    .header(Row::new(vec![
                        "PID", "NAME", "CPU", "MEMORY", "THREADS", "STATE",
                    ]))
                    .block(Block::default().title(" Processes ").borders(Borders::ALL));

                    frame.render_widget(process_table, area);
                }
                Screen::ProcessDetail => {
                    let area = content_area;

                    let detail_text = match selected_process_identity {
                        Some((pid, start_time_ticks)) => {
                            match processes.as_ref().and_then(|processes| {
                                processes.iter().find(|process| {
                                    process.0 == pid && process.9 == start_time_ticks
                                })
                            }) {
                                Some(process) => {
                                    let namespaces = &process.7;
                                    let limits = &process.8;

                                    format!(
                                        "PID: {}\n\
     Name: {}\n\
     State: {}\n\
     Parent PID: {}\n\
     CPU Usage: {:.2}%\n\
     Memory: {} KB\n\
     Threads: {}\n\n\
     Namespaces:\n\
     PID: {}\n\
     NET: {}\n\
     MNT: {}\n\
     USER: {}\n\n\
     Resource Limits:\n\
     Max Open Files:\n\
       Soft: {}\n\
       Hard: {}\n\
     Max Processes:\n\
       Soft: {}\n\
       Hard: {}\n\
     Max Locked Memory:\n\
       Soft: {}\n\
       Hard: {}\n\
     Max Address Space:\n\
       Soft: {}\n\
       Hard: {}",
                                        process.0,
                                        process.1,
                                        process.2,
                                        process.3,
                                        process.6,
                                        process.4,
                                        process.5,
                                        format_namespace(namespaces, "PID"),
                                        format_namespace(namespaces, "NET"),
                                        format_namespace(namespaces, "MNT"),
                                        format_namespace(namespaces, "USER"),
                                        format_limit(limits.max_open_files.0),
                                        format_limit(limits.max_open_files.1),
                                        format_limit(limits.max_processes.0),
                                        format_limit(limits.max_processes.1),
                                        format_limit(limits.max_locked_memory.0),
                                        format_limit(limits.max_locked_memory.1),
                                        format_limit(limits.max_address_space.0),
                                        format_limit(limits.max_address_space.1)
                                    )
                                }

                                None => "Process no longer exists.".to_string(),
                            }
                        }

                        None => "No process selected.".to_string(),
                    };

                    let detail_text = if let Some(action) = pending_signal {
                        if let Some((pid, _start_time_ticks)) = selected_process_identity {
                            let action_name = match action {
                                PendingSignal::Terminate => "SIGTERM",
                                PendingSignal::ForceKill => "SIGKILL",
                            };

                            format!(
                                "{}\n\nCONFIRM: Send {} to PID {}? [y/N]",
                                detail_text, action_name, pid
                            )
                        } else {
                            detail_text
                        }
                    } else {
                        detail_text
                    };

                    let detail_widget = Paragraph::new(detail_text).block(
                        Block::default()
                            .title(" Process Details ")
                            .borders(Borders::ALL),
                    );

                    frame.render_widget(detail_widget, area);
                }
            }

            if let Some(message) = status_message.as_deref() {
                frame.render_widget(Paragraph::new(message), areas[1]);
            }
        })?;

        tokio::select! {
            _ = shutdown.cancelled() => {
                tracing::info!("TUI shutting down");
                break;
            }
            _ = tokio::time::sleep(refresh_interval) => {}
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::namespace::{NamespaceError, ProcessNamespaces};

    #[test]
    fn formats_limit_correctly() {
        assert_eq!(format_limit(Some(1024)), "1024");
        assert_eq!(format_limit(Some(0)), "0");
        assert_eq!(format_limit(None), "Unlimited");
    }

    #[test]
    fn formats_namespaces_correctly() {
        let namespaces = ProcessNamespaces {
            pid: Ok(4026531836),
            net: Ok(4026531992),
            mnt: Err(NamespaceError::PermissionDenied),
            user: Err(NamespaceError::InvalidFormat),
        };

        assert_eq!(format_namespace(&namespaces, "PID"), "4026531836");
        assert_eq!(format_namespace(&namespaces, "NET"), "4026531992");
        assert_eq!(format_namespace(&namespaces, "MNT"), "PermissionDenied");
        assert_eq!(format_namespace(&namespaces, "USER"), "InvalidFormat");
        assert_eq!(format_namespace(&namespaces, "IPC"), "Unknown");
    }
}
