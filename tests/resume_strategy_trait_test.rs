use std::path::PathBuf;

use async_trait::async_trait;

use palingenesis::monitor::classifier::StopReason;
use palingenesis::resume::{ResumeContext, ResumeOutcome, ResumeStrategy};

struct MockStrategy;

#[async_trait]
impl ResumeStrategy for MockStrategy {
    async fn execute(
        &self,
        ctx: &ResumeContext,
    ) -> Result<ResumeOutcome, palingenesis::resume::ResumeError> {
        Ok(ResumeOutcome::success(
            ctx.session_path.clone(),
            "mock resume",
        ))
    }

    fn name(&self) -> &'static str {
        "MockStrategy"
    }
}

#[tokio::test]
async fn mock_strategy_executes() {
    let ctx = ResumeContext::new(PathBuf::from("/tmp/session.md"), StopReason::Completed);
    let strategy = MockStrategy;

    let outcome = strategy.execute(&ctx).await.expect("outcome");
    assert!(outcome.is_success());
}
