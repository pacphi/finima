-- 014_recurring_unique_constraint.sql
-- Add missing unique constraint required by the ON CONFLICT upsert in recurring_repo.
-- Up
CREATE UNIQUE INDEX IF NOT EXISTS idx_recurring_groups_portfolio_merchant
    ON recurring_groups (portfolio_id, merchant_name);

-- Down
-- DROP INDEX IF EXISTS idx_recurring_groups_portfolio_merchant;
