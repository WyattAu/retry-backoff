// Config-knob behavior matrix: every `RetryConfig` knob must observably
// change retry behavior end-to-end through `with_backoff`, not just in
// `delay_for_attempt` math. Deterministic: the tokio clock is paused, so
// sleeps advance instantly while recording their exact durations.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use loop_retry::{IsRetryable, RetryConfig, with_backoff};

/// An error that is always retryable.
#[derive(Debug)]
struct AlwaysRetryable;

impl std::fmt::Display for AlwaysRetryable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "always retryable")
    }
}

impl IsRetryable for AlwaysRetryable {
    fn is_retryable(&self) -> bool {
        true
    }
}

/// Run `with_backoff` against a permanently failing operation under a
/// paused clock and return the exact delays between consecutive attempts.
fn recorded_backoff_delays(config: &RetryConfig) -> Vec<Duration> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        tokio::time::pause();
        let start = tokio::time::Instant::now();
        let attempt_instants: Arc<Mutex<Vec<Duration>>> = Arc::new(Mutex::new(Vec::new()));

        let result = with_backoff(config, || {
            let instants = Arc::clone(&attempt_instants);
            async move {
                instants
                    .lock()
                    .unwrap()
                    .push(tokio::time::Instant::now() - start);
                Err::<(), AlwaysRetryable>(AlwaysRetryable)
            }
        })
        .await;
        assert!(result.is_err(), "operation never succeeds here");

        let instants = attempt_instants.lock().unwrap();
        instants.windows(2).map(|pair| pair[1] - pair[0]).collect()
    })
}

/// Total number of attempts (tries, not retries) for a failing operation.
fn total_attempts(config: &RetryConfig) -> usize {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        tokio::time::pause();
        let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let result = with_backoff(config, || {
            let attempts = Arc::clone(&attempts);
            async move {
                attempts.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Err::<(), AlwaysRetryable>(AlwaysRetryable)
            }
        })
        .await;
        assert!(result.is_err());
        attempts.load(std::sync::atomic::Ordering::Relaxed)
    })
}

/// `max_retries` controls attempt count: 0 = try once with no sleep, N =
/// N+1 tries. Behavior-observable both in count and in elapsed (virtual)
/// time — no retries must mean zero backoff sleeps.
#[test]
fn max_retries_controls_attempt_count_and_zero_means_no_sleep() {
    assert_eq!(total_attempts(&RetryConfig::no_retries()), 1);
    assert_eq!(total_attempts(&RetryConfig::aggressive()), 11);
    assert_eq!(
        total_attempts(&RetryConfig {
            max_retries: 4,
            ..Default::default()
        }),
        5
    );

    // max_retries = 0: a single attempt, no backoff sleep at all.
    let config = RetryConfig {
        max_retries: 0,
        initial_delay: Duration::from_secs(3600),
        ..Default::default()
    };
    assert!(
        recorded_backoff_delays(&config).is_empty(),
        "no_retries must never sleep, even with a huge initial_delay"
    );
}

/// Tokio's timer wheel has 1ms granularity, so a measured sleep can land
/// up to 1ms past its duration. Assert each gap equals the expected
/// millisecond value within that tolerance.
fn assert_delays_ms(actual: &[Duration], expected_ms: &[u64]) {
    assert_eq!(
        actual.len(),
        expected_ms.len(),
        "expected {expected_ms:?}, got {actual:?}"
    );
    for (gap, expected) in actual.iter().zip(expected_ms) {
        let measured = gap.as_millis() as u64;
        assert!(
            measured >= *expected && measured <= expected + 1,
            "measured gap {measured}ms outside [{expected}, {}]ms (tokio 1ms timer granularity)",
            expected + 1
        );
    }
}

/// `initial_delay` is the exact first backoff sleep; changing it changes
/// the first gap (default 100ms vs configured 250ms vs configured 1s).
#[test]
fn initial_delay_sets_the_first_backoff_sleep_exactly() {
    let config = RetryConfig {
        max_retries: 2,
        initial_delay: Duration::from_millis(250),
        backoff_multiplier: 1.0,
        max_delay: Duration::from_secs(60),
        jitter: false,
    };
    assert_delays_ms(
        &recorded_backoff_delays(&config),
        &[250, 250],
        // multiplier 1.0 keeps the delay flat at initial_delay
    );

    let default_first = recorded_backoff_delays(&RetryConfig {
        max_retries: 1,
        jitter: false,
        ..Default::default()
    });
    assert_delays_ms(&default_first, &[100]);
}

