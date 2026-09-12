# Changelog

All notable changes to this project are documented here. Format: [Keep a
Changelog](https://keepachangelog.com/) — versions follow [semver](https://semver.org).

## [0.1.1] - 2026-09-12

### Added
- `tests/config_matrix.rs`: behavior-observable test for every `RetryConfig`
  knob (`max_retries`, `initial_delay`, `backoff_multiplier`, `max_delay`,
  `jitter`) measured end-to-end through `with_backoff` under a paused tokio
  clock. Dead-knob sweep found zero dead knobs.

### Changed
- Dev-only: tokio dev-dependency gains `test-util` for deterministic clock
  tests. No public API changes.

## [0.1.0] - 2026-09-05

### Added
- Initial public release.
