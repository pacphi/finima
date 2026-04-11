-- 013_flow_groups.sql
-- Up
CREATE TABLE IF NOT EXISTS flow_groups (
    id UUID PRIMARY KEY,
    portfolio_id UUID NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_flow_groups_portfolio_id ON flow_groups (portfolio_id);

-- Add FK from account_flows.flow_group_id to flow_groups
ALTER TABLE account_flows
    ADD CONSTRAINT fk_account_flows_flow_group
    FOREIGN KEY (flow_group_id) REFERENCES flow_groups(id)
    ON DELETE SET NULL;

-- Down
-- ALTER TABLE account_flows DROP CONSTRAINT IF EXISTS fk_account_flows_flow_group;
-- DROP TABLE IF EXISTS flow_groups;
