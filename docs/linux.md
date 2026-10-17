# Linux Internals Used by RWatch

## Overview

RWatch uses Linux interfaces to collect system information
from userspace.

Instead of implementing a separate kernel monitoring mechanism,
RWatch reads information exposed by Linux and converts it into
structured metrics.

The main interfaces include `/proc`, `/sys`, and Linux-specific
system calls.

## The /proc Filesystem

The `/proc` filesystem exposes information about running
processes and kernel-maintained system statistics.

RWatch uses these interfaces for monitoring.

### CPU Statistics

`/proc/stat` contains aggregate CPU statistics and per-CPU
statistics.

CPU usage can be estimated by reading the counters at two
different times and calculating their differences.

Conceptually:

    CPU usage = busy time / total elapsed CPU time * 100

The exact calculation must account for the CPU counters
included in the implementation.

### Memory Statistics

`/proc/meminfo` exposes memory statistics such as:

- MemTotal
- MemFree
- MemAvailable
- Buffers
- Cached

These values help describe memory availability and usage.

`MemAvailable` is generally more useful than `MemFree` alone
when estimating how much memory is available for new workloads.

### Process Information

Linux exposes process-specific information under:

    /proc/<pid>/

RWatch can use these files to inspect process metadata and
resource consumption.

The available information depends on permissions, process
lifetime, and kernel behavior.

Processes may terminate while RWatch is collecting their data.
Consequently, a failed read does not necessarily indicate a
problem with RWatch.

## The /sys Filesystem

The `/sys` filesystem exposes information about devices,
drivers, and kernel subsystems.

Its available contents depend on the running kernel and
the system configuration.

RWatch should handle missing files and unavailable information
without assuming every Linux environment exposes identical data.

## Network Statistics

Linux exposes network interface statistics through:

    /proc/net/dev

The counters can be sampled repeatedly to estimate traffic rates.

For example:

    Receive rate = change in received bytes / elapsed time

    Transmit rate = change in transmitted bytes / elapsed time

Rates depend on the sampling interval and the counters
provided by the kernel.

RWatch also inspects TCP and UDP socket information exposed
through Linux networking interfaces.

## Disk Statistics

Linux exposes block-device statistics through:

    /proc/diskstats

Repeated readings allow RWatch to calculate changes in
read and write counters over time.

These changes can be used to estimate throughput and
input/output operations per second.

The meaning of individual counters must be interpreted
according to the Linux kernel interface.

## Cgroups

Control groups provide mechanisms for organizing processes
and accounting for or limiting resource consumption.

The files and metrics available depend on whether the
system uses cgroup v1 or cgroup v2 and on the permissions
available to the monitoring process.

RWatch must account for these environmental differences.

## PID Reuse and Process Identity

A process ID is not a permanent identity. After a process exits,
Linux may reuse its PID for another process.

RWatch records process start-time information and uses it to
check whether a process still matches the identity expected by
the caller.

For process-control operations, RWatch follows this sequence:

1. Validate the supplied PID.
2. Open a PID file descriptor using `pidfd_open`.
3. Read the current process information.
4. Compare the current start-time ticks with the expected value.
5. Reject the operation if the process identity is stale.
6. Send the signal through `pidfd_send_signal`.

Using a PID file descriptor helps prevent a signal from being
sent to a different process merely because the numeric PID
was reused.

These operations depend on Linux kernel support for PID file
descriptors. RWatch reports unsupported-operation errors when
the relevant system call is unavailable.

## Error Handling and Portability

Linux interfaces can become unavailable because of permissions,
kernel configuration, or environmental differences.

RWatch handles errors differently across collectors.

The CPU collector converts failures to read `/proc/stat` into
a `CpuError::Io` result.

The disk collector currently uses `expect()` when reading
`/proc/diskstats`. The network collector uses the same approach
when reading `/proc/net/dev`.

Consequently, a failure to read either file can cause a panic
in the corresponding collection operation.

These paths are candidates for improved error handling.

The project is developed and tested in WSL2. Native Linux
distributions, containers, and kernel configurations may expose
different capabilities.

## Design Principles

1. Prefer documented Linux interfaces.
2. Treat system information as potentially unavailable.
3. Expect processes and devices to change during collection.
4. Distinguish raw kernel counters from calculated metrics.
5. Validate assumptions about kernel interfaces.
6. Use advanced mechanisms only when they solve a demonstrated
   problem.
