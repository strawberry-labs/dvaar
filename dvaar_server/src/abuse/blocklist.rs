//! Subdomain blocklist for preventing brand impersonation and abuse

use std::collections::HashSet;
use once_cell::sync::Lazy;

/// Blocked subdomain patterns - these cannot be registered
static BLOCKED_EXACT: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        // === Brand Impersonation (High Risk) ===
        // Payment & Banking
        "paypal", "venmo", "cashapp", "zelle", "stripe", "square",
        "chase", "bankofamerica", "wellsfargo", "citibank", "capitalone",
        "amex", "americanexpress", "visa", "mastercard", "discover",

        // Tech Giants
        "google", "gmail", "youtube", "android", "chrome",
        "apple", "icloud", "itunes", "appstore",
        "microsoft", "outlook", "office", "windows", "azure", "xbox",
        "amazon", "aws", "prime", "alexa",
        "facebook", "instagram", "whatsapp", "messenger", "meta",
        "twitter", "x",
        "netflix", "spotify", "hulu", "disney", "disneyplus",
        "linkedin", "github", "gitlab", "bitbucket",
        "dropbox", "box", "onedrive", "gdrive",
        "slack", "zoom", "teams", "webex",
        "telegram", "signal", "discord",
        "tiktok", "snapchat", "reddit", "pinterest",

        // Crypto
        "coinbase", "binance", "kraken", "gemini", "crypto",
        "bitcoin", "ethereum", "metamask", "ledger", "trezor",
        "opensea", "nft", "wallet", "defi",

        // E-commerce
        "ebay", "etsy", "shopify", "alibaba", "aliexpress", "wish",
        "walmart", "target", "bestbuy", "costco",

        // Shipping
        "fedex", "ups", "usps", "dhl", "amazon-shipping",

        // Security/Auth
        "okta", "auth0", "duo", "lastpass", "1password", "bitwarden",

        // Government
        "irs", "ssa", "medicare", "dmv", "gov", "government",

        // === Security Keywords ===
        "login", "signin", "sign-in", "logon", "log-on",
        "secure", "security", "verify", "verification", "validate",
        "account", "accounts", "myaccount", "my-account",
        "password", "passwd", "reset", "recover", "recovery",
        "update", "confirm", "confirmation", "authenticate", "auth",
        "banking", "bank", "payment", "pay", "billing",
        "support", "helpdesk", "help-desk", "customer-service",
        "admin", "administrator", "root", "system", "sysadmin",

        // === Reserved (Dvaar Infrastructure) ===
        "api", "www", "app", "mail", "email", "smtp", "imap", "pop",
        "ftp", "sftp", "ssh", "vpn", "proxy", "cdn", "static",
        "assets", "img", "images", "js", "css", "fonts",
        "status", "health", "metrics", "monitor", "monitoring",
        "docs", "documentation", "blog", "news",
        "dashboard", "dash", "panel", "console",
        "dvaar", "tunnel", "tunnels", "edge", "node", "nodes",
        "internal", "private", "public", "test", "testing", "dev",
        "stage", "staging", "prod", "production", "demo",
    ]
    .into_iter()
    .collect()
});

/// Blocked substrings - subdomain cannot contain these
static BLOCKED_CONTAINS: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![
        // Brand fragments
        "paypal", "google", "apple", "microsoft", "amazon", "facebook",
        "netflix", "coinbase", "binance", "metamask",

        // Phishing patterns
        "-login", "login-", "-secure", "secure-", "-verify", "verify-",
        "-account", "account-", "-update", "update-", "-confirm", "confirm-",
        "-support", "support-", "-service", "service-", "-help", "help-",
        "-auth", "auth-", "-pay", "pay-", "-billing", "billing-",

        // Suspicious patterns
        "official", "real", "legit", "genuine", "authentic",
        "free-money", "winner", "prize", "lottery", "giveaway",
    ]
});

/// Result of subdomain validation
#[derive(Debug, Clone)]
pub enum SubdomainCheck {
    /// Subdomain is allowed
    Allowed,
    /// Subdomain is blocked with reason
    Blocked(BlockReason),
}

#[derive(Debug, Clone)]
pub enum BlockReason {
    /// Matches exact blocklist entry
    ExactMatch(String),
    /// Contains blocked substring
    ContainsBlocked(String),
    /// Too short
    TooShort,
    /// Too long
    TooLong,
    /// Invalid characters
    InvalidCharacters,
    /// Looks like an IP address
    LooksLikeIP,
    /// All numbers (suspicious)
    AllNumeric,
}

