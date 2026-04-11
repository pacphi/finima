-- 006_transactions.sql
-- Up
CREATE TABLE IF NOT EXISTS transactions (
    id UUID PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    amount DECIMAL NOT NULL,
    description TEXT NOT NULL,
    original_description TEXT NOT NULL,
    category TEXT,
    subcategory TEXT,
    merchant_name TEXT,
    tags TEXT[] NOT NULL DEFAULT '{}',
    notes TEXT,
    is_recurring BOOLEAN NOT NULL DEFAULT FALSE,
    recurring_group_id UUID,
    llm_confidence DOUBLE PRECISION,
    user_overridden BOOLEAN NOT NULL DEFAULT FALSE,
    dedup_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Composite index for date range queries per account
CREATE INDEX idx_transactions_account_date ON transactions (account_id, date);

-- Unique constraint for dedup within an account
CREATE UNIQUE INDEX idx_transactions_account_dedup ON transactions (account_id, dedup_hash);

-- GIN index on tags array for tag-based queries
CREATE INDEX idx_transactions_tags ON transactions USING GIN (tags);

-- Index for category-based filtering
CREATE INDEX idx_transactions_category ON transactions (category);

-- Down
-- DROP TABLE IF EXISTS transactions;
