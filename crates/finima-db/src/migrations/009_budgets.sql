-- 009_budgets.sql
-- Up
CREATE TABLE IF NOT EXISTS budgets (
    id UUID PRIMARY KEY,
    portfolio_id UUID NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
    category TEXT NOT NULL,
    monthly_limit DECIMAL NOT NULL,
    rollover BOOLEAN NOT NULL DEFAULT FALSE,
    month DATE NOT NULL
);

CREATE INDEX idx_budgets_portfolio_id ON budgets (portfolio_id);
CREATE UNIQUE INDEX idx_budgets_portfolio_category_month ON budgets (portfolio_id, category, month);

-- Down
-- DROP TABLE IF EXISTS budgets;
