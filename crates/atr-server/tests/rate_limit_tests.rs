//! Tests for the rate limiter

use atr_server::rate_limit::RateLimiter;

#[test]
fn test_rate_limit_allows_within_limit() {
    let limiter = RateLimiter::new(5, 60);

    for i in 0..5 {
        let result = limiter.check("key-1");
        assert!(result.is_ok(), "Request {} should be allowed", i + 1);
    }
}

#[test]
fn test_rate_limit_blocks_over_limit() {
    let limiter = RateLimiter::new(3, 60);

    // Use up the limit
    assert!(limiter.check("key-1").is_ok());
    assert!(limiter.check("key-1").is_ok());
    assert!(limiter.check("key-1").is_ok());

    // Should be blocked
    let result = limiter.check("key-1");
    assert!(result.is_err());
}

#[test]
fn test_rate_limit_separate_keys() {
    let limiter = RateLimiter::new(2, 60);

    // Key 1
    assert!(limiter.check("key-1").is_ok());
    assert!(limiter.check("key-1").is_ok());
    assert!(limiter.check("key-1").is_err()); // blocked

    // Key 2 should still work
    assert!(limiter.check("key-2").is_ok());
    assert!(limiter.check("key-2").is_ok());
}

#[test]
fn test_rate_limit_remaining_count() {
    let limiter = RateLimiter::new(5, 60);

    assert_eq!(limiter.check("key-1").unwrap(), 4); // 5 - 1 = 4 remaining
    assert_eq!(limiter.check("key-1").unwrap(), 3);
    assert_eq!(limiter.check("key-1").unwrap(), 2);
    assert_eq!(limiter.check("key-1").unwrap(), 1);
    assert_eq!(limiter.check("key-1").unwrap(), 0);
    assert!(limiter.check("key-1").is_err());
}

#[test]
fn test_rate_limit_cleanup() {
    let limiter = RateLimiter::new(100, 60);
    limiter.check("key-1").unwrap();
    limiter.check("key-2").unwrap();
    // Cleanup shouldn't panic
    limiter.cleanup();
}

#[test]
fn test_rate_limit_default() {
    let limiter = RateLimiter::default();
    // Default is 100 per minute — should allow many requests
    for _ in 0..100 {
        assert!(limiter.check("key-1").is_ok());
    }
    assert!(limiter.check("key-1").is_err());
}
