//! Database connection and models

use chrono::{DateTime, Utc};
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

/// Initialize the database connection pool
pub async fn init_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}

/// Run database migrations
pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

/// User model
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub stripe_customer_id: Option<String>,
    pub plan: String,
    pub stripe_subscription_id: Option<String>,
    pub plan_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    /// Check if user has a paid plan
    pub fn is_paid(&self) -> bool {
        self.plan != "free"
    }
}

/// API key model
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ApiKey {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token: String,
    pub label: Option<String>,
    pub last_used_at: Option<DateTime<Utc>>,
}

/// Domain model
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Domain {
    pub subdomain: String,
    pub user_id: Uuid,
    pub is_active: bool,
}

/// Custom domain model (for CNAME support)
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CustomDomain {
    pub id: Uuid,
    pub domain: String,
    pub subdomain: String,
    pub user_id: Uuid,
    pub verified: bool,
    pub created_at: DateTime<Utc>,
}

/// User plan model
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserPlan {
    pub user_id: Uuid,
    pub plan: String,
    pub max_tunnels: i32,
    pub max_bandwidth_bytes: i64,
    pub custom_domains_allowed: bool,
}

/// Database queries
pub mod queries {
    use super::*;

    /// Find a user by their API token
    pub async fn find_user_by_token(pool: &PgPool, token: &str) -> Result<Option<User>, sqlx::Error> {
        let result = sqlx::query_as::<_, User>(
            r#"
            SELECT u.id, u.email, u.stripe_customer_id, u.plan, u.stripe_subscription_id,
                   u.plan_expires_at, u.created_at, u.updated_at
            FROM users u
            INNER JOIN api_keys ak ON ak.user_id = u.id
            WHERE ak.token = $1
            "#,
        )
        .bind(token)
        .fetch_optional(pool)
        .await?;

        // Update last_used_at for the API key
        if result.is_some() {
            sqlx::query("UPDATE api_keys SET last_used_at = NOW() WHERE token = $1")
                .bind(token)
                .execute(pool)
                .await?;
        }

        Ok(result)
    }

    /// Find a user by ID
    pub async fn find_user_by_id(pool: &PgPool, id: Uuid) -> Result<Option<User>, sqlx::Error> {
        sqlx::query_as::<_, User>(
            r#"SELECT id, email, stripe_customer_id, plan, stripe_subscription_id,
                      plan_expires_at, created_at, updated_at FROM users WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    /// Find a user by email
    pub async fn find_user_by_email(pool: &PgPool, email: &str) -> Result<Option<User>, sqlx::Error> {
        sqlx::query_as::<_, User>(
            r#"SELECT id, email, stripe_customer_id, plan, stripe_subscription_id,
                      plan_expires_at, created_at, updated_at FROM users WHERE email = $1"#,
        )
        .bind(email)
        .fetch_optional(pool)
        .await
    }

    /// Find a user by Stripe customer ID
    pub async fn find_user_by_stripe_customer(pool: &PgPool, customer_id: &str) -> Result<Option<User>, sqlx::Error> {
        sqlx::query_as::<_, User>(
            r#"SELECT id, email, stripe_customer_id, plan, stripe_subscription_id,
                      plan_expires_at, created_at, updated_at FROM users WHERE stripe_customer_id = $1"#,
        )
        .bind(customer_id)
        .fetch_optional(pool)
        .await
    }

    /// Create a new user
    pub async fn create_user(pool: &PgPool, email: &str) -> Result<User, sqlx::Error> {
        sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (email)
            VALUES ($1)
            RETURNING id, email, stripe_customer_id, plan, stripe_subscription_id,
                      plan_expires_at, created_at, updated_at
            "#,
        )
        .bind(email)
        .fetch_one(pool)
        .await
    }

    /// Create or get user by email (upsert)
    pub async fn upsert_user(pool: &PgPool, email: &str) -> Result<User, sqlx::Error> {
        sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (email)
            VALUES ($1)
            ON CONFLICT (email) DO UPDATE SET updated_at = NOW()
            RETURNING id, email, stripe_customer_id, plan, stripe_subscription_id,
                      plan_expires_at, created_at, updated_at
            "#,
        )
        .bind(email)
        .fetch_one(pool)
        .await
    }

