CREATE TABLE embedding_index (
    id UUID PRIMARY KEY,
    portfolio_id UUID NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
    description TEXT NOT NULL,
    description_normalized TEXT NOT NULL,
    embedding BYTEA NULL,
    embedding_dim INTEGER NULL,
    category TEXT NOT NULL,
    subcategory TEXT NOT NULL,
    confidence DOUBLE PRECISION NOT NULL,
    source_tier TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_embedding_index_portfolio ON embedding_index(portfolio_id);
CREATE INDEX idx_embedding_index_portfolio_cat ON embedding_index(portfolio_id, category, subcategory);
