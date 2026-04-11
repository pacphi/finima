-- 011_user_category_overrides.sql
-- Up
CREATE TABLE IF NOT EXISTS user_category_overrides (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    description_pattern TEXT NOT NULL,
    category TEXT NOT NULL,
    subcategory TEXT NOT NULL DEFAULT ''
);

CREATE INDEX idx_user_category_overrides_user_id ON user_category_overrides (user_id);

-- Down
-- DROP TABLE IF EXISTS user_category_overrides;