    /// Update user's Stripe customer ID
    pub async fn update_stripe_customer(pool: &PgPool, user_id: Uuid, customer_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET stripe_customer_id = $1 WHERE id = $2")
            .bind(customer_id)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Update user's subscription and plan
    pub async fn update_user_subscription(
        pool: &PgPool,
        user_id: Uuid,
        plan: &str,
        subscription_id: Option<&str>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"UPDATE users SET plan = $1, stripe_subscription_id = $2, plan_expires_at = $3 WHERE id = $4"#
        )
            .bind(plan)
            .bind(subscription_id)
            .bind(expires_at)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Create an API key for a user
    pub async fn create_api_key(
        pool: &PgPool,
        user_id: Uuid,
        token: &str,
        label: Option<&str>,
    ) -> Result<ApiKey, sqlx::Error> {
        sqlx::query_as::<_, ApiKey>(
            r#"
            INSERT INTO api_keys (user_id, token, label)
            VALUES ($1, $2, $3)
            RETURNING id, user_id, token, label, last_used_at
            "#,
        )
        .bind(user_id)
        .bind(token)
        .bind(label)
        .fetch_one(pool)
        .await
    }

    /// Check if a subdomain is reserved by a user
    pub async fn check_subdomain_owner(
        pool: &PgPool,
        subdomain: &str,
    ) -> Result<Option<Domain>, sqlx::Error> {
        sqlx::query_as::<_, Domain>(
            "SELECT subdomain, user_id, is_active FROM domains WHERE subdomain = $1",
        )
        .bind(subdomain)
        .fetch_optional(pool)
        .await
    }

    /// Reserve a subdomain for a user
    pub async fn reserve_subdomain(
        pool: &PgPool,
        subdomain: &str,
        user_id: Uuid,
    ) -> Result<Domain, sqlx::Error> {
        sqlx::query_as::<_, Domain>(
            r#"
            INSERT INTO domains (subdomain, user_id)
            VALUES ($1, $2)
            ON CONFLICT (subdomain) DO UPDATE SET is_active = TRUE
            RETURNING subdomain, user_id, is_active
            "#,
        )
        .bind(subdomain)
        .bind(user_id)
        .fetch_one(pool)
        .await
    }

    /// Find subdomain by custom domain (CNAME lookup)
    pub async fn find_subdomain_by_custom_domain(
        pool: &PgPool,
        domain: &str,
    ) -> Result<Option<String>, sqlx::Error> {
        let result: Option<(String,)> = sqlx::query_as(
            "SELECT subdomain FROM custom_domains WHERE domain = $1 AND verified = TRUE",
        )
        .bind(domain)
        .fetch_optional(pool)
        .await?;

        Ok(result.map(|(s,)| s))
    }

    /// Add a custom domain for a user
    pub async fn add_custom_domain(
        pool: &PgPool,
        domain: &str,
        subdomain: &str,
        user_id: Uuid,
    ) -> Result<CustomDomain, sqlx::Error> {
        sqlx::query_as::<_, CustomDomain>(
            r#"
            INSERT INTO custom_domains (domain, subdomain, user_id)
            VALUES ($1, $2, $3)
            RETURNING id, domain, subdomain, user_id, verified, created_at
            "#,
        )
        .bind(domain)
        .bind(subdomain)
        .bind(user_id)
        .fetch_one(pool)
        .await
    }

    /// Verify a custom domain
    pub async fn verify_custom_domain(
        pool: &PgPool,
        domain: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("UPDATE custom_domains SET verified = TRUE WHERE domain = $1")
            .bind(domain)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Get user plan
    pub async fn get_user_plan(pool: &PgPool, user_id: Uuid) -> Result<Option<UserPlan>, sqlx::Error> {
        sqlx::query_as::<_, UserPlan>(
            "SELECT user_id, plan, max_tunnels, max_bandwidth_bytes, custom_domains_allowed FROM user_plans WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
    }

    /// Create or update user plan
    pub async fn upsert_user_plan(
        pool: &PgPool,
        user_id: Uuid,
        plan: &str,
        max_tunnels: i32,
        max_bandwidth_bytes: i64,
        custom_domains_allowed: bool,
    ) -> Result<UserPlan, sqlx::Error> {
        sqlx::query_as::<_, UserPlan>(
            r#"
            INSERT INTO user_plans (user_id, plan, max_tunnels, max_bandwidth_bytes, custom_domains_allowed)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (user_id) DO UPDATE SET
                plan = EXCLUDED.plan,
                max_tunnels = EXCLUDED.max_tunnels,
                max_bandwidth_bytes = EXCLUDED.max_bandwidth_bytes,
                custom_domains_allowed = EXCLUDED.custom_domains_allowed,
                updated_at = NOW()
            RETURNING user_id, plan, max_tunnels, max_bandwidth_bytes, custom_domains_allowed
            "#,
        )
        .bind(user_id)
        .bind(plan)
        .bind(max_tunnels)
        .bind(max_bandwidth_bytes)
        .bind(custom_domains_allowed)
        .fetch_one(pool)
        .await
    }
}
