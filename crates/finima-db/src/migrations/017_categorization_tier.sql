-- 017_categorization_tier.sql
-- Track which tier assigned the category for observability.
-- Values: 'merchant_lookup', 'pattern_engine', 'semantic_search', 'llm', 'user'

-- Up
ALTER TABLE transactions ADD COLUMN source_tier TEXT DEFAULT 'llm';

-- Down
-- ALTER TABLE transactions DROP COLUMN source_tier;
