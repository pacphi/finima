-- Flow patterns: learned transfer description → target account mappings
-- Used by SONA-enhanced flow detection (ADR-017) to resolve one-sided flows
-- and improve matching accuracy over time.

CREATE TABLE flow_patterns (
    id UUID PRIMARY KEY,
    portfolio_id UUID NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
    description_text TEXT NOT NULL,
    source_account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    target_account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    confidence DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    match_count INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_flow_patterns_portfolio_id ON flow_patterns(portfolio_id);
CREATE INDEX idx_flow_patterns_source ON flow_patterns(source_account_id);
