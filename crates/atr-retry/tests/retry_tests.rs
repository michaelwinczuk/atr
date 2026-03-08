//! Tests for the retry engine

use atr_core::error::{AtrError, AtrResult};
use atr_retry::{RetryEngine, RetryPolicy};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_retry_succeeds_first_try() {
    let engine = RetryEngine::new(RetryPolicy::default());
    let result = engine.execute(|_attempt| async { Ok::<_, AtrError>(42) }).await;
    assert_eq!(result.unwrap(), 42);
}

#[tokio::test]
async fn test_retry_succeeds_after_failures() {
    let counter = Arc::new(AtomicU32::new(0));
    let engine = RetryEngine::new(RetryPolicy {
        max_attempts: 3,
        initial_backoff: Duration::from_millis(10),
        max_backoff: Duration::from_millis(100),
        backoff_multiplier: 1.0,
        use_jitter: false,
        fee_escalation: 0.0,
        timeout: None,
    });

    let c = counter.clone();
    let result = engine
        .execute(move |_attempt| {
            let c = c.clone();
            async move {
                let count = c.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(AtrError::RpcError("temporary".to_string()))
                } else {
                    Ok("success")
                }
            }
        })
        .await;

    assert_eq!(result.unwrap(), "success");
    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn test_retry_exhausted() {
    let engine = RetryEngine::new(RetryPolicy {
        max_attempts: 2,
        initial_backoff: Duration::from_millis(10),
        max_backoff: Duration::from_millis(100),
        backoff_multiplier: 1.0,
        use_jitter: false,
        fee_escalation: 0.0,
        timeout: None,
    });

    let result: AtrResult<()> = engine
        .execute(|_| async { Err(AtrError::RpcError("always fails".to_string())) })
        .await;

    assert!(matches!(result.unwrap_err(), AtrError::RetryLimitExceeded));
}

#[test]
fn test_retry_policy_backoff() {
    let policy = RetryPolicy {
        max_attempts: 5,
        initial_backoff: Duration::from_secs(1),
        max_backoff: Duration::from_secs(10),
        backoff_multiplier: 2.0,
        use_jitter: false,
        fee_escalation: 0.1,
        timeout: None,
    };

    // attempt 0: 1s
    let d0 = policy.backoff_duration(0);
    assert_eq!(d0, Duration::from_secs(1));

    // attempt 1: 2s
    let d1 = policy.backoff_duration(1);
    assert_eq!(d1, Duration::from_secs(2));

    // attempt 2: 4s
    let d2 = policy.backoff_duration(2);
    assert_eq!(d2, Duration::from_secs(4));

    // attempt 3: 8s
    let d3 = policy.backoff_duration(3);
    assert_eq!(d3, Duration::from_secs(8));

    // attempt 4: would be 16s but capped at 10s
    let d4 = policy.backoff_duration(4);
    assert_eq!(d4, Duration::from_secs(10));
}

#[test]
fn test_retry_policy_fee_escalation() {
    let policy = RetryPolicy {
        fee_escalation: 0.1, // 10% per attempt
        ..RetryPolicy::default()
    };

    assert_eq!(policy.escalated_fee(1000, 0), 1000);
    assert_eq!(policy.escalated_fee(1000, 1), 1100);
    assert_eq!(policy.escalated_fee(1000, 2), 1200);
    assert_eq!(policy.escalated_fee(1000, 5), 1500);
}

#[test]
fn test_retry_policy_should_retry() {
    let policy = RetryPolicy {
        max_attempts: 3,
        ..RetryPolicy::default()
    };

    assert!(policy.should_retry(0));
    assert!(policy.should_retry(1));
    assert!(policy.should_retry(2));
    assert!(!policy.should_retry(3));
    assert!(!policy.should_retry(4));
}
