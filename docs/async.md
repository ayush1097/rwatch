# Asynchronous Architecture in RWatch

## Overview

RWatch uses Tokio to coordinate monitoring tasks and process
system information asynchronously.

The goal is to collect information from multiple Linux interfaces
without forcing every monitoring task to run sequentially.

## Concurrent Collectors

RWatch monitors different aspects of the system, including:

- CPU usage
- Memory usage
- Processes
- Disk statistics
- Network interfaces
- Cgroups

These collectors have different sampling intervals and workloads.

Running them as independent tasks allows the application to
coordinate their work without requiring every collector to finish
before another collector can proceed.

## Channels and Application State

Collectors communicate updates through Tokio channels.

Conceptually, the data flow is:

    Linux interfaces
          |
          v
       Collectors
          |
          v
    Bounded Tokio channels
          |
          v
    Central event loop
          |
          v
    Application state
       /         \
      v           v
     TUI       HTTP API

This separates data collection from presentation.

The terminal UI and HTTP API consume application state instead
of independently duplicating the system collection logic.


## Bounded Channels and Backpressure

RWatch uses two bounded Tokio MPSC channels:

- Metrics channel: capacity of 100 events.
- Critical channel: capacity of 32 events.

Collectors send events using asynchronous `.send(...).await`
operations.

When a channel reaches capacity, a sender waits asynchronously
until capacity becomes available or the receiver is closed.
This provides backpressure and prevents the queue from growing
without a fixed bound.

The channels separate ordinary monitoring updates from process
lifecycle events.

### Event Processing Priority

The central event loop uses `tokio::select!` with the `biased;`
directive.

The critical channel is listed before the ordinary metrics
channel. When both receive operations are ready, the critical
branch is preferred.

The critical channel handles process-started and process-exited
events. The ordinary channel updates shared monitoring state.

This priority helps process lifecycle events receive attention,
but it does not guarantee immediate processing. A continuously
ready critical channel can delay ordinary metric updates.

### Design Trade-offs

Bounded channels limit queued messages, but they do not eliminate
all latency or guarantee that every event will eventually be
processed.

Channel capacity, producer rates, and consumer behavior all
affect responsiveness.

## Cancellation and Shutdown

RWatch uses Tokio's `CancellationToken` to coordinate shutdown.

When Ctrl+C is received, the application cancels the shared
shutdown token. Monitoring collectors select on cancellation
alongside their normal monitoring operations.

The main event loop also listens for cancellation. When it
receives the shutdown signal, it exits the event loop and awaits
the CPU, memory, disk, network, process, cgroup, TCP, and UDP
collector tasks.

After these collector tasks finish, the application prints
`RWatch shutdown complete`.

### Shutdown Limitations

Awaiting collector tasks allows the application to observe their
termination, but it does not guarantee that every task will
finish promptly.

A task may be delayed by blocking operations or other work that
does not respond promptly to cancellation.

The UI and HTTP API tasks are spawned separately and are not
awaited by the main shutdown sequence.

These limitations should be considered when evaluating the
application's complete shutdown behavior.

## Blocking Work and Async Rust

An async function is not automatically non-blocking.

Reading Linux information from `/proc` and `/sys` involves
filesystem operations. Such operations can still block the
thread executing them.

This distinction matters when designing Tokio applications.

Blocking work should be evaluated according to its cost and
frequency. Where appropriate, it can be isolated from tasks
that need to remain responsive.

## Design Principles

RWatch follows these principles:

1. Keep Linux data collection separate from presentation.
2. Use channels to communicate between components.
3. Bound queues where predictable resource usage matters.
4. Make cancellation and shutdown behavior explicit.
5. Measure performance rather than assuming asynchronous
   execution is automatically faster.
