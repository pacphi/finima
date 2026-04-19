ALTER TABLE flow_patterns
    ADD COLUMN description_embedding BYTEA NULL,
    ADD COLUMN embedding_dim INTEGER NULL;

-- Composite lookup: source-account + target-account for confirmed-pattern
-- upserts from the confirm/dismiss path. `target_account_id` is nullable
-- nowhere (it's NOT NULL per 021), so a simple composite index suffices.
CREATE INDEX idx_flow_patterns_source_target
    ON flow_patterns(source_account_id, target_account_id);
