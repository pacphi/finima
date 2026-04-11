-- 005_accounts.sql
-- Up
CREATE TABLE IF NOT EXISTS accounts (
    id UUID PRIMARY KEY,
    portfolio_id UUID NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    institution TEXT,
    account_type TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    opening_balance DECIMAL NOT NULL DEFAULT 0,
    is_primary_income BOOLEAN NOT NULL DEFAULT FALSE,
    is_archived BOOLEAN NOT NULL DEFAULT FALSE,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_accounts_portfolio_id ON accounts (portfolio_id);

-- Down
-- DROP TABLE IF EXISTS accounts;
