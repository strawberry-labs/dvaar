-- Custom domains table for CNAME support
CREATE TABLE custom_domains (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    domain TEXT UNIQUE NOT NULL,           -- e.g., "api.mycompany.com"
    subdomain TEXT NOT NULL,               -- e.g., "myapp" (maps to myapp.dvaar.app)
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    verified BOOLEAN DEFAULT FALSE,        -- DNS verification status
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

CREATE INDEX idx_custom_domains_domain ON custom_domains(domain);
CREATE INDEX idx_custom_domains_subdomain ON custom_domains(subdomain);
CREATE INDEX idx_custom_domains_user_id ON custom_domains(user_id);

-- User plans table
CREATE TABLE user_plans (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    plan TEXT NOT NULL DEFAULT 'free',     -- free, hobby, pro
    max_tunnels INT NOT NULL DEFAULT 3,
    max_bandwidth_bytes BIGINT NOT NULL DEFAULT 1073741824,  -- 1GB
    custom_domains_allowed BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

-- Trigger for user_plans updated_at
CREATE TRIGGER update_user_plans_updated_at
    BEFORE UPDATE ON user_plans
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Create default plan for existing users
INSERT INTO user_plans (user_id, plan, max_tunnels, max_bandwidth_bytes, custom_domains_allowed)
SELECT id, 'free', 3, 1073741824, FALSE FROM users
ON CONFLICT (user_id) DO NOTHING;
