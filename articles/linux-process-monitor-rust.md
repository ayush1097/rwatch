# Building a Linux Process Monitor from `/proc` in Rust

## Introduction

Linux exposes detailed information about running processes
through the `/proc` filesystem.

This information includes process identifiers, names, execution
states, memory statistics, and other details maintained by
the kernel.

However, the raw information exposed by Linux is not always
convenient for applications to consume directly.

In RWatch, I use Rust to read this information, parse it into
structured data, and make it available to the monitoring system.

This article explains the underlying Linux interfaces, the
implementation decisions, and the challenges involved in
building a process monitor.

## 1. Understanding the `/proc` Filesystem

The `/proc` filesystem is a virtual filesystem that exposes
information about processes and kernel-maintained system state.

Each process has a directory identified by its process ID:

    /proc/<pid>/

For example:

    /proc/1/
    /proc/100/
    /proc/1234/

These directories contain files that expose different aspects
of a process.

Some important examples include:

- `stat`: process state and numerical statistics.
- `status`: human-readable process and memory information.
- `cmdline`: command-line arguments.
- `fd/`: symbolic links representing open file descriptors.

The files and information available can depend on permissions,
kernel configuration, and whether the process still exists.

## 2. Why Use Rust?

Reading information from `/proc` is only the first part of
building a monitoring tool.

The application must also parse the returned text, handle
unexpected input, manage errors, and represent the collected
information in a useful format.

Rust provides several useful features for this work:

- Strong types for representing process information.
- `Result` and `Option` for handling recoverable errors
  and missing values.
- Ownership and borrowing for managing data safely.
- Testing support for validating parsing logic.

These features help make the boundary between raw Linux
information and structured application data explicit.

## 3. Discovering Processes

Linux exposes information about running processes through
directories inside `/proc`.

RWatch discovers processes by reading the `/proc` directory:

