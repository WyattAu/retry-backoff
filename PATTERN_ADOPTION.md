# PATTERN_ADOPTION: loop-retry

Relationship to the WyattAu pattern canon. Statuses follow the closed
vocabulary: adopted, partial, planned, refused, n/a. Evidence paths are
relative to this repo.

| Pattern | Status | Evidence |
|---|---|---|
| P24 | refused | Written reason: the crate is stateless by design. src/retry.rs:21-26 (with_backoff takes the config by reference and keeps no cross-call state), src/config.rs:7-22 (plain config values, no budget ledger). Success-refilled retry budgets need shared mutable state across calls; callers compose budgets above this crate. Per-call retry amplification is bounded only by max_retries |
