-- 010_savings_goals.sql
-- Up
CREATE TABLE IF NOT EXISTS savings_goals (
    id UUID PRIMARY KEY,
    portfolio_id UUID NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    target_amount DECIMAL NOT NULL,
    current_amount DECIMAL NOT NULL DEFAULT 0,
    target_date DATE,
    linked_account_id UUID REFERENCES accounts(id) ON DELETE SET NULL
);

CREATE INDEX idx_savings_goals_portfolio_id ON savings_goals (portfolio_id);

-- Down
-- DROP TABLE IF EXISTS savings_goals;