```rust
let entries = match fs::read_dir("/proc") {
    Ok(entries) => entries,
    Err(_) => {
        return processes;
    }
};

The directory entries can include both process directories and
other kernel information. RWatch examines these entries and
attempts to read process information for valid process IDs.

A process can terminate while the monitor is scanning `/proc`.
Therefore, the monitor cannot assume that every discovered
process will remain available throughout collection.

RWatch handles unavailable process information by treating
failed reads as missing data rather than assuming every read
will succeed.

## 4. Collecting Process Information

RWatch reads several files under `/proc/<pid>/` to construct
a structured representation of each process.

### `/proc/<pid>/comm`

This file provides the process name.

RWatch reads the file and removes surrounding whitespace
before storing the name.

### `/proc/<pid>/status`

This file provides human-readable process information.

RWatch extracts the following fields:

- `State`: the process state.
- `PPid`: the parent process ID.
- `VmRSS`: resident memory size, stored in kilobytes.
- `Threads`: the number of threads.

The implementation searches through the file line by line
and parses the values into Rust types.

### `/proc/<pid>/stat`

This file provides numerical process statistics.

RWatch extracts three important values:

- `utime`: CPU time spent executing in user mode.
- `stime`: CPU time spent executing in kernel mode.
- `starttime`: the time at which the process started,
  expressed in clock ticks since boot.

RWatch combines `utime` and `stime` to track the process's
accumulated CPU time.

Parsing this file requires care because the process name
appears inside parentheses and can contain spaces. RWatch
uses the last closing parenthesis to locate the fields
that follow the process name.

This avoids incorrectly splitting the process name using
whitespace alone.

## 5. Representing Processes with Rust Types

Instead of passing raw text throughout the application,
RWatch represents collected information using a
`ProcessInfo` structure.

Its fields include:

- Process ID and name.
- Process start time and state.
- Parent process ID.
- Resident memory usage.
- Thread count.
- User-mode and kernel-mode CPU time.
- Calculated CPU usage.
- Namespace information.
- Resource-limit information.

This structure provides a consistent representation
that other parts of the monitoring system can consume.

It also derives `Serialize`, allowing process information
to be serialized for supported output formats.

Namespace and resource-limit information are collected
through separate functions, keeping those responsibilities
outside the main process-parsing logic.

## 6. Calculating Per-Process CPU Usage

Reading `/proc/<pid>/stat` provides accumulated CPU time,
but accumulated time alone does not tell us how much CPU
a process uses during a particular interval.

RWatch calculates CPU usage by comparing consecutive samples.

### Step 1: Record the initial CPU times

When `ProcessMonitor` is initialized, it collects the
currently available processes.

For each process, it records:

    process_cpu_ticks = utime + stime

It also records the total system CPU ticks.

These values become the baseline for the next sample.

### Step 2: Collect another sample

When `sample()` is called, RWatch collects process
information again and reads the total system CPU ticks.

It calculates the difference between the current and
previous system CPU ticks:

    system_tick_delta = current_system_ticks
                        - previous_system_ticks

For each process observed in both samples, it calculates:

    process_tick_delta = current_process_ticks
                         - previous_process_ticks

These differences represent CPU time accumulated during
the sampling interval.

### Step 3: Calculate CPU usage

RWatch calculates per-process CPU usage using:

    cpu_usage = (process_tick_delta / system_tick_delta)
                * 100
                * cpu_count

Here, `cpu_count` is the number of logical CPUs detected
by RWatch.

Multiplying by 100 converts the ratio into a percentage.
Multiplying by the CPU count expresses usage relative to
one logical CPU rather than distributing the result
across all logical CPUs.

Consequently, a process can report more than 100% CPU
usage when it uses multiple CPUs during the sampling
interval.

### Step 4: Update the baseline

After calculating usage, RWatch replaces the previous
per-process CPU tick values with the current values.

It also updates the previous system CPU tick count.

The next call to `sample()` can then calculate usage
over the following interval.

### Handling new processes and counter differences

A process that was not present in the previous sample
has no previous CPU-time baseline in `previous_ticks`.
Its CPU usage therefore remains at the initial value
of `0.0` for that sample.

RWatch also uses `saturating_sub()` when calculating
tick differences. This prevents unsigned subtraction
from underflowing if a current counter is smaller than
its previous value.

A zero system tick delta prevents the CPU percentage
calculation from running for that sample.

## 7. Detecting Process Lifecycle Events

A monitoring tool can provide more useful information
than a list of currently running processes. It can also
identify processes that appear or disappear between scans.

RWatch implements this functionality through
`detect_lifecycle_events()`.

The function compares two sets of information:

- `previous`: a `HashMap` containing previously observed
  process IDs and their names.
- `current`: a slice containing the latest `ProcessInfo`
  records.

### Detecting newly observed processes

RWatch iterates over the current processes.

If a process ID does not exist in the previous map,
the function generates a `Started` event containing
the process ID and name.

### Detecting disappeared processes

RWatch then examines the previously observed processes.

If a previous process ID is absent from the current
sample, the function generates an `Exited` event.

A `HashSet` of current process IDs makes these membership
checks straightforward.

The resulting events distinguish newly observed processes
from processes that are no longer present in the latest
scan.

One important limitation is that this comparison identifies
changes between observations. A process that starts and
exits entirely between scans may never be observed.

## 8. Handling Errors and Race Conditions

The `/proc` filesystem represents live kernel state.
Its contents can change while an application is reading them.

For example, a process might terminate after RWatch discovers
its directory but before it reads `/proc/<pid>/status`.

The file read can then fail because the process no longer
exists.

RWatch's `read_process()` function returns `Option<ProcessInfo>`.
If a required file cannot be read or a required value cannot
be parsed, the function returns `None`.

This prevents an incomplete process record from being
constructed when required information is unavailable.

However, silently returning `None` also means that the caller
does not receive the precise reason for the failure.

This is an important design trade-off: the collector can
continue operating despite individual process-read failures,
but richer error reporting could help with debugging and
diagnostics.

## Conclusion

Building a process monitor involves more than reading files
from `/proc`.

RWatch combines several Linux interfaces, parses their
contents into structured Rust types, calculates CPU usage
from successive samples, and detects process lifecycle
changes.

The implementation also has to account for the fact that
processes can disappear while collection is in progress.

These challenges illustrate an important aspect of Linux
systems programming: applications must interpret kernel
information carefully while remaining resilient to changing
system state.
