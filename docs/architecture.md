# RWatch Architecture

## 1. Overview

RWatch is a Linux monitoring application written in Rust.

It collects system information, processes that information into
useful metrics, and exposes the results through a terminal UI
and HTTP API.

The project explores Linux internals, asynchronous programming,
systems programming, and resource monitoring.

## 2. High-Level Architecture

The current application follows this general data flow:

Linux System Interfaces
        |
        v
Linux Monitoring Modules
        |
        v
Metric Events
        |
        v
Bounded Tokio MPSC Channels
        |
        v
Central Event Loop
        |
        v
Shared Application State
        |
        +------------------+
        |                  |
        v                  v
    Terminal UI        HTTP API

The application also supports a JSON snapshot mode.

## 3. Linux Monitoring Modules

RWatch organizes monitoring functionality into separate modules.

Examples include:

- CPU monitoring
- Memory monitoring
- Disk monitoring
- Network monitoring
- Process monitoring
- Cgroup discovery
- TCP connection inspection
- UDP socket inspection

The modules are exposed through the library interface in `lib.rs`.

Linux-specific data collection is implemented in the corresponding
monitoring modules.

## 4. Collectors and Asynchronous Execution

The application uses Tokio to run independent monitoring tasks.

Collectors periodically obtain system information and send results
to the central event loop.

The main application currently starts tasks for:

- CPU
- Memory
- Disk
- Network
- Processes
- Cgroups
- TCP connections
- UDP sockets

Sampling intervals are configured or defined by the application.

Collectors use cancellation tokens to respond to application
shutdown.

## 5. Event Communication

Collectors communicate with the central event loop through
Tokio MPSC channels.

The main application uses bounded channels.

The ordinary metrics channel carries events such as CPU usage,
memory information, disk rates, network rates, process snapshots,
and connection information.

A separate channel carries process lifecycle events.

This separation allows process lifecycle events to be handled
independently of ordinary metric updates.

## 6. Process Lifecycle Detection

The process collector periodically samples the running processes.

It compares the current process snapshot with the previous snapshot
to detect processes that have started or exited.

The detected events are sent through a dedicated channel.

This is snapshot-based detection. It should not be described as
kernel event-driven process monitoring.

## 7. Shared Application State

`RWatchState`, defined in `state.rs`, stores the latest collected
results.

The state includes:

- CPU usage
- Memory information
- Disk rates
- Network rates
- Process information
- Cgroup information
- TCP rows and connections
- UDP rows

The fields are optional because data may not be available when
the application first starts or when collection fails.

The main application shares this state through
`Arc<RwLock<RWatchState>>`.

## 8. Presentation and JSON Output

RWatch provides a terminal interface and an HTTP API.

The terminal UI uses Ratatui and Crossterm.

The HTTP API uses Axum.

The application also supports a JSON snapshot mode and provides
an `export_json()` method for writing application state to a file.

## 9. Error Handling and Reliability

System information can change while it is being collected.

Processes may exit, system files may be inaccessible, and
some information may be unavailable because of permissions
or kernel configuration.

Collection failures should be handled without unnecessarily
terminating the entire monitoring application.

## 10. Shutdown

The application listens for Ctrl+C.

A cancellation token signals tasks that support graceful shutdown.

The main task waits for collector tasks to finish before
reporting that shutdown is complete.

## 11. Security

RWatch should remain read-only by default.

Operations that affect processes must be explicit and carefully
validated.

See [security.md](security.md) for the project's security model.

## 12. Testing and Performance

Tests should cover parsing, error handling, and monitoring behavior.

Benchmarks should measure specific operations and report results
without making unsupported performance claims.

## 13. Design Principles

- Keep Linux-specific behavior understandable.
- Prefer reliable measurements over unsupported performance claims.
- Handle permission errors and disappearing processes.
- Use bounded channels where appropriate.
- Keep monitoring read-only by default.
- Test changes before integrating them into the main application.
- Introduce advanced kernel mechanisms only when they solve
  a demonstrated problem.