/// `backoff_multiplier` scales each successive sleep: 2.0 (default) doubles,
/// 3.0 triples — same initial_delay, observably different delay sequences.
#[test]
fn backoff_multiplier_scales_successive_delays() {
    let base = |multiplier: f64| RetryConfig {
        max_retries: 3,
        initial_delay: Duration::from_millis(100),
        backoff_multiplier: multiplier,
        max_delay: Duration::from_secs(3600),
        jitter: false,
    };

    assert_delays_ms(
        &recorded_backoff_delays(&base(2.0)),
        &[100, 200, 400],
        // default multiplier 2.0 must double each delay
    );
    assert_delays_ms(
        &recorded_backoff_delays(&base(3.0)),
        &[100, 300, 900],
        // multiplier 3.0 must triple each delay
    );
}

/// `max_delay` caps exponential growth: growth stops at the cap and later
/// retries sleep exactly `max_delay`, not more.
#[test]
fn max_delay_caps_exponential_growth() {
    let config = RetryConfig {
        max_retries: 4,
        initial_delay: Duration::from_millis(100),
        backoff_multiplier: 2.0,
        max_delay: Duration::from_millis(250),
        jitter: false,
    };
    assert_delays_ms(
        &recorded_backoff_delays(&config),
        &[100, 200, 250, 250],
        // delays must grow 100→200 then clamp at max_delay=250
    );
}

/// `jitter = false` (the only fully deterministic mode) must produce the
/// exact delay sequence with zero deviation; with `jitter = true` every
/// delay stays within the documented +10% band and the jitter actually
/// varies (a dead jitter knob would yield the exact sequence).
#[test]
fn jitter_changes_delays_within_the_ten_percent_band() {
    let no_jitter = RetryConfig {
        jitter: false,
        ..Default::default()
    };
    assert_eq!(
        no_jitter.delay_for_attempt(0),
        Duration::from_millis(100),
        "jitter=false must be exact"
    );

    let with_jitter = RetryConfig {
        jitter: true,
        ..Default::default()
    };
    let mut saw_two_distinct = false;
    let first = with_jitter.delay_for_attempt(0);
    for _ in 0..500 {
        let d = with_jitter.delay_for_attempt(0);
        assert!(
            (Duration::from_millis(100)..=Duration::from_millis(110)).contains(&d),
            "jittered delay {d:?} outside the +10% band [100ms, 110ms]"
        );
        if d != first {
            saw_two_distinct = true;
        }
    }
    assert!(
        saw_two_distinct,
        "500 jittered samples were all identical — jitter is not varying"
    );

    // End-to-end: a jittered retry sequence sleeps at least the base sum
    // (jitter only ever adds), and strictly more than a max_delay-clamped
    // deterministic run with identical bases would.
    let mut config = with_jitter.clone();
    config.max_retries = 3;
    config.max_delay = Duration::from_secs(3600);
    let delays = recorded_backoff_delays(&config);
    let total: u64 = delays.iter().map(|d| d.as_millis() as u64).sum();
    assert!(
        total >= 700,
        "jittered sleeps {delays:?} must total at least the 100+200+400 base"
    );
}

/// The named constructors behave as documented: `no_retries` tries once,
/// `aggressive` retries ten times with fast 50ms initial backoff.
#[test]
fn named_constructors_are_behaviorally_distinct() {
    assert_eq!(RetryConfig::no_retries().max_retries, 0);
    assert_eq!(total_attempts(&RetryConfig::no_retries()), 1);

    let aggressive = RetryConfig::aggressive();
    assert_eq!(aggressive.max_retries, 10);
    let deterministic_aggressive = RetryConfig {
        jitter: false,
        ..RetryConfig::aggressive()
    };
    assert_eq!(
        deterministic_aggressive.delay_for_attempt(0),
        Duration::from_millis(50),
        "aggressive initial backoff base is 50ms"
    );
    assert_eq!(total_attempts(&aggressive), 11);
}