impl BlockReason {
    pub fn message(&self) -> String {
        match self {
            BlockReason::ExactMatch(s) => format!("'{}' is a reserved name", s),
            BlockReason::ContainsBlocked(s) => format!("Subdomain cannot contain '{}'", s),
            BlockReason::TooShort => "Subdomain must be at least 3 characters".to_string(),
            BlockReason::TooLong => "Subdomain must be 63 characters or less".to_string(),
            BlockReason::InvalidCharacters => "Subdomain can only contain lowercase letters, numbers, and hyphens".to_string(),
            BlockReason::LooksLikeIP => "Subdomain cannot look like an IP address".to_string(),
            BlockReason::AllNumeric => "Subdomain cannot be all numbers".to_string(),
        }
    }
}

/// Check if a subdomain is allowed
pub fn check_subdomain(subdomain: &str) -> SubdomainCheck {
    let subdomain = subdomain.to_lowercase();

    // Length checks
    if subdomain.len() < 3 {
        return SubdomainCheck::Blocked(BlockReason::TooShort);
    }
    if subdomain.len() > 63 {
        return SubdomainCheck::Blocked(BlockReason::TooLong);
    }

    // Character validation (lowercase alphanumeric + hyphens, no leading/trailing hyphens)
    if !is_valid_subdomain_chars(&subdomain) {
        return SubdomainCheck::Blocked(BlockReason::InvalidCharacters);
    }

    // All numeric check
    if subdomain.chars().all(|c| c.is_ascii_digit()) {
        return SubdomainCheck::Blocked(BlockReason::AllNumeric);
    }

    // IP-like pattern (e.g., "192-168-1-1")
    if looks_like_ip(&subdomain) {
        return SubdomainCheck::Blocked(BlockReason::LooksLikeIP);
    }

    // Exact match check
    if BLOCKED_EXACT.contains(subdomain.as_str()) {
        return SubdomainCheck::Blocked(BlockReason::ExactMatch(subdomain));
    }

    // Contains check
    for blocked in BLOCKED_CONTAINS.iter() {
        if subdomain.contains(blocked) {
            return SubdomainCheck::Blocked(BlockReason::ContainsBlocked(blocked.to_string()));
        }
    }

    SubdomainCheck::Allowed
}

/// Validate subdomain characters
fn is_valid_subdomain_chars(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    // Cannot start or end with hyphen
    if s.starts_with('-') || s.ends_with('-') {
        return false;
    }

    // Cannot have consecutive hyphens (except for punycode, but we block that anyway)
    if s.contains("--") {
        return false;
    }

    // Only lowercase alphanumeric and hyphens
    s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Check if subdomain looks like an IP address
fn looks_like_ip(s: &str) -> bool {
    // Pattern like "192-168-1-1" or "10-0-0-1"
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() == 4 {
        return parts.iter().all(|p| p.parse::<u8>().is_ok());
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_subdomains() {
        assert!(matches!(check_subdomain("myapp"), SubdomainCheck::Allowed));
        assert!(matches!(check_subdomain("cool-project"), SubdomainCheck::Allowed));
        assert!(matches!(check_subdomain("app123"), SubdomainCheck::Allowed));
        assert!(matches!(check_subdomain("my-cool-app"), SubdomainCheck::Allowed));
    }

    #[test]
    fn test_blocked_brands() {
        assert!(matches!(check_subdomain("paypal"), SubdomainCheck::Blocked(_)));
        assert!(matches!(check_subdomain("google"), SubdomainCheck::Blocked(_)));
        assert!(matches!(check_subdomain("netflix"), SubdomainCheck::Blocked(_)));
        assert!(matches!(check_subdomain("coinbase"), SubdomainCheck::Blocked(_)));
    }

    #[test]
    fn test_blocked_contains() {
        assert!(matches!(check_subdomain("my-paypal-login"), SubdomainCheck::Blocked(_)));
        assert!(matches!(check_subdomain("google-verify"), SubdomainCheck::Blocked(_)));
        assert!(matches!(check_subdomain("secure-bank"), SubdomainCheck::Blocked(_)));
    }

    #[test]
    fn test_reserved_names() {
        assert!(matches!(check_subdomain("api"), SubdomainCheck::Blocked(_)));
        assert!(matches!(check_subdomain("admin"), SubdomainCheck::Blocked(_)));
        assert!(matches!(check_subdomain("www"), SubdomainCheck::Blocked(_)));
    }

    #[test]
    fn test_invalid_format() {
        assert!(matches!(check_subdomain("ab"), SubdomainCheck::Blocked(BlockReason::TooShort)));
        assert!(matches!(check_subdomain("-test"), SubdomainCheck::Blocked(BlockReason::InvalidCharacters)));
        assert!(matches!(check_subdomain("test-"), SubdomainCheck::Blocked(BlockReason::InvalidCharacters)));
        assert!(matches!(check_subdomain("123456"), SubdomainCheck::Blocked(BlockReason::AllNumeric)));
        assert!(matches!(check_subdomain("192-168-1-1"), SubdomainCheck::Blocked(BlockReason::LooksLikeIP)));
    }
}
