# Requirements — retry-backoff

Numbered, testable requirements. Every requirement maps to at least one named
test; every security-relevant test cites at least one requirement. Threat
IDs reference `THREAT-MODEL.md`.

Scope note: `loop-retry` provides generic async retry with exponential
backoff and jitter — `RetryConfig` presets and custom budgets,
`delay_for_attempt` policy math, `with_backoff` as the retry executor,
`RetryError` for exhaustion outcomes, and the `IsRetryable` trait for
caller-defined retryability.

## Functional

| ID | Requirement | Priority |
|----|-------------|----------|
| REQ-RB-001 | `RetryConfig::default()` provides a sensible preset (3 retries, 5s max delay, jitter on) | MUST |
| REQ-RB-002 | `delay_for_attempt` grows exponentially with the attempt index and respects `max_delay` as a hard cap | MUST |
| REQ-RB-003 | `delay_for_attempt` returns non-negative durations with jitter on and off | MUST |
| REQ-RB-004 | `no_retries()` disables retrying; `aggressive()` raises the budget (10 retries, 30s cap) | SHOULD |
| REQ-RB-005 | `with_backoff` executes the wrapped operation, retrying per config, and surfaces `RetryError` on exhaustion | MUST |
| REQ-RB-006 | `RetryConfig` is `Clone` and clone preserves all values | SHOULD |

## Security

| ID | Requirement | Priority |
|----|-------------|----------|
| REQ-RB-100 | Delay arithmetic is overflow-safe: for any attempt index the computed delay stays within `[0, max_delay]` (T1) | MUST |
| REQ-RB-101 | Jittered delays are non-negative and bounded by the cap (T2) | MUST |
| REQ-RB-102 | Retry budgets are finite and preset values are pinned by tests — configuration drift cannot silently create unbounded retry loops (T3) | MUST |
| REQ-RB-103 | Retryability is caller-owned via `IsRetryable`; the crate never retries non-retryable errors on its own authority (T4) | MUST |

## Robustness

| ID | Requirement | Priority |
|----|-------------|----------|
| REQ-RB-200 | Without jitter, delays increase monotonically with attempt index | MUST |

## Traceability Matrix

| Requirement | Test (fn, file) | Property class |
|-------------|-----------------|----------------|
| REQ-RB-001 | `default_config` (`src/config.rs` tests) | unit |
| REQ-RB-002 | `delay_increases_exponentially`, `delay_respects_max` (`src/config.rs`); `delay_never_exceeds_max`, `delay_increases_monotonically_without_jitter` (`tests/proptest.rs`) | unit/property |
| REQ-RB-003 | `delay_for_attempt_always_non_negative`, `delay_with_jitter_still_non_negative` (`tests/proptest.rs`) | property |
| REQ-RB-004 | `no_retries_config_zero_max` (`tests/proptest.rs`), `aggressive` (config tests) | unit/property |
| REQ-RB-005 | `with_backoff` executor paths (`src/retry.rs` tests) | unit |
| REQ-RB-006 | `config_clone_preserves_values` (`tests/proptest.rs`) | property |
| REQ-RB-100 | `delay_never_exceeds_max`, `delay_for_attempt_always_non_negative` (`tests/proptest.rs`) | property |
| REQ-RB-101 | `delay_with_jitter_still_non_negative`, `delay_never_exceeds_max` | property |
| REQ-RB-102 | `no_retries_config_zero_max`, `default_config`, `delay_respects_max` | unit/property |
| REQ-RB-103 | `IsRetryable` trait contract (`src/traits.rs`) | design/unit |
| REQ-RB-200 | `delay_increases_monotonically_without_jitter` (`tests/proptest.rs`) | property |

## Test Count

- 9 `#[test]` functions in the unit suite plus 5 property tests
  (`tests/proptest.rs`).
- All-features suite passes with 0 failures; no-default-features suite passes.
