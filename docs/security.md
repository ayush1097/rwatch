# RWatch Security Model

## 1. Overview

RWatch is a Linux monitoring tool written in Rust.

It reads information exposed by Linux through interfaces such as `/proc`
and other system resources. It also supports explicit process-control
operations through signals.

The default purpose of RWatch is observation, not modification.

## 2. Permissions

RWatch can run as a regular user without root privileges.

However, Linux may restrict access to information about processes owned
by other users. Access depends on system permissions, mount options,
kernel configuration, and security policies.

RWatch should handle inaccessible process information gracefully rather
than assuming that every process can be inspected.

Running RWatch as root increases the information and processes it may
be able to access. Root privileges should not be required for ordinary
monitoring.

## 3. Read-Only Monitoring

Collecting process, CPU, memory, and network information is intended
to be observational.

Monitoring should not automatically terminate, stop, or modify
monitored processes.

Process-control operations are separate from routine metric collection.

## 4. Process Control

RWatch supports process-control operations using Linux signals.

The TUI requires explicit confirmation before sending SIGTERM or
SIGKILL.

- SIGTERM requests that a process terminate.
- SIGKILL forcibly terminates a process.
- Users should understand the consequences before confirming either action.

Invalid PIDs must be rejected.

PID 0 must be rejected because Linux interprets it specially when
sending signals.

## 5. PID Reuse and pidfd

Linux may reuse a PID after its original process exits.

Consequently, a PID alone is not a permanent process identity.

RWatch records a process's start time and uses it to check whether the
selected process still matches the expected identity.

The PIDFD-based signaling implementation opens a file descriptor for
the process and sends the signal through that descriptor rather than
signaling through the numeric PID directly.

This helps avoid accidentally signaling a different process after PID
reuse.

If the expected start time does not match, RWatch rejects the request
as a stale process identity.

## 6. Error Handling

Process-control operations may fail because:

- The process no longer exists.
- The PID is invalid.
- The process identity is stale.
- The current user lacks permission.
- The operating system reports another error.

These failures should be reported without silently retrying a
potentially destructive operation against another process.

## 7. Limitations

RWatch does not bypass Linux permissions.

It cannot guarantee access to every process or every system resource.

PIDFD support depends on Linux kernel and system support. Unsupported
systems may not be able to perform PIDFD-based operations.

RWatch should report failures clearly instead of assuming that
privileged operations will succeed.

## 8. Security Principles

RWatch follows these principles:

1. Monitoring is observational by default.
2. Process-control operations require explicit user action.
3. Invalid process identifiers are rejected.
4. Process identity matters in addition to the PID.
5. Permission errors are handled explicitly.
6. Security claims must be supported by tests.