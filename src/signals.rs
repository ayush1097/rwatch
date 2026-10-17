use nix::sys::signal::Signal;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

#[derive(Debug, PartialEq)]
pub enum RwatchError {
    InvalidPid,
    ProcessGone,
    PermissionDenied,
    StaleProcess,
    UnsupportedPlatform,
    Unknown(String),
}

fn open_pidfd(pid: u32) -> Result<OwnedFd, RwatchError> {
    if pid == 0 {
        return Err(RwatchError::InvalidPid);
    }

    let pid = i32::try_from(pid).map_err(|_| RwatchError::InvalidPid)?;

    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0u32) };

    if fd < 0 {
        return Err(map_errno(std::io::Error::last_os_error()));
    }

    // SAFETY: A successful pidfd_open call returns a new owned file descriptor.
    Ok(unsafe { OwnedFd::from_raw_fd(fd as i32) })
}

fn map_errno(error: std::io::Error) -> RwatchError {
    match error.raw_os_error() {
        Some(libc::ESRCH) => RwatchError::ProcessGone,

        Some(libc::EPERM) | Some(libc::EACCES) => RwatchError::PermissionDenied,

        Some(libc::ENOSYS) => RwatchError::UnsupportedPlatform,

        _ => RwatchError::Unknown(error.to_string()),
    }
}

pub fn send_signal(
    pid: u32,
    expected_start_time_ticks: u64,
    signal: Signal,
) -> Result<(), RwatchError> {
    if pid == 0 {
        return Err(RwatchError::InvalidPid);
    }

    // Open a descriptor referring to this process instance.
    let pidfd = open_pidfd(pid)?;

    // Verify the identity AFTER opening the descriptor.
    // This detects a PID that was already reused before pidfd_open().
    let current = crate::process::read_process(pid).ok_or(RwatchError::ProcessGone)?;

    if current.start_time_ticks != expected_start_time_ticks {
        return Err(RwatchError::StaleProcess);
    }

    // Signal through the descriptor, not through the reusable numeric PID.
    let result = unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            pidfd.as_raw_fd(),
            signal as libc::c_int,
            std::ptr::null::<libc::siginfo_t>(),
            0u32,
        )
    };

    if result < 0 {
        return Err(map_errno(std::io::Error::last_os_error()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_signal_rejects_overflowing_pid() {
        let result = send_signal(u32::MAX, 100, Signal::SIGTERM);
        assert_eq!(result, Err(RwatchError::InvalidPid));
    }

    #[test]
    fn send_signal_rejects_zero_pid() {
        let result = send_signal(0, 100, Signal::SIGTERM);
        assert_eq!(result, Err(RwatchError::InvalidPid));
    }

    #[test]
    fn send_signal_reports_process_gone_for_dead_pid() {
        let pid_max: u32 = std::fs::read_to_string("/proc/sys/kernel/pid_max")
            .expect("could not read kernel PID limit")
            .trim()
            .parse()
            .expect("invalid kernel PID limit");

        let result = send_signal(pid_max, 100, Signal::SIGCONT);

        assert_eq!(result, Err(RwatchError::ProcessGone));
    }
    #[test]
    fn send_signal_rejects_stale_process_identity() {
        let pid = std::process::id();

        let result = send_signal(pid, u64::MAX, Signal::SIGCONT);

        assert_eq!(result, Err(RwatchError::StaleProcess));
    }
    #[test]
    fn send_signal_succeeds_for_matching_process_identity() {
        use std::process::Command;

        // Spawn a dedicated child process for this test.
        let mut child = Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("failed to spawn test process");

        let pid = child.id();

        // Read the child's actual start time.
        let process = crate::process::read_process(pid).expect("failed to read child process");

        // Send SIGCONT using the matching process identity.
        let result = send_signal(pid, process.start_time_ticks, Signal::SIGCONT);

        // Clean up the child, regardless of the signal result.
        let _ = child.kill();
        let _ = child.wait();

        assert_eq!(result, Ok(()));
    }
}
