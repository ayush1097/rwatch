# Why a Process Monitor Needs Backpressure

## Introduction

RWatch collects information from several Linux subsystems, including CPU,
memory, disks, networks, processes, cgroups, TCP, and UDP.

These collectors operate asynchronously and send their results to a central
event loop.

But what happens when collectors produce events faster than the event loop
can process them?

Without a way to control the flow of data, queued messages could consume
increasing amounts of memory. RWatch addresses this problem using bounded
Tokio channels.

This article explains backpressure, bounded channels, event prioritization,
and the trade-offs involved in this design.

## 1. The Producer-Consumer Problem

RWatch has two main roles:

- **Producers:** collectors that gather system information and publish events.
- **Consumer:** the main event loop that receives events and updates
  application state.

For example, a CPU collector reads CPU statistics and sends a
`MetricEvent::Cpu` event. The consumer receives that event and updates
the CPU information in shared application state.

Ideally, producers and consumers operate at compatible speeds.

However, the consumer may temporarily fall behind because it is processing
other events or performing additional work.

A monitoring application must handle this situation without allowing
an unbounded queue to grow indefinitely.

## 2. What Is Backpressure?

Backpressure is a mechanism that slows producers when downstream consumers
cannot keep up.

Imagine a collector producing 100 events per second while the consumer
processes only 70 events per second.

If the queue has no effective limit, the backlog can grow over time.

With a bounded channel, the queue has a fixed capacity. Once it fills,
a producer that awaits channel capacity must wait until the consumer
receives messages.

This creates a feedback mechanism: a slower consumer can slow down
the producers.

Backpressure does not make the consumer faster. It controls how the system
behaves when production exceeds consumption.

## 3. RWatch Uses Bounded Channels

RWatch creates two channels in `src/main.rs`:

```rust
let (sender, mut receiver) = mpsc::channel(100);

let (critical_sender, mut critical_receiver) =
    mpsc::channel::<MetricEvent>(32);
```

The channels have different purposes.

### Regular metrics channel

The regular channel has a capacity of 100 messages.

Collectors send events such as:

- `MetricEvent::Cpu`
- `MetricEvent::Memory`
- `MetricEvent::Disk`
- `MetricEvent::Network`
- `MetricEvent::Processes`
- `MetricEvent::Cgroups`
- `MetricEvent::Tcp`
- `MetricEvent::Udp`

The consumer receives these events and updates the corresponding application
state.

### Critical events channel

The critical channel has a capacity of 32 messages.

RWatch uses it for process lifecycle events, including:

- `MetricEvent::ProcessStarted`
- `MetricEvent::ProcessExited`

The consumer logs these events separately from ordinary resource metrics.

Separating the channels allows RWatch to give lifecycle events different
handling from regular metric updates.

The capacities are design choices, not guarantees that the application can
never run out of memory. Other allocations and tasks also consume resources.

## 4. Why Use `.send().await`?

RWatch's collectors send events using asynchronous channel sends.

For example:

```rust
sender.send(MetricEvent::Cpu(usage)).await
```

The `send` operation returns a future.

If the channel has capacity, the message can be sent. If the channel is full,
the sender waits asynchronously for capacity to become available.

This is different from repeatedly trying to send messages without waiting.

The `.await` allows the Tokio task to yield while waiting instead of
synchronously blocking the thread on channel capacity.

When a send fails because the receiver has been closed, the operation returns
an error. RWatch's collector code checks for send failures and exits the
relevant loop.

Backpressure therefore affects both performance and shutdown behavior.

## 5. Prioritizing Critical Events

RWatch uses `tokio::select!` to receive events from both channels.

The main event loop contains:

```rust
tokio::select! {
    biased;

    Some(event) = critical_receiver.recv() => {
        // Handle critical events.
    }

    Some(event) = receiver.recv() => {
        // Handle regular metrics.
    }
}
```

The `biased;` directive makes Tokio check the branches in the order in which
they appear.

When both receive operations are ready, the critical channel's branch is
checked first.

This gives critical events priority over ordinary metric events.

However, priority does not mean that every critical event is guaranteed
immediate processing. A busy critical channel can delay regular metrics,
and the event loop must still process events one at a time.

This is a trade-off between responsiveness to important events and fairness
between event sources.

## 6. What Happens When a Channel Fills?

Consider the regular metrics channel.

1. Collectors publish events.
2. The consumer receives events and updates application state.
3. If the consumer falls behind, queued messages accumulate.
4. Once the channel reaches its capacity, awaited sends must wait for space.
5. As the consumer receives messages, producers can send again.

This limits the number of messages waiting in that channel.

But bounded channels do not solve every overload problem.

For example, if a collector is delayed by a full channel, its next collection
may happen later than intended. The resulting metrics can become less timely.

The capacity also does not limit the size of every message or the memory
used elsewhere in the application.

Backpressure is one component of resource management, not a complete
overload-control strategy.

## 7. Backpressure and Shutdown

RWatch also uses cancellation tokens to coordinate shutdown.

Collectors select between their collection timers and cancellation signals.
When cancellation is received, they can stop collecting and leave their loops.

However, an awaited channel send can itself be waiting for capacity.

A robust cancellation design should consider what happens if a collector
is waiting to send while shutdown begins.

One possible pattern is to select between sending and cancellation:

```rust
tokio::select! {
    result = sender.send(event) => {
        if result.is_err() {
            break;
        }
    }

    _ = shutdown.cancelled() => {
        break;
    }
}
```

This is an illustrative pattern, not a claim that every RWatch send currently
uses it. The event must also be available to the send branch without being
moved into multiple branches.

Cancellation-aware sending can prevent a task from remaining indefinitely
waiting for channel capacity during shutdown.

## 8. Design Trade-offs

Backpressure introduces several trade-offs.

### Benefits

- Limits queued messages per channel.
- Prevents an unlimited backlog in those queues.
- Slows producers when consumers fall behind.
- Provides explicit capacity limits.
- Allows critical events to be handled separately.

### Costs

- Producers may wait when channels are full.
- Collection intervals may become less predictable.
- Prioritizing one channel can delay another.
- Queue capacity requires a deliberate design choice.
- Backpressure does not eliminate blocking filesystem operations or
  expensive consumer work.

A monitoring tool must balance data freshness, responsiveness, memory usage,
and reliability.

## Conclusion

RWatch uses bounded Tokio channels to connect asynchronous collectors to
a central event loop.

The regular channel has a capacity of 100 messages, while the critical
channel has a capacity of 32. Awaited sends provide backpressure, and the
biased receive loop prioritizes critical events when multiple branches
are ready.

These choices help control queued work, but they also introduce trade-offs
around latency, fairness, and shutdown.

Understanding these trade-offs is essential when building reliable
asynchronous monitoring software.
