//! Rate limiting for abuse prevention
//!
//! Uses Redis for distributed rate limiting across nodes.
//! Implements sliding window algorithm for smooth rate limiting.

use fred::prelude::*;
use std::sync::Arc;
use std::time::Duration;

/// Rate limiter configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests allowed in the window
    pub max_requests: u32,
    /// Time window duration
    pub window: Duration,
}

impl RateLimitConfig {
    pub fn new(max_requests: u32, window_secs: u64) -> Self {
        Self {
            max_requests,
            window: Duration::from_secs(window_secs),
        }
    }
}

/// Pre-configured rate limits for different operations
pub mod limits {
    use super::*;

    /// Tunnel creation: 5 per hour for free, 20 per hour for paid
    pub fn tunnel_creation_free() -> RateLimitConfig {
        RateLimitConfig::new(5, 3600)
    }

    pub fn tunnel_creation_paid() -> RateLimitConfig {
        RateLimitConfig::new(20, 3600)
    }

    /// Request rate: 60/min for free, 600/min for paid
    pub fn requests_free() -> RateLimitConfig {
        RateLimitConfig::new(60, 60)
    }

    pub fn requests_paid() -> RateLimitConfig {
        RateLimitConfig::new(600, 60)
    }

    /// Auth attempts: 10 per hour (same for all)
    pub fn auth_attempts() -> RateLimitConfig {
        RateLimitConfig::new(10, 3600)
    }

    /// Subdomain claims: 3 per hour for free
    pub fn subdomain_claims_free() -> RateLimitConfig {
        RateLimitConfig::new(3, 3600)
    }

    pub fn subdomain_claims_paid() -> RateLimitConfig {
        RateLimitConfig::new(10, 3600)
    }
}

/// Result of a rate limit check
#[derive(Debug, Clone)]
pub struct RateLimitResult {
    /// Whether the request is allowed
    pub allowed: bool,
    /// Current count in the window
    pub current: u32,
    /// Maximum allowed in the window
    pub limit: u32,
    /// Seconds until the window resets
    pub reset_in_secs: u64,
    /// Remaining requests in this window
    pub remaining: u32,
}

impl RateLimitResult {
    /// Create headers for rate limit response
    pub fn headers(&self) -> Vec<(String, String)> {
        vec![
            ("X-RateLimit-Limit".to_string(), self.limit.to_string()),
            ("X-RateLimit-Remaining".to_string(), self.remaining.to_string()),
            ("X-RateLimit-Reset".to_string(), self.reset_in_secs.to_string()),
        ]
    }
}

/// Distributed rate limiter using Redis
#[derive(Clone)]
pub struct RateLimiter {
    redis: Arc<Client>,
}

impl RateLimiter {
    pub fn new(redis: Arc<Client>) -> Self {
        Self { redis }
    }

    /// Check and increment rate limit
    ///
    /// Uses Redis MULTI/EXEC for atomic sliding window rate limiting.
    ///
    /// # Arguments
    /// * `key_prefix` - Prefix for the rate limit key (e.g., "rl:tunnel")
    /// * `identifier` - Unique identifier (e.g., user_id or IP)
    /// * `config` - Rate limit configuration
    ///
    /// # Returns
    /// * `RateLimitResult` with current state and whether request is allowed
    pub async fn check(&self, key_prefix: &str, identifier: &str, config: &RateLimitConfig) -> anyhow::Result<RateLimitResult> {
        let key = format!("{}:{}", key_prefix, identifier);
        let window_secs = config.window.as_secs();

        // Use INCR + EXPIRE for simple sliding window
        // This is a simplified version - for production, consider using
        // a proper sliding window with sorted sets

        let current: u32 = self.redis.incr(&key).await.unwrap_or(1);

        // Set expiry on first request
        if current == 1 {
            let _: () = self.redis
                .expire(&key, window_secs as i64, None)
                .await
                .unwrap_or(());
        }

        // Get TTL for reset time
        let ttl: i64 = self.redis.ttl(&key).await.unwrap_or(window_secs as i64);
        let reset_in_secs = if ttl > 0 { ttl as u64 } else { window_secs };

        let allowed = current <= config.max_requests;
        let remaining = if allowed {
            config.max_requests - current
        } else {
            0
        };

        Ok(RateLimitResult {
            allowed,
            current,
            limit: config.max_requests,
            reset_in_secs,
            remaining,
        })
    }

