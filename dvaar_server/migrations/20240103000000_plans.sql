-- Add plan column to users table

-- Plan enum: free, pro, team
ALTER TABLE users ADD COLUMN plan TEXT NOT NULL DEFAULT 'free';

-- Stripe subscription tracking
ALTER TABLE users ADD COLUMN stripe_subscription_id TEXT;
ALTER TABLE users ADD COLUMN plan_expires_at TIMESTAMPTZ;

-- Index for quick plan lookups
CREATE INDEX idx_users_plan ON users(plan);

-- Comment for clarity
COMMENT ON COLUMN users.plan IS 'User subscription plan: free, pro, or team';
COMMENT ON COLUMN users.stripe_subscription_id IS 'Active Stripe subscription ID';
COMMENT ON COLUMN users.plan_expires_at IS 'When the current plan period ends (for grace period handling)';
