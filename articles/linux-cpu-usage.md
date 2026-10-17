# How CPU Usage Is Actually Calculated on Linux

## Introduction

CPU usage is one of the most common metrics displayed by
system monitoring tools.

However, calculating CPU usage is not as simple as reading
a single value from the Linux kernel.

Linux exposes cumulative CPU time counters through
`/proc/stat`. Monitoring applications can use these counters
to calculate CPU utilization over a period of time.

In RWatch, CPU monitoring involves reading these counters,
parsing them into Rust types, comparing consecutive samples,
and calculating the percentage of CPU time that was busy.

This article explains the Linux interface, the calculation,
and the implementation decisions behind RWatch's CPU monitor.

## 1. Understanding `/proc/stat`

Linux exposes aggregate CPU statistics through:

    /proc/stat

The file contains lines representing CPU time accumulated
by the system.

For example:

    cpu  1000 100 500 8000 200 50 25 10 0 0

The first line, beginning with `cpu`, contains aggregate
statistics across the logical CPUs.

Individual logical CPUs can also have their own entries,
such as `cpu0` and `cpu1`.

The counters represent accumulated CPU time measured in
units of `USER_HZ` clock ticks. They are not instantaneous
CPU percentages.

RWatch reads `/proc/stat` and processes the first line,
which begins with the aggregate `cpu` label.

## 2. Which CPU Counters Does RWatch Use?

Linux exposes several CPU time categories.

RWatch currently extracts five of them:

- `user`: time spent executing normal processes in user mode.
- `nice`: time spent executing niced processes in user mode.
- `system`: time spent executing in kernel mode.
- `idle`: time spent idle.
- `iowait`: time waiting for I/O.

These values are stored in the `CpuTimes` structure:

    user
    nice
    system
    idle
    iowait

Linux exposes additional fields, including interrupt time,
steal time, and guest time. The current RWatch calculation
does not include those fields.

This distinction matters because the counters included in
the calculation determine how the resulting percentage
should be interpreted.

## 3. Parsing CPU Statistics with Rust

The `read_cpu_times()` function reads `/proc/stat`:

    let contents = fs::read_to_string("/proc/stat")
        .map_err(|_| CpuError::Io)?;

It then passes the contents to `parse_cpu_times()`.

The parser reads the first line and splits it into
whitespace-separated fields.

It skips the first field, which contains the `cpu` label,
and parses the next five values into `u64` integers.

The parser uses explicit error variants to distinguish
different failures, including:

- `NoData`: the input contains no lines.
- `MissingCpuLabel`: the first line has no label.
- `MissingUser`: the user counter is absent.
- `InvalidUser`: the user counter cannot be parsed.
- `MissingIdle`: the idle counter is absent.

Equivalent missing-value and invalid-value errors exist
for the other counters that RWatch reads.

Returning `Result<CpuTimes, CpuError>` makes parsing
failures explicit instead of silently treating malformed
input as valid CPU statistics.

## 4. Why Two Samples Are Necessary

The CPU counters are cumulative.

A single reading tells us how much CPU time has accumulated
since the counters began, but it does not tell us how much
CPU time was used during a specific interval.

RWatch therefore compares two samples.

Consider these example values.

First sample:

    user:   100
    nice:     0
    system:  50
    idle:   850
    iowait:   0

Total:

    100 + 0 + 50 + 850 + 0 = 1000 ticks

Second sample:

    user:   150
    nice:     0
    system: 100
    idle:  1750
    iowait:   0

Total:

    150 + 0 + 100 + 1750 + 0 = 2000 ticks

The total CPU time increased by:

    total_delta = 2000 - 1000
                = 1000 ticks

Idle time, including I/O wait, is calculated as:

    idle_time = idle + iowait

The idle-time difference is:

    idle_delta = 1750 - 850
               = 900 ticks

The time considered busy by this calculation is:

    busy_delta = total_delta - idle_delta
               = 1000 - 900
               = 100 ticks

CPU utilization is then:

    cpu_usage = (busy_delta / total_delta) * 100

Therefore:

    cpu_usage = (100 / 1000) * 100
              = 10%

RWatch's unit test `calculates_cpu_usage()` verifies
this calculation.

## 5. How RWatch Implements the Calculation

The `CpuTimes` structure provides three important methods.

### `total()`

This method adds the five counters:

    user + nice + system + idle + iowait

It gives the total number of ticks considered by
RWatch's CPU utilization calculation.

### `idle_time()`

This method calculates:

    idle + iowait

RWatch treats both idle time and I/O wait time as
non-busy time for this particular calculation.

### `usage_between()`

This method compares an older `CpuTimes` value with
a newer one.

It calculates the difference in total ticks, calculates
the difference in idle ticks, and subtracts the idle
difference from the total difference.

Finally, it converts the busy-time ratio into a percentage.

If the total difference is zero, the method returns `0.0`
instead of dividing by zero.

## 6. Sampling with `CpuMonitor`

RWatch wraps the sampling process in the `CpuMonitor`
structure.

When `CpuMonitor::new()` is called, it reads the current
CPU counters and stores them as the previous sample.

When `sample()` is called, the monitor:

1. Reads the current CPU counters.
2. Compares them with the previous sample.
3. Calculates CPU utilization.
4. Stores the current counters for the next comparison.
5. Returns the calculated percentage.

The key operation is:

    let usage = self.previous.usage_between(&current);

After calculating the percentage, RWatch updates its
baseline:

    self.previous = current;

This design separates collecting CPU statistics from
calculating utilization.

It also avoids putting a fixed sleep inside the calculation
function. The caller controls when samples are taken.

The first call to `sample()` compares the current counters
with the baseline collected during initialization.
Consequently, the reported percentage covers the time
between those two readings.

## 7. Testing the CPU Monitor

RWatch includes tests for both successful calculations
and malformed input.

For example, `calculates_cpu_usage()` verifies that the
example samples produce a result of `10.0`.

The test `returns_zero_when_no_time_has_passed()` verifies
that identical samples produce zero utilization.

Other tests verify that the parser rejects empty input,
missing values, and invalid numeric data.

These tests are valuable because the parsing and arithmetic
can be tested using fixed input without depending on the
machine's current CPU activity.

## 8. Limitations and Design Considerations

The current implementation is intentionally simple,
but several details matter when interpreting its output.

### Selected counters

RWatch uses five counters and does not include every
field available in `/proc/stat`.

Its result should therefore be understood in the context
of that specific calculation.

### Counter differences

The implementation subtracts the previous counters
from the current counters directly.

It does not use saturating subtraction in `usage_between()`.
The calculation assumes that the counters are monotonic
and that the newer sample contains values at least as
large as the older sample.

### I/O wait

RWatch includes `iowait` in its idle-time calculation.

This means that time waiting for I/O is treated as
non-busy time when computing the percentage.

### Sampling interval

The reported value depends on the interval between samples.
A short interval and a long interval can produce different
views of system activity.

The sampling interval should therefore be considered when
interpreting CPU utilization.

## Conclusion

Linux exposes cumulative CPU counters rather than a
ready-made utilization percentage.

RWatch reads five of these counters, parses them into
a `CpuTimes` structure, and calculates utilization by
comparing consecutive samples.

The implementation demonstrates several useful systems
programming concepts: parsing kernel-provided data,
representing failures with Rust enums, separating sampling
from calculation, and testing arithmetic with deterministic
inputs.

Understanding these details makes it easier to interpret
CPU metrics correctly and to reason about the trade-offs
behind a monitoring tool's implementation.