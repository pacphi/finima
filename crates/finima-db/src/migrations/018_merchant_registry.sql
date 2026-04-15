-- 018_merchant_registry.sql
-- Persistent merchant registry for Tier 0 categorization.
-- Source values: 'mcc', 'seed', 'llm_learned', 'user_defined'

-- Up
CREATE TABLE merchant_registry (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    canonical_name TEXT NOT NULL,
    aliases TEXT[] NOT NULL DEFAULT '{}',
    category TEXT NOT NULL,
    subcategory TEXT NOT NULL DEFAULT '',
    confidence DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    source TEXT NOT NULL,
    hit_count BIGINT NOT NULL DEFAULT 0,
    last_seen TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(canonical_name)
);

CREATE INDEX idx_merchant_registry_aliases ON merchant_registry USING GIN (aliases);
CREATE INDEX idx_merchant_registry_category ON merchant_registry (category);

-- Down
-- DROP INDEX IF EXISTS idx_merchant_registry_category;
-- DROP INDEX IF EXISTS idx_merchant_registry_aliases;
-- DROP TABLE IF EXISTS merchant_registry;
