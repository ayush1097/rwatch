# What Tokio Gives You on Linux

## Introduction

RWatch monitors several aspects of a Linux system, including
CPU utilization, memory, disk activity, network traffic,
processes, cgroups, and socket information.

Collecting all this information involves repeatedly reading
system interfaces, processing the results, and making the
latest metrics available to the rest of the application.

Doing this in one sequential loop would tightly couple
the collection and processing of different metrics.

RWatch uses Tokio to organize asynchronous tasks, coordinate
metric delivery, and manage shared application state.

This article explains how these components fit together
and what asynchronous Rust does—and does not—provide.

## 1. Why Use Asynchronous Tasks?

Different metrics are collected at regular intervals.

For example, a CPU collector can wait for its next timer
tick while other tasks handle memory, disk, or network data.

Tokio allows the application to maintain multiple tasks
without requiring a separate operating-system thread
for every monitoring task.

Tasks can make progress when other tasks are waiting
on asynchronous operations.

This is particularly useful when an application manages
several independent collection loops.

However, asynchronous programming does not automatically
make every operation faster or nonblocking.

The benefit depends on the operations performed by
each task.

## 2. Organizing Metrics as Events

RWatch represents collected metrics using the `MetricEvent`
enum.

Its variants include:

- `Cpu`
- `Memory`
- `Disk`
- `Network`
- `Processes`
- `Cgroups`
- `Tcp`
- `Udp`

Each variant carries the corresponding collected data.

For example, a CPU event carries a floating-point value,
while a process event carries a vector of `ProcessInfo`
records.

This design gives the application a common type for
communicating results from different collectors.

Instead of requiring every collector to update the
application's state directly, a collector can send an
event to a channel.

The receiving component can then process that event.

## 3. Using Bounded Channels

RWatch creates a regular metric channel:

```rust
let (sender, mut receiver) = mpsc::channel(100);
```

The channel has a capacity of 100 messages.

It also creates a separate channel for critical events:

```rust
let (critical_sender, mut critical_receiver) =
    mpsc::channel::<MetricEvent>(32);
```

This channel has a capacity of 32 messages.

A bounded channel limits how many messages can wait
in the queue at one time.

When a sender attempts to send a message to a full
channel using `send().await`, it waits asynchronously
until capacity becomes available or the receiver
is closed.

This provides backpressure: producers cannot continue
adding messages to a full queue without waiting.

Backpressure can help prevent an unbounded backlog
if a consumer temporarily processes messages more
slowly than producers generate them.

However, bounded capacity alone does not guarantee
that every event will eventually be processed.
That also depends on how producers, consumers, and
shutdown are managed.

## 4. Separating Collection from State Updates

A collector gathers information and sends it through
a channel.

The receiving component processes the event and updates
the shared monitoring state.

RWatch uses a `RwLock` around its `RWatchState`:

```rust
Arc<RwLock<state::RWatchState>>
```

The `Arc` allows multiple tasks to hold shared ownership
of the state.

The asynchronous `RwLock` coordinates access to the
state between tasks.

For example, when a CPU event is received, the state
can be updated with the latest CPU utilization value.

This separates the responsibility of collecting metrics
from the responsibility of maintaining the latest
application state.

The UI and API can then access shared monitoring data
without each collector independently managing every
consumer.

## 5. Coordinating Tasks with `tokio::select!`

RWatch uses `tokio::select!` in its collection loops.

For example, a collector can wait for either a timer
tick or a cancellation request.

The timer branch performs collection work, while the
cancellation branch allows the task to exit.

The same macro can coordinate other asynchronous
operations, such as receiving messages and observing
shutdown.

This is useful when a task must respond to more than
one event source.

However, `tokio::select!` does not run its branches
simultaneously. It waits for a branch to become ready
and executes that branch within the task.

The other branch futures are cancelled when a branch
wins, so developers must consider whether cancellation
is safe for the operations involved.

## 6. Prioritizing Critical Events

RWatch has a separate channel for process lifecycle
events, such as process starts and exits.

Its main event loop uses:

```rust
tokio::select! {
    biased;

    // Critical events are handled first when ready.
    // Regular metric events are handled in another branch.
}
```

The `biased;` directive makes branch polling follow
the order in which branches appear.

Because the critical channel is listed first, a ready
critical event is preferred when both channel branches
are ready.

This can help lifecycle events receive prompt attention.

However, prioritization has a trade-off: if the first
branch remains continuously ready, later branches may
be delayed.

Separating critical and ordinary events is therefore
a scheduling policy, not a guarantee that critical
events will always be processed immediately.

## 7. What Tokio Does Not Make Nonblocking

RWatch reads Linux interfaces such as `/proc` and `/sys`.

These reads are ordinary filesystem operations.
Using an asynchronous runtime does not automatically
turn them into nonblocking filesystem operations.

For example, code that calls:

```rust
fs::read_to_string("/proc/stat")
```

performs a synchronous read.

If that operation takes time, it occupies the thread
executing it until the call returns.

Tokio can coordinate asynchronous tasks, timers, and
channel operations, but synchronous work can still
occupy a runtime worker thread.

This distinction is important when designing a monitoring
application.

Asynchronous architecture helps coordinate concurrent
activities, but blocking work must still be considered
when evaluating responsiveness and scalability.

## 8. Cancellation and Task Management

RWatch uses `CancellationToken` to communicate shutdown
requests to collectors.

A collector can wait for cancellation alongside its
normal timer or collection operations.

When cancellation is observed, the task can exit its loop.

RWatch also retains collector task handles and awaits
them during shutdown.

This illustrates two separate responsibilities:

- Cancellation communicates that a task should stop.
- Awaiting a task handle allows the application to wait
  for the task's completion.

Both matter when coordinating the shutdown of multiple
background tasks.

## Conclusion

Tokio provides RWatch with tools for organizing asynchronous
tasks, transferring metrics through bounded channels,
coordinating operations with `select!`, and managing
cancellation.

The application combines these tools with shared state
to collect and present information from Linux interfaces.

The important lesson is that an asynchronous runtime
provides coordination mechanisms, not automatic performance
improvements for every operation.

Understanding channels, task scheduling, cancellation,
and synchronous filesystem work helps developers make
better architectural decisions when building monitoring
systems in Rust.
