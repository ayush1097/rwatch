use std::io;
use std::mem;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

const CN_IDX_PROC: u32 = 1;
const CN_VAL_PROC: u32 = 1;
const PROC_CN_MCAST_LISTEN: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
struct CbId {
    idx: u32,
    val: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CnMsg {
    id: CbId,
    seq: u32,
    ack: u32,
    len: u16,
    flags: u16,
}

fn create_netlink_socket() -> io::Result<OwnedFd> {
    let fd = unsafe {
        libc::socket(
            libc::AF_NETLINK,
            libc::SOCK_DGRAM | libc::SOCK_CLOEXEC,
            libc::NETLINK_CONNECTOR,
        )
    };

    if fd < 0 {
        return Err(io::Error::last_os_error());
    }

    let socket = unsafe { OwnedFd::from_raw_fd(fd) };

    let mut address: libc::sockaddr_nl = unsafe { mem::zeroed() };
    address.nl_family = libc::AF_NETLINK as libc::sa_family_t;
    address.nl_pid = 0;
    address.nl_groups = 0;

    let result = unsafe {
        libc::bind(
            socket.as_raw_fd(),
            &address as *const libc::sockaddr_nl as *const libc::sockaddr,
            mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
        )
    };

    if result < 0 {
        return Err(io::Error::last_os_error());
    }

    // Keep receive operations from blocking indefinitely.
    let timeout = libc::timeval {
        tv_sec: 3,
        tv_usec: 0,
    };

    let result = unsafe {
        libc::setsockopt(
            socket.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_RCVTIMEO,
            &timeout as *const libc::timeval as *const libc::c_void,
            mem::size_of::<libc::timeval>() as libc::socklen_t,
        )
    };

    if result < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(socket)
}

fn get_netlink_port_id(socket: &OwnedFd) -> io::Result<u32> {
    let mut address: libc::sockaddr_nl = unsafe { mem::zeroed() };

    let mut address_len =
        mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t;

    let result = unsafe {
        libc::getsockname(
            socket.as_raw_fd(),
            &mut address as *mut libc::sockaddr_nl as *mut libc::sockaddr,
            &mut address_len,
        )
    };

    if result < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(address.nl_pid)
}

fn subscribe_to_process_events(
    socket: &OwnedFd,
    port_id: u32,
) -> io::Result<()> {
    let operation = PROC_CN_MCAST_LISTEN;

    let message = CnMsg {
        id: CbId {
            idx: CN_IDX_PROC,
            val: CN_VAL_PROC,
        },
        seq: 0,
        ack: 0,
        len: mem::size_of::<u32>() as u16,
        flags: 0,
    };

    // Build: Netlink header + connector header + operation.
    let mut buffer = Vec::new();

    let nl_header = libc::nlmsghdr {
        nlmsg_len: (mem::size_of::<libc::nlmsghdr>()
            + mem::size_of::<CnMsg>()
            + mem::size_of::<u32>()) as u32,
        nlmsg_type: libc::NLMSG_DONE as u16,
        nlmsg_flags: 0,
        nlmsg_seq: 0,
        nlmsg_pid: port_id,
    };

    let nl_bytes = unsafe {
        std::slice::from_raw_parts(
            &nl_header as *const libc::nlmsghdr as *const u8,
            mem::size_of::<libc::nlmsghdr>(),
        )
    };

    let cn_bytes = unsafe {
        std::slice::from_raw_parts(
            &message as *const CnMsg as *const u8,
            mem::size_of::<CnMsg>(),
        )
    };

    let op_bytes = unsafe {
        std::slice::from_raw_parts(
            &operation as *const u32 as *const u8,
            mem::size_of::<u32>(),
        )
    };

    buffer.extend_from_slice(nl_bytes);
    buffer.extend_from_slice(cn_bytes);
    buffer.extend_from_slice(op_bytes);

    let mut destination: libc::sockaddr_nl = unsafe { mem::zeroed() };
    destination.nl_family = libc::AF_NETLINK as libc::sa_family_t;
    destination.nl_pid = 0;
    destination.nl_groups = 0;

    let sent = unsafe {
        libc::sendto(
            socket.as_raw_fd(),
            buffer.as_ptr() as *const libc::c_void,
            buffer.len(),
            0,
            &destination as *const libc::sockaddr_nl as *const libc::sockaddr,
            mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
        )
    };

    if sent < 0 {
        return Err(io::Error::last_os_error());
    }

    if sent as usize != buffer.len() {
        return Err(io::Error::new(
            io::ErrorKind::WriteZero,
            "Incomplete Netlink message sent",
        ));
    }

    Ok(())
}

fn receive_netlink_message(socket: &OwnedFd) -> io::Result<()> {
    let mut buffer = [0u8; 4096];

    let received = unsafe {
        libc::recv(
            socket.as_raw_fd(),
            buffer.as_mut_ptr() as *mut libc::c_void,
            buffer.len(),
            0,
        )
    };

    if received < 0 {
        return Err(io::Error::last_os_error());
    }

    let received = received as usize;

    if received < mem::size_of::<libc::nlmsghdr>() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Netlink message is shorter than its header",
        ));
    }

    let header = unsafe {
        std::ptr::read_unaligned(
            buffer.as_ptr() as *const libc::nlmsghdr
        )
    };

    if header.nlmsg_len as usize > received
        || (header.nlmsg_len as usize) < mem::size_of::<libc::nlmsghdr>()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid Netlink message length",
        ));
    }

    println!(
        "Received Netlink message: type={}, length={}, sender_port_id={}",
        header.nlmsg_type,
        header.nlmsg_len,
        header.nlmsg_pid,
    );

    if header.nlmsg_type == libc::NLMSG_ERROR as u16 {
        let error_offset = mem::size_of::<libc::nlmsghdr>();

        if received < error_offset + mem::size_of::<i32>() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Netlink error response is incomplete",
            ));
        }

        let error_code = unsafe {
            std::ptr::read_unaligned(
                buffer.as_ptr().add(error_offset) as *const i32
            )
        };

        if error_code == 0 {
            println!("Received a Netlink ACK.");
        } else {
            return Err(io::Error::from_raw_os_error(-error_code));
        }
    }

    Ok(())
}

fn main() -> io::Result<()> {
    let socket = create_netlink_socket()?;
    println!("Netlink socket created successfully.");

    let port_id = get_netlink_port_id(&socket)?;
    println!("Netlink port ID: {}", port_id);

    subscribe_to_process_events(&socket, port_id)?;
    println!("Process-event subscription request sent.");

    println!("Waiting up to 3 seconds for a Netlink message...");

    match receive_netlink_message(&socket) {
        Ok(()) => println!("A Netlink message was received."),
        Err(error)
            if error.kind() == io::ErrorKind::WouldBlock
                || error.kind() == io::ErrorKind::TimedOut =>
        {
            println!("No Netlink message arrived within 3 seconds.");
            println!("Subscription success is not yet confirmed.");
        }
        Err(error) => return Err(error),
    }

    Ok(())
}