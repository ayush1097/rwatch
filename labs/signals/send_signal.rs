use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use std::env;
#[derive(Debug)]
pub enum RwatchError {
    InvalidPid,
    ProcessGone,
    PermissionDenied,
    Unknown(String),
}
fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        println!("Usage: send_signal <pid> <action>");
        return;
    }

    let pid: i32 = match args[1].parse() {
        Ok(pid) => pid,
        Err(_) => {
            println!("Invalid PID");
            return;
        }
    };
    let action = &args[2];

    let signal = match action.as_str() {
        "terminate" => Signal::SIGTERM,
        "stop" => Signal::SIGSTOP,
        "continue" => Signal::SIGCONT,
        _ => {
            println!("Unknown action");
            return;
        }
    };

    match kill(Pid::from_raw(pid), signal) {
        Ok(()) => println!("Signal sent successfully"),

        Err(nix::Error::ESRCH) => {
            println!("Process does not exist");
        }

        Err(nix::Error::EPERM) => {
            println!("Permission denied");
        }

        Err(e) => {
            println!("Other error: {e}");
        }
    }
}
