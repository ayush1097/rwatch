# Monitoring Linux cgroups from Rust

## Introduction

A Linux system can run many processes simultaneously. Sometimes, we need to
understand not only individual processes but also how groups of processes
consume system resources.

Linux provides control groups, commonly called cgroups, to organize processes
and apply resource controls.

RWatch monitors cgroups by reading files under `/sys/fs/cgroup`. It uses Rust
to discover cgroup directories, parse resource statistics, and represent the
results in structured data.

This article explains how the implementation works and discusses its
limitations.

## 1. What Are Linux cgroups?

A cgroup is a Linux mechanism for grouping processes and tracking or controlling
their resource consumption.

For example, a cgroup can have:
- A memory usage value and memory limit.
- CPU usage statistics and CPU quota settings.
- A count of processes and a maximum process limit.
- Child cgroups that form a hierarchy.

Cgroups are useful for containers, system services, and other workloads that
need resource accounting or isolation.

RWatch reads the cgroup filesystem to observe this information.

## 2. Where Linux Exposes cgroup Information

RWatch uses the following directory as its root:

`/sys/fs/cgroup`

The implementation assumes the cgroup files and layout it reads are available
there, as they commonly are with cgroup v2.

Different files expose different kinds of information:

| File | Information |
|---|---|
| `cgroup.procs` | Process IDs belonging to a cgroup |
| `memory.current` | Current memory usage in bytes |
| `memory.max` | Memory limit, or `max` if unlimited |
| `cpu.max` | CPU quota and scheduling period |
| `cpu.stat` | CPU usage and user/system time |
| `pids.current` | Current process count |
| `pids.max` | Maximum permitted process count, or `max` |

These files provide the raw values that RWatch parses.

## 3. Discovering cgroups Recursively

The public entry point is:

`discover_cgroups()`

It starts at `/sys/fs/cgroup` and calls `discover_recursive()`.

The recursive function:
1. Reads the entries in the current directory.
2. Ignores entries that are not directories.
3. Builds a cgroup path relative to the cgroup root.
4. Counts the cgroup and its subgroups.
5. Attempts to read the cgroup's resource information.
6. Recursively explores its child directories.

The function returns:

`io::Result<(Vec<Cgroup>, usize, usize)>`

The three values represent the discovered cgroup records, the total number
of cgroups encountered, and the number of sub-cgroups encountered.

A cgroup is added to the results only if `read_cgroup()` succeeds. If reading
a cgroup fails, the implementation logs the error and continues exploring
its children.

This allows discovery to continue when an individual cgroup cannot be read.

## 4. Representing cgroup Information in Rust

RWatch stores the collected information in a `Cgroup` structure.

Its fields include:

- `path`: cgroup path.
- `pids`: process IDs read from `cgroup.procs`.
- `parent`: parent cgroup path, if present.
- `memory_current`: current memory usage.
- `memory_max`: configured memory limit, if readable.
- `cpu_usage_usec`: total CPU usage in microseconds.
- `cpu_user_usec`: user CPU time in microseconds.
- `cpu_system_usec`: system CPU time in microseconds.
- `cpu_quota`: configured CPU quota, if available.
- `cpu_period_usec`: CPU quota period in microseconds.
- `pids_current`: current process count.
- `pids_max`: configured process limit, if available.

Optional fields use Rust's `Option<T>` where a value may be unavailable or
unlimited.

The structure derives `Serialize`, allowing it to be serialized by compatible
serialization libraries.

## 5. Reading Memory Usage and Limits

RWatch reads `memory.current` to determine current memory consumption.

The value is parsed as a `u64`:

`parse_memory_current("10485760")`

This produces `10485760`, representing bytes.

The `memory.max` file can contain a numeric limit or the string `max`.

For example:

`52428800`

represents a limit of 50 MiB.

But:

`max`

means that this cgroup has no finite memory limit configured through this
setting.

The parser represents these cases as:

- `Some(limit)` for a numeric limit.
- `None` for `max`.

This distinction is important: an unlimited setting is different from a
configured limit of zero.

## 6. Understanding CPU Usage and Quotas

RWatch reads two CPU-related files.

