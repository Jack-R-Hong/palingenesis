use std::time::Duration;

use palingenesis::monitor::classifier::{
    RateLimitInfo, RetryAfterSource, StopReason, UserExitInfo, UserExitType,
};
use palingenesis::resume::{StrategySelector, UnknownStrategy};

#[test]
fn strategy_selector_maps_rate_limit_to_same_session() {
    let selector = StrategySelector::new();
    let reason = StopReason::RateLimit(RateLimitInfo {
        retry_after: Duration::from_secs(10),
        source: RetryAfterSource::Header,
        message: None,
    });

    let strategy = selector.select(&reason).expect("strategy");
    assert_eq!(strategy.name(), "SameSessionStrategy");
}

#[test]
fn strategy_selector_maps_context_exhausted_to_new_session() {
    let selector = StrategySelector::new();
    let reason = StopReason::ContextExhausted(None);

    let strategy = selector.select(&reason).expect("strategy");
    assert_eq!(strategy.name(), "NewSessionStrategy");
}

#[test]
fn strategy_selector_skips_user_exit() {
    let selector = StrategySelector::new();
    let reason = StopReason::UserExit(UserExitInfo {
        exit_type: UserExitType::ExitCommand,
        exit_code: None,
        message: Some("exit".to_string()),
    });

    assert!(selector.select(&reason).is_none());
}

#[test]
fn strategy_selector_allows_unknown_default_override() {
    let selector = StrategySelector::with_unknown_default(UnknownStrategy::SameSession);
    let reason = StopReason::Unknown("mystery".to_string());

    let strategy = selector.select(&reason).expect("strategy");
    assert_eq!(strategy.name(), "SameSessionStrategy");
}
