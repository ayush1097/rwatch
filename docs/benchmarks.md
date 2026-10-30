# RWatch Performance Benchmarks

## Environment

- Platform: Linux on WSL2
- Benchmark profile: Rust optimized release profile
- Network source: /proc/net/dev
- Network collection iterations: 100,000
- Network-rate calculation iterations: 100,000

## Network Collection

| Metric | Result |
|---|---:|
| Total time | 908.66 ms |
| Average time per operation | 9.09 µs |
| Operations per second | 110,052 |
| Total interface records | 200,000 |

## Network-Rate Calculation

| Metric | Result |
|---|---:|
| Total time | 3.61 ms |
| Average time per operation | 0.04 µs |
| Operations per second | 27,671,052 |

## Notes

These are preliminary measurements from one WSL2 environment.
Results can vary with system load, kernel behavior, and hardware.

The collection benchmark includes reading and parsing /proc/net/dev.
The rate-calculation benchmark operates on two previously collected samples.

## Resource Usage Benchmark

### Workload

- Duration: approximately 30 seconds
- Process scans: 30
- Total process records: 1,569
- Average processes per scan: 52.30

### Results

| Metric | Result |
|---|---:|
| CPU time | 0.070 seconds |
| CPU utilization | 0.23% of one CPU |
| RSS memory at end of run | 2.25 MiB |

### Limitations

This benchmark measures a process-scanning workload,
not the complete RWatch application.

CPU usage is calculated from process CPU ticks.
RSS is a single measurement at the end of the run,
not peak memory usage.

Results are specific to this WSL2 environment and workload.


## Process Collection Benchmark

### Workload

- Iterations: 100
- Average processes per scan: 55

### Results

| Metric | Result |
|---|---:|
| Total benchmark time | 198.32 ms |
| Average scan time | 1.983 ms |
| Minimum scan time | 1.652 ms |
| Maximum scan time | 3.766 ms |

### Notes

The benchmark measures collect_processes() over 100 iterations.

Results are specific to this WSL2 environment and workload.
The benchmark does not measure the full application's
end-to-end monitoring latency.



## Network Parser Benchmark

### Workload

- Iterations per operation: 1,000,000
- Platform: Linux on WSL2
- Build profile: optimized release benchmark

### Results

| Operation | Total time | Time per operation | Operations per second |
|---|---:|---:|---:|
| IPv4 parsing | 103.46 ms | 103.46 ns | 9,665,450 |
| IPv6 parsing | 97.13 ms | 97.13 ns | 10,295,815 |
| Endpoint formatting | 47.69 ms | 47.69 ns | 20,967,882 |

### Notes

These results measure individual parsing and formatting functions,
not complete network collection.

Results may vary with hardware, system load, and compiler settings.


## Network Parser Allocation Benchmark

### Workload

- Iterations per implementation: 100,000
- Input: one network interface record
- Build profile: optimized release benchmark

### Results

| Metric | Original parser | Optimized parser |
|---|---:|---:|
| Execution time | 23.26 ms | 14.42 ms |
| Total allocations | 400,000 | 100,000 |
| Allocations per operation | 4.00 | 1.00 |
| Operations per second | 4,298,955 | 6,935,999 |

### Observations

The optimized parser reduced the measured allocation count by 75%
and execution time by approximately 38% in this benchmark.

The optimized implementation avoids collecting all whitespace-separated
fields into a temporary Vec.

Both implementations still allocate an owned String for the interface name.

### Limitations

Results are specific to the test input and environment.
Allocation counts measure successful allocations and reallocations,
not allocated bytes or peak memory usage.


## Live Application Resource Observations

### Environment

* Platform: Linux on WSL2
* Application: RWatch release binary
* Process ID during measurement: 10121
* Measurement duration: approximately 30–60 seconds per run

### Observed Memory Usage

| Run                   | Average sampled RSS | Peak sampled RSS |
| --------------------- | ------------------: | ---------------: |
| Earlier observation 1 |            6.97 MiB |         7.07 MiB |
| Earlier observation 2 |            7.90 MiB |         8.24 MiB |
| Later observation     |            9.50 MiB |         9.62 MiB |
| Later observation     |            9.83 MiB |         9.87 MiB |

### Observed CPU Usage

CPU samples were generally low, usually between 0% and 2% of one logical CPU, with occasional brief readings reaching 9%.

### HTTP API Smoke Test

A scripted test completed 900 HTTP requests across the `/api/system`, `/api/processes`, and `/api/network` endpoints without reporting a failed request.

### Limitations

These observations come from separate application runs and are not a controlled performance comparison. The available results do not establish why RSS differed between runs.

The resource measurements and HTTP requests were not confirmed to have run concurrently. Therefore, the results must not be interpreted as resource consumption under HTTP load.

CPU readings are sampled observations, not a complete CPU profile. These results are specific to the observed WSL2 environment and workload.
