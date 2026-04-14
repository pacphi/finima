-- 015_custom_categories.sql
-- Up
CREATE TABLE IF NOT EXISTS custom_categories (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    label TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, key)
);

CREATE INDEX idx_custom_categories_user_id ON custom_categories (user_id);

-- Down
-- DROP TABLE IF EXISTS custom_categories;
