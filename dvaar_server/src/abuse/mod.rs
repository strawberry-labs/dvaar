//! Abuse prevention module
//!
//! Provides protection against:
//! - Brand impersonation (subdomain blocklist)
//! - Rate limiting (tunnel creation, requests)
//! - Phishing detection (future)
//! - Anomaly detection (future)

pub mod blocklist;
pub mod rate_limit;

pub use blocklist::{check_subdomain, BlockReason, SubdomainCheck};
pub use rate_limit::{RateLimitConfig, RateLimitResult, RateLimiter};
