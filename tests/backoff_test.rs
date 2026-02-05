use std::time::Duration;

use rand::SeedableRng;
use rand::rngs::StdRng;

use palingenesis::resume::{Backoff, BackoffConfig, BackoffError};

#[test]
fn backoff_initial_delay_is_base() {
    let mut config = BackoffConfig::default();
    config.jitter_enabled = false;
    let backoff = Backoff::with_config(config).expect("backoff");

    let delay = backoff.delay_for_attempt(1);
    assert_eq!(delay, Duration::from_secs(30));
}

#[test]
fn backoff_caps_at_max_delay() {
    let mut config = BackoffConfig::default();
    config.jitter_enabled = false;
    let backoff = Backoff::with_config(config).expect("backoff");

    let delay = backoff.delay_for_attempt(5);
    assert_eq!(delay, Duration::from_secs(300));
}

#[test]
fn backoff_jitter_stays_within_bounds() {
    let config = BackoffConfig::default();
    let backoff = Backoff::with_config(config).expect("backoff");
    let mut rng = StdRng::seed_from_u64(42);

    let delay = backoff.delay_for_attempt_with_rng(1, &mut rng);
    let millis = delay.as_millis() as i128;
    let base = Duration::from_secs(30).as_millis() as i128;
    let lower = (base as f64 * 0.9).round() as i128;
    let upper = (base as f64 * 1.1).round() as i128;
    assert!(millis >= lower);
    assert!(millis <= upper);
}

#[test]
fn backoff_enforces_max_retries() {
    let mut config = BackoffConfig::default();
    config.jitter_enabled = false;
    config.max_retries = 2;
    let mut backoff = Backoff::with_config(config).expect("backoff");

    backoff.next_delay().expect("attempt 1");
    backoff.next_delay().expect("attempt 2");
    let err = backoff.next_delay().expect_err("attempt 3 should fail");
    assert!(matches!(err, BackoffError::MaxRetriesExceeded { .. }));
}

#[test]
fn backoff_reset_clears_attempts() {
    let mut config = BackoffConfig::default();
    config.jitter_enabled = false;
    let mut backoff = Backoff::with_config(config).expect("backoff");

    backoff.next_delay().expect("attempt 1");
    backoff.reset();
    assert_eq!(backoff.attempt(), 0);
    let delay = backoff.next_delay().expect("attempt 1 again");
    assert_eq!(delay, Duration::from_secs(30));
}

#[test]
fn backoff_config_validation_rejects_invalid_values() {
    let mut config = BackoffConfig::default();
    config.base_delay = Duration::from_secs(0);
    assert!(matches!(
        Backoff::with_config(config),
        Err(BackoffError::InvalidConfig(_))
    ));

    let mut config = BackoffConfig::default();
    config.max_delay = Duration::from_secs(10);
    config.base_delay = Duration::from_secs(20);
    assert!(matches!(
        Backoff::with_config(config),
        Err(BackoffError::InvalidConfig(_))
    ));

    let mut config = BackoffConfig::default();
    config.jitter_percent = 2.0;
    assert!(matches!(
        Backoff::with_config(config),
        Err(BackoffError::InvalidConfig(_))
    ));
}