### cpu.stat

The `cpu.stat` file provides cumulative CPU accounting values.

RWatch extracts:

- `usage_usec`: total CPU usage.
- `user_usec`: CPU time spent in user mode.
- `system_usec`: CPU time spent in system mode.

The parser requires all three fields. Missing or invalid required values
produce an `io::Error`.

These values are cumulative accounting counters, not CPU percentages.
Calculating a percentage requires comparing samples over time and defining
the normalization used.

### cpu.max

The `cpu.max` file contains a quota and a period.

For example:

`20000 100000`

means a quota of 20,000 microseconds per 100,000-microsecond period.

The quota can also be `max`, indicating no finite CPU quota through this
setting.

RWatch represents the quota and period separately so other parts of the
application can interpret the configuration.

A CPU quota is a resource-control setting; it is not itself a measurement of
current CPU utilization.

## 7. Tracking Process Counts

RWatch reads two additional files.

### pids.current

This reports the current number of tasks counted by the cgroup's PID
controller.

### pids.max

This provides the configured process limit or `max` for an unlimited setting.

The parser converts the current count to `u64` and represents the maximum
as `Option<u64>`.

RWatch also reads `cgroup.procs` to obtain process IDs.

The parser:
1. Splits the file into lines.
2. Trims whitespace.
3. Skips empty lines.
4. Parses each remaining line as a `u32`.
5. Ignores PID zero.

The resulting list is stored in the `pids` field.

The PID list and `pids.current` are different pieces of information: one is
a list of IDs read from `cgroup.procs`, while the other is a kernel-reported
count.

## 8. Error Handling and Partial Results

The implementation uses Rust's `io::Result` for operations that can fail.

For example, directory traversal can fail, files may be inaccessible, and
contents may contain invalid values.

The recursive discovery code logs a failed cgroup read and continues with
other directories.

Some fields have more permissive handling:
- `memory.current` becomes `None` if reading or parsing fails.
- `memory.max`, `cpu.max`, and PID-limit information fall back to default
  values when their reader functions fail.
- A failure to read or parse `cpu.stat` causes that cgroup's read operation
  to fail.

These behaviors mean a missing value does not always mean the kernel reports
zero or that a resource is unlimited. It may also reflect a read failure.

A production monitoring tool should make this distinction explicit when
presenting incomplete data.

## 9. Tests for Parsing

The implementation includes tests for:
- Numeric and invalid memory usage values.
- Numeric memory limits and the `max` setting.
- CPU quotas and unlimited CPU quota settings.
- Required CPU statistics and missing fields.
- PID counts and PID limits.
- Process IDs from `cgroup.procs`.

Parser tests are valuable because they check the interpretation of Linux
file formats without requiring every test to read the live cgroup filesystem.

They also help catch regressions when parsing logic changes.

## 10. Limitations and Future Improvements

The current implementation has several limitations worth understanding.

### Assumed cgroup layout

The code reads paths beneath `/sys/fs/cgroup` and expects files such as
`memory.current`, `cpu.max`, and `pids.current`. It does not detect and
support every possible cgroup version or layout.

### Permission and availability issues

Some cgroups may not be readable. The implementation can skip cgroups whose
required reads fail, so the returned list may be incomplete.

### Error visibility

Several optional readers fall back to default values. This can hide the
difference between an unlimited setting, a default value, and a read failure.

### Live filesystem races

Cgroups and processes can change while discovery is running. A directory
that exists during enumeration may disappear before its files are read.

### CPU percentages

The collected CPU counters are cumulative microsecond values. The fields
alone do not provide a normalized CPU percentage; that requires sampling
deltas and choosing a normalization strategy.

These are useful areas for future improvements, especially if RWatch is to
be used for production monitoring.

## Conclusion

RWatch uses Linux's cgroup filesystem to discover resource groups and collect
memory, CPU, and process information.

Rust structures represent the collected data, parser functions validate
file contents, and recursive traversal builds a view of the cgroup hierarchy.

This approach demonstrates how a Linux monitoring tool can turn kernel
interfaces into structured resource information while making error handling,
partial results, and platform assumptions explicit.
