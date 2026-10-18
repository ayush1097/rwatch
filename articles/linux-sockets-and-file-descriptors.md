# Connecting Linux File Descriptors to Network Sockets

## Introduction

Network monitoring involves more than identifying local
and remote IP addresses.

A useful monitoring tool should also help answer questions
such as:

- Which TCP connections are established?
- Which process owns a socket?
- Which file descriptor refers to that socket?
- What information does Linux expose about network endpoints?

RWatch investigates these questions using Linux's `/proc`
filesystem.

It reads TCP and UDP tables, inspects process file
descriptors, and connects the resulting information
through socket inode numbers.

This article explains how these Linux interfaces work
together in RWatch.

## 1. Processes and File Descriptors

Linux represents an open file, socket, pipe, or other
resource through a file descriptor associated with a process.

A process exposes its open file descriptors through:

    /proc/<pid>/fd/

For example:

    /proc/1234/fd/3
    /proc/1234/fd/4
    /proc/1234/fd/5

These entries are symbolic links to the underlying
resources.

A socket descriptor can point to a link formatted like:

    socket:[12345]

The number inside the brackets identifies the socket
inode exposed through this interface.

RWatch uses these links to discover which processes
hold descriptors referring to sockets.

## 2. Discovering Socket Owners

RWatch implements this functionality in
`src/socket.rs`.

The `build_socket_owners()` function begins by reading
the `/proc` directory.

It examines entries whose names contain only decimal
digits, treating them as candidate process IDs.

For each candidate process, it attempts to:

1. Parse the process ID.
2. Read the process name from `/proc/<pid>/comm`.
3. Open `/proc/<pid>/fd/`.
4. Enumerate the process's file descriptors.
5. Read each descriptor's symbolic link.
6. Check whether the link represents a socket.
7. Extract the socket inode and record its owner.

The resulting mapping associates each discovered socket
inode with one or more `SocketOwner` records.

Each record contains:

- `pid`: the process ID.
- `fd`: the file descriptor number.
- `process`: the process name.

A socket can have multiple owners or descriptors, so
RWatch stores a vector of owners for each inode rather
than assuming every socket maps to exactly one process.

## 3. Parsing Socket Inodes

RWatch uses `parse_socket_inode()` to identify socket
symbolic links.

For example:

    socket:[12345]

The parser removes the `socket:[` prefix and the closing
`]` suffix, then parses the remaining value as a `u64`.

If the link does not match the expected format or the
number cannot be parsed, the function returns `None`.

This keeps socket-link parsing separate from the filesystem
traversal logic.

RWatch tests valid socket links as well as malformed
links and links representing other resource types.

These tests help ensure that ordinary files and pipes
are not incorrectly identified as sockets.

## 4. Reading TCP Information

Linux exposes IPv4 TCP information through:

    /proc/net/tcp

IPv6 TCP information is exposed through:

    /proc/net/tcp6

RWatch reads these files in `src/tcp.rs`.

For each table, it skips the header line and splits the
remaining lines into whitespace-separated fields.

It then extracts information including:

- Local IP address and port.
- Remote IP address and port.
- TCP state.
- Socket inode.

The endpoint parsing functions convert the kernel's
endpoint representation into IP addresses and port
numbers suitable for the monitoring application's data
structures.

RWatch maps the TCP state field to a `TcpState` enum.

Examples include:

- `ESTABLISHED`
- `SYN_SENT`
- `SYN_RECV`
- `TIME_WAIT`
- `LISTEN`
- `CLOSE_WAIT`

Unknown state values are represented by `TcpState::Unknown`.

This converts the kernel's encoded state into a form
that other parts of RWatch can interpret.

## 5. Connecting TCP Rows to Processes

Reading a TCP table provides socket information, but
the table alone does not directly provide the owning
process name in RWatch's data model.

RWatch connects the two sources using the socket inode.

The socket discovery code builds a mapping:

    socket_inode -> Vec<SocketOwner>

The TCP parser reads the inode associated with each
TCP entry and looks it up in that mapping.

When a matching inode is found, the corresponding
owners are attached to the `TcpRow`.

If no owner is found, RWatch stores an empty owner vector.

This approach joins information from two different
Linux interfaces:

- `/proc/net/tcp` and `/proc/net/tcp6` describe TCP entries.
- `/proc/<pid>/fd/` exposes process file descriptors
  that can refer to sockets.

The shared inode provides the connection between them.

## 6. Reconstructing Established TCP Connections

A TCP connection involves two endpoints.

When both endpoints are visible in the collected TCP
rows, RWatch attempts to identify them as a pair.

The `find_tcp_connections()` function compares rows
and looks for two entries that:

- Are both in the `Established` state.
- Have reversed local and remote endpoints.

For example, the endpoints might appear as:

    Endpoint A:
    Local:  192.0.2.10:50000
    Remote: 192.0.2.20:443

    Endpoint B:
    Local:  192.0.2.20:443
    Remote: 192.0.2.10:50000

The reversed endpoint relationship indicates that
the rows represent opposite sides of the same connection.

RWatch creates a `TcpConnection` containing both rows.

It also generates a normalized connection key to avoid
recording the same endpoint pair more than once.

This reconstruction depends on both endpoint rows being
visible and matching the expected relationship. It should
not be treated as a guarantee that every connection will
always be represented by two discoverable rows.

## 7. Reading UDP Information

RWatch reads UDP information from:

    /proc/net/udp
    /proc/net/udp6

The implementation in `src/udp.rs` extracts fields
including:

- Local IP address and port.
- Remote IP address and port.
- User ID.
- Socket inode.
- Transmit queue.
- Receive queue.
- Drop count.

The queue field contains two hexadecimal values separated
by a colon.

RWatch parses these values using base-16 conversion and
stores them as numeric transmit and receive queue values.

It then looks up the socket inode in the same owner mapping
used for TCP entries.

This allows UDP rows to include the process and descriptor
information discovered by `build_socket_owners()`.

## 8. Limitations and Race Conditions

The `/proc` filesystem exposes live system state, so
information can change while RWatch is collecting it.

A process can exit while its file descriptors are being
enumerated. A descriptor can also disappear between
enumeration and symbolic-link reading.

RWatch handles many individual discovery failures by
skipping entries it cannot read or parse.

However, the TCP and UDP table readers use `expect()`
for file reads and some field parsing operations. An
unexpected read failure or malformed entry can therefore
cause a panic rather than a recoverable collection error.

Permissions can also prevent RWatch from inspecting
some processes.

Consequently, an empty owner list does not necessarily
mean that no process owns the socket. It can mean that
the owner was not discovered during the scan.

These limitations matter when interpreting monitoring
results as observations of a changing system rather than
a perfectly consistent snapshot.

## Conclusion

RWatch combines information from several Linux interfaces
to build a more useful picture of network activity.

It reads TCP and UDP tables, discovers socket owners
through process file descriptors, and associates the
results using socket inode numbers.

It also interprets TCP states and attempts to reconstruct
established connections from matching endpoint pairs.

The key idea is that Linux exposes different parts of
the same system through different interfaces. A monitoring
tool can combine those interfaces to produce information
that is more useful than any individual file provides.