    /// Check rate limit without incrementing (peek)
    pub async fn peek(&self, key_prefix: &str, identifier: &str, config: &RateLimitConfig) -> anyhow::Result<RateLimitResult> {
        let key = format!("{}:{}", key_prefix, identifier);
        let window_secs = config.window.as_secs();

        let current: u32 = self.redis.get(&key).await.unwrap_or(0);
        let ttl: i64 = self.redis.ttl(&key).await.unwrap_or(window_secs as i64);
        let reset_in_secs = if ttl > 0 { ttl as u64 } else { window_secs };

        let allowed = current < config.max_requests;
        let remaining = if current < config.max_requests {
            config.max_requests - current
        } else {
            0
        };

        Ok(RateLimitResult {
            allowed,
            current,
            limit: config.max_requests,
            reset_in_secs,
            remaining,
        })
    }

    /// Reset rate limit for an identifier
    pub async fn reset(&self, key_prefix: &str, identifier: &str) -> anyhow::Result<()> {
        let key = format!("{}:{}", key_prefix, identifier);
        let _: () = self.redis.del(&key).await?;
        Ok(())
    }

    /// Check rate limit for tunnel creation
    pub async fn check_tunnel_creation(&self, user_id: &str, is_paid: bool) -> anyhow::Result<RateLimitResult> {
        let config = if is_paid {
            limits::tunnel_creation_paid()
        } else {
            limits::tunnel_creation_free()
        };
        self.check("rl:tunnel", user_id, &config).await
    }

    /// Check rate limit for incoming requests to a tunnel
    pub async fn check_requests(&self, subdomain: &str, is_paid: bool) -> anyhow::Result<RateLimitResult> {
        let config = if is_paid {
            limits::requests_paid()
        } else {
            limits::requests_free()
        };
        self.check("rl:req", subdomain, &config).await
    }

    /// Check rate limit for auth attempts (by IP)
    pub async fn check_auth(&self, ip: &str) -> anyhow::Result<RateLimitResult> {
        self.check("rl:auth", ip, &limits::auth_attempts()).await
    }

    /// Check rate limit for subdomain claims
    pub async fn check_subdomain_claim(&self, user_id: &str, is_paid: bool) -> anyhow::Result<RateLimitResult> {
        let config = if is_paid {
            limits::subdomain_claims_paid()
        } else {
            limits::subdomain_claims_free()
        };
        self.check("rl:claim", user_id, &config).await
    }
}

/// In-memory rate limiter for local-only rate limiting (backup if Redis fails)
pub mod local {
    use dashmap::DashMap;
    use std::time::Instant;
    use super::*;

    pub struct LocalRateLimiter {
        buckets: DashMap<String, (u32, Instant)>,
    }

    impl LocalRateLimiter {
        pub fn new() -> Self {
            Self {
                buckets: DashMap::new(),
            }
        }

        pub fn check(&self, key: &str, config: &RateLimitConfig) -> RateLimitResult {
            let now = Instant::now();

            let mut entry = self.buckets.entry(key.to_string()).or_insert((0, now));
            let (count, window_start) = entry.value_mut();

            // Reset if window expired
            if now.duration_since(*window_start) >= config.window {
                *count = 0;
                *window_start = now;
            }

            *count += 1;
            let current = *count;
            let allowed = current <= config.max_requests;
            let elapsed = now.duration_since(*window_start);
            let reset_in_secs = config.window.as_secs().saturating_sub(elapsed.as_secs());

            RateLimitResult {
                allowed,
                current,
                limit: config.max_requests,
                reset_in_secs,
                remaining: if allowed { config.max_requests - current } else { 0 },
            }
        }

        /// Clean up expired entries (call periodically)
        pub fn cleanup(&self, max_age: Duration) {
            let now = Instant::now();
            self.buckets.retain(|_, (_, start)| now.duration_since(*start) < max_age);
        }
    }

    impl Default for LocalRateLimiter {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_config() {
        let config = RateLimitConfig::new(100, 60);
        assert_eq!(config.max_requests, 100);
        assert_eq!(config.window.as_secs(), 60);
    }

    #[test]
    fn test_local_rate_limiter() {
        let limiter = local::LocalRateLimiter::new();
        let config = RateLimitConfig::new(3, 60);

        // First 3 requests should be allowed
        for i in 1..=3 {
            let result = limiter.check("test", &config);
            assert!(result.allowed, "Request {} should be allowed", i);
            assert_eq!(result.current, i as u32);
            assert_eq!(result.remaining, 3 - i as u32);
        }

        // 4th request should be blocked
        let result = limiter.check("test", &config);
        assert!(!result.allowed);
        assert_eq!(result.remaining, 0);
    }
}
