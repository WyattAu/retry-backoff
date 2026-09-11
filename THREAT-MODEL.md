# Threat Model — retry-backoff

Status: **v1.0** · Method: STRIDE over the public API surface
(`RetryConfig`, `with_backoff`, `RetryError`, `IsRetryable`).

Trust boundaries: (1) caller-supplied configuration (delays,
multipliers, jitter, retry budget), (2) the wrapped fallible operation
whose errors decide retryability, (3) wall-clock time and randomness
(jitter).

The crate is a pure async control-flow utility: no I/O, no state. Its
threat surface is *policy arithmetic*: hostile configs or adversarial
error patterns must not produce unbounded waits, overflow, or
budget-exhaustion attacks against callers.

## Assets

| ID | Asset | Example |
|----|-------|---------|
| A1 | Bounded retry budgets | A permanently failing dependency causing effectively infinite retry loops |
| A2 | Bounded delay arithmetic | `multiplier^attempt` overflowing to infinity/NaN producing absurd sleeps |
| A3 | Correct retryability decisions | Non-retryable errors (auth failures) retried, hammering the dependency |
| A4 | Caller visibility | Retry exhaustion silently swallowed without the final error |

## STRIDE Analysis

| # | Threat | Category | Surface | Mitigation | Verifying test |
|---|--------|----------|---------|------------|----------------|
| T1 | Delay arithmetic overflow (huge attempt counts, huge multipliers) | Tampering/DoS | `RetryConfig::delay_for_attempt` | Delay is computed in `f64` space and capped by `max_delay` before conversion; property tests pin the result to `[0, max_delay]` for arbitrary attempts | `delay_never_exceeds_max`, `delay_for_attempt_always_non_negative`, `delay_increases_monotonically_without_jitter` (`tests/proptest.rs`) |
| T2 | Jitter producing negative or zero-progression sleeps | Tampering | `delay_for_attempt` (jitter on) | Jitter is bounded to ±10% of the capped delay and property-tested non-negative | `delay_with_jitter_still_non_negative` (`tests/proptest.rs`) |
| T3 | Unbounded retries via misconfiguration | DoS | `RetryConfig` | Retry count is a `u32` budget; `no_retries()` exists for disable-and-observe; presets (`default`, `aggressive`) are pinned by tests so silent preset drift is detectable | `no_retries_config_zero_max` (proptest), `default_config`, `delay_increases_exponentially`, `delay_respects_max` |
| T4 | Retryable misclassification amplifies outages | Elevation | `IsRetryable` | Retryability is a caller-implemented trait — the crate never second-guesses typed errors; `RetryError` distinguishes exhausted-retry outcomes from operation errors so callers keep full information | `RetryError` variants in API; caller trait tests in suite |
| T5 | Config corruption between construction and use | Tampering | `RetryConfig` | Config is plain data (`Clone`) and property-tested to preserve values across clones | `config_clone_preserves_values` (`tests/proptest.rs`) |

## OPEN RISKS (missing mitigations — not fabricated)

- **OPEN-1 — no circuit-breaker.** The crate retries per-call; it does
  not coordinate failure budgets across calls. Callers in outage
  scenarios should layer a circuit breaker upstream.
- **OPEN-2 — no retry-budget telemetry.** Retry counts are not emitted
  anywhere; observability of retry storms is the caller's job.
- **OPEN-3 — `aggressive()` preset is generous by design** (10 retries);
  using it against rate-limited services can look like an attack.
  Documented preset, caller's choice.

## Out of Scope

- Distributed retry coordination (idempotency keys, dedup on the
  server side).
- Jitter randomness quality (10% uniform jitter; not crypto-relevant).
- Time-source injection (uses tokio timers as given).

## Residual Risks

- Retry storms remain possible if many callers share a failing
  dependency and all use jittered backoff simultaneously — jitter
  spreads but does not eliminate correlation.
