# Designing Cancellation in Async Rust

## Introduction

An asynchronous application often runs several tasks
concurrently.

A monitoring system such as RWatch may have separate tasks
collecting CPU, memory, disk, network, process, and other
system information.

These tasks must continue running while monitoring is active.
However, when the user requests shutdown, they should stop
without unnecessarily delaying the application.

In asynchronous Rust, cancellation requires more than
terminating the main function. The application must
communicate the shutdown request to its tasks and decide
how those tasks should respond.

RWatch uses Tokio and `CancellationToken` to coordinate
this process.

This article explains the cancellation design and the
trade-offs involved in shutting down an asynchronous
monitoring system.

## 1. Why Cancellation Matters

Consider a monitoring application with several background
collectors.

Each collector periodically reads information from Linux
interfaces such as `/proc` and `/sys`.

Without a shutdown mechanism, a collector could remain
inside its loop even after the user requests that the
application stop.

Cancellation provides a way to communicate that the
application is shutting down.

A well-designed cancellation mechanism should help tasks
stop at appropriate points and allow the application to
manage their completion.

## 2. Using CancellationToken

RWatch uses `CancellationToken` from Tokio's utilities.

The application creates a token:

```rust
let shutdown = CancellationToken::new();
```

It then clones the token for tasks that need to respond
to the shutdown request.

Cloned tokens refer to the same cancellation state.
When cancellation is requested through one of these tokens,
the other clones can observe that request.

This allows the application to communicate shutdown
without requiring every task to share a mutable flag.

## 3. Handling Ctrl+C

RWatch starts a task that waits for the user to press Ctrl+C:

```rust
let shutdown_signal = shutdown.clone();

tokio::spawn(async move {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for Ctrl+C");

    shutdown_signal.cancel();
});
```

When the signal handler completes, it calls `cancel()`.

This requests cancellation for tasks that share the token.

The signal handler does not directly terminate every
collector. Instead, each task is responsible for observing
the cancellation request and responding appropriately.

This separation keeps the shutdown trigger independent
from the individual collectors.

## 4. Making Collectors Cancellation-Aware

RWatch's CPU collector uses `tokio::select!` to wait for
either a timer tick or a cancellation request:

```rust
tokio::select! {
    _ = ticker.tick() => {
        // Collect and send a CPU sample.
    }
    _ = shutdown.cancelled() => {
        tracing::info!(
            collector = "cpu",
            "collector shutting down"
        );
        break;
    }
}
```

The timer branch performs the regular collection work.

The cancellation branch logs the shutdown event and
breaks out of the loop.

This pattern allows the collector to stop waiting for
the next timer tick when cancellation is observed.

Other RWatch collectors use similar cancellation-aware
select expressions.

## 5. Cancellation Is Cooperative

Cancellation does not forcibly interrupt arbitrary
synchronous code.

A task must reach a point where it observes the
cancellation request or awaits a cancellation-aware
operation.

For example, if a collector is waiting in a `select!`
between a timer and `shutdown.cancelled()`, it can
respond when the cancellation branch becomes ready.

However, synchronous work already being executed is
not automatically interrupted by the cancellation token.

This distinction matters for monitoring applications
because reading and parsing Linux files can involve
ordinary filesystem operations.

A cancellation-aware loop improves responsiveness, but
it does not guarantee that every operation can be
interrupted immediately.

## 6. Stopping the Main Event Loop

RWatch's main event loop also observes the shared
shutdown token.

When cancellation is observed, the event loop exits.

After leaving the loop, the application awaits its
collector task handles:

```rust
let _ = cpu_handle.await;
let _ = memory_handle.await;
let _ = disk_handle.await;
let _ = network_handle.await;
let _ = process_handle.await;
let _ = cgroup_handle.await;
let _ = tcp_handle.await;
let _ = udp_handle.await;
```

Awaiting these handles allows the application to wait
for those collector tasks to finish.

The return values are intentionally ignored here, so
this code does not report task join failures.

The important distinction is that cancellation requests
tasks to stop, while awaiting their handles gives the
application an opportunity to wait for their completion.

## 7. Draining Pending Metric Events

RWatch also coordinates metric events through a channel.

When the coordinator observes cancellation, it attempts
to process events that are already queued:

```rust
while let Ok(event) = receiver.try_recv() {
    update_state_with_event(&state, event).await;
}
```

It then exits its loop.

This helps preserve queued metric updates instead of
immediately abandoning every event waiting in the channel.

However, this is a best-effort drain of events available
at that point. It does not guarantee that every event
from every producer has been processed.

The behavior depends on which events have already arrived
and whether producers are still active.

## 8. Shutdown Is More Than Cancelling Tasks

RWatch's shutdown sequence illustrates several separate
responsibilities:

1. Detect the shutdown request.
2. Notify tasks through the cancellation token.
3. Allow cancellation-aware loops to exit.
4. Stop the main event loop.
5. Await the collector task handles.

These operations are related, but they are not identical.

For example, a task may receive a cancellation request
while it is performing work that cannot be interrupted
immediately.

Similarly, awaiting a task handle does not itself request
cancellation. The task must have a way to finish.

RWatch also starts a separate UI task and stores its
handle. The collector handles are explicitly awaited
in the shutdown sequence described above, but the same
sequence does not explicitly await the UI handle.

This is an area to consider when evaluating how completely
the application coordinates shutdown across all its tasks.

## Conclusion

Cancellation is an important part of building reliable
asynchronous applications.

RWatch uses a shared `CancellationToken` to communicate
shutdown requests, `tokio::select!` to let collectors
observe cancellation, and task handles to wait for
collector completion.

The design demonstrates that graceful shutdown is not
a single operation. It involves communicating intent,
allowing tasks to respond, and coordinating their exit.

Understanding these distinctions is useful when designing
any asynchronous Rust application that manages multiple
long-running tasks.
