-- 012_account_flows.sql
-- Up
CREATE TABLE IF NOT EXISTS account_flows (
    id UUID PRIMARY KEY,
    portfolio_id UUID NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
    source_account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    target_account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    source_transaction_id UUID REFERENCES transactions(id) ON DELETE SET NULL,
    target_transaction_id UUID REFERENCES transactions(id) ON DELETE SET NULL,
    amount DECIMAL NOT NULL,
    flow_date DATE NOT NULL,
    is_auto_detected BOOLEAN NOT NULL DEFAULT TRUE,
    is_confirmed BOOLEAN NOT NULL DEFAULT FALSE,
    flow_group_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_account_flows_portfolio_id ON account_flows (portfolio_id);
CREATE INDEX idx_account_flows_source ON account_flows (source_account_id);
CREATE INDEX idx_account_flows_target ON account_flows (target_account_id);
CREATE INDEX idx_account_flows_date ON account_flows (flow_date);

-- Down
-- DROP TABLE IF EXISTS account_flows;
