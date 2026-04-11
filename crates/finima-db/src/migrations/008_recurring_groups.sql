-- 008_recurring_groups.sql
-- Up
CREATE TABLE IF NOT EXISTS recurring_groups (
    id UUID PRIMARY KEY,
    portfolio_id UUID NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
    merchant_name TEXT NOT NULL,
    category TEXT NOT NULL,
    frequency TEXT NOT NULL,
    avg_amount DECIMAL NOT NULL,
    is_confirmed BOOLEAN NOT NULL DEFAULT FALSE,
    next_expected_date DATE,
    metadata JSONB NOT NULL DEFAULT '{}'
);

CREATE INDEX idx_recurring_groups_portfolio_id ON recurring_groups (portfolio_id);

-- Add FK from transactions.recurring_group_id to recurring_groups
ALTER TABLE transactions
    ADD CONSTRAINT fk_transactions_recurring_group
    FOREIGN KEY (recurring_group_id) REFERENCES recurring_groups(id)
    ON DELETE SET NULL;

-- Down
-- ALTER TABLE transactions DROP CONSTRAINT IF EXISTS fk_transactions_recurring_group;
-- DROP TABLE IF EXISTS recurring_groups;
