# Changelog

All notable changes to RWatch will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project follows Semantic Versioning.

## [Unreleased]

### Added

* GitHub Actions CI workflow for automated quality checks.
* Formatting checks using `cargo fmt`.
* Strict linting using Clippy.
* Automated test execution.
* Rust documentation checks.
* Release binary build verification.

### Improved

* CLI help and version output.
* JSON snapshot output for machine-readable system metrics.
* Configuration error reporting.
* Defensive parsing of Linux TCP and UDP networking data.
* Terminal restoration when the interactive TUI exits or encounters an error.
* Process selection stability across sorting and live refreshes.

### Quality

* Enforced warning-free Clippy checks.
* Added automated documentation and release-build validation.
