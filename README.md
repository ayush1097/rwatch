# RWatch

**An asynchronous Linux monitoring and internals observatory written in Rust.**

RWatch collects Linux system information, converts it into useful
metrics, and presents the results through a terminal UI and HTTP API.

The project explores Linux internals, Rust systems programming,
asynchronous execution, and reliable system monitoring.

## Problem

Linux exposes system information through interfaces such as `/proc`
and `/sys`. However, raw system information is not always convenient
to interpret.

RWatch aims to provide a structured view of system activity by
collecting, processing, and presenting this information.

## Goals

- Explore Linux system interfaces using Rust.
- Monitor CPU, memory, disk, and network activity.
- Inspect processes and Linux resource information.
- Use Tokio for asynchronous monitoring tasks.
- Communicate between collectors using bounded channels.
- Present monitoring information through a terminal UI and HTTP API.
- Handle permission errors and processes that disappear during collection.
- Keep the monitoring functionality read-only by default.

## Features

- CPU and memory monitoring.
- Disk and network statistics.
- Process inspection and lifecycle detection.
- Cgroup information.
- TCP and UDP socket inspection.
- Terminal interface built with Ratatui and Crossterm.
- HTTP API built with Axum.
- JSON snapshot output.
- Configurable monitoring intervals and logging.
- Explicit process-control operations with safety checks.

## Architecture

RWatch separates data collection from presentation.

Linux system interfaces
        |
        v
Monitoring modules
        |
        v
Metric events
        |
        v
Bounded Tokio channels
        |
        v
Central event loop
        |
        v
Shared application state
       / \
      v   v
 Terminal  HTTP API

Collectors gather system information and send updates through
Tokio channels. The application processes these updates into shared
state for the terminal interface and HTTP API.

Process lifecycle detection currently uses periodic process snapshots
to identify processes that appear or disappear.

See [docs/architecture.md](docs/architecture.md) for more details.

## Technology Stack

- **Language:** Rust
- **Async runtime:** Tokio
- **Terminal UI:** Ratatui and Crossterm
- **HTTP API:** Axum
- **Serialization:** Serde and JSON
- **Linux interfaces:** `/proc`, `/sys`, and Linux-specific APIs

## Requirements

- A Linux environment.
- A Rust toolchain with Cargo.
- The dependencies specified in `Cargo.toml`.

RWatch has been developed and tested in a WSL2 Linux environment.
Native Linux distributions and kernel configurations may expose
different capabilities.

## Build

### Requirements

* Linux or WSL2 with a supported Linux environment.
* Rust toolchain and Cargo.
* The dependencies specified in `Cargo.toml`.

RWatch uses Linux-specific interfaces, including `/proc`, `/sys`, and Linux process APIs. Native Windows and macOS execution are not supported.

### Clone and build

Clone the repository:

```bash
git clone <repository-url>
cd rwatch
```

Build the debug binary:

```bash
cargo build --bin rwatch
```

Build an optimized release binary:

```bash
cargo build --release --bin rwatch
```

Run the test suite:

```bash
cargo test --all-targets --all-features
```

Check formatting and linting:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

Clone the repository and enter the project directory:

```bash
git clone <repository-url>
cd rwatch
```

## Configuration

RWatch reads its configuration from `rwatch.toml`.

Example:

```toml
refresh_interval = "1s"
process_interval = "5s"
network_interval = "2s"
log_level = "info"
```

- `refresh_interval`: Terminal UI refresh interval.
- `process_interval`: Process monitoring interval.
- `network_interval`: Network monitoring interval.
- `log_level`: Logging filter. `RUST_LOG` can override this setting.

## Usage

Start RWatch, including its terminal UI and HTTP API:

```bash
cargo run
```

Generate a JSON snapshot:

```bash
cargo run -- snapshot --json
```

## HTTP API

The HTTP API listens on port `3000`.

| Endpoint | Description |
|---|---|
| `GET /api/system` | System monitoring information |
| `GET /api/processes` | List processes |
| `GET /api/processes/{pid}` | Inspect a specific process |
| `GET /api/network` | Network interface statistics |
| `GET /api/cgroups` | Cgroup information |
| `GET /api/events` | Server-Sent Events (SSE) stream of periodic system snapshots |

### Examples

Retrieve system information:

```bash
curl http://127.0.0.1:3000/api/system
```

List processes:

```bash
curl http://127.0.0.1:3000/api/processes
```

Inspect a process:

```bash
curl http://127.0.0.1:3000/api/processes/287
```

Retrieve network information:

```bash
curl http://127.0.0.1:3000/api/network
```

Retrieve cgroup information:

```bash
curl http://127.0.0.1:3000/api/cgroups
```

A lookup for a nonexistent process returns `404 Not Found`.

Some Linux information may be unavailable because of permissions or kernel configuration.

**Security:** The API currently binds to `0.0.0.0:3000`, listening on all available IPv4 interfaces. Use appropriate network and firewall restrictions when running RWatch on a network-accessible machine.

## Testing

## Continuous Integration

RWatch uses GitHub Actions to automate code quality checks on pushes and pull requests.

The CI workflow checks:

* Rust formatting with `cargo fmt`.
* Code quality with Clippy.
* Tests across available targets and features.
* Documentation builds with warnings treated as errors.
* Optimized release builds.

Run the same checks locally before submitting changes:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo doc --no-deps --all-features
cargo build --release --bin rwatch
```


## Security

RWatch is designed to be read-only by default. Operations that affect processes require explicit user action and validation.

See [docs/security.md](docs/security.md) for more details.

## Non-goals

- Replacing every feature of tools such as `htop`.
- Initially supporting Windows and macOS as native platforms.
- Hiding Linux-specific behavior behind excessive abstraction.
- Optimizing prematurely without measurements.

## Project Status

RWatch is an evolving systems programming project. Features and supported environments may change as development continues.