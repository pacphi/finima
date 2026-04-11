-- 002_magic_links.sql
-- Up
CREATE TABLE IF NOT EXISTS magic_links (
    id UUID PRIMARY KEY,
    email TEXT NOT NULL,
    token_hash TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ
);

CREATE INDEX idx_magic_links_token_hash ON magic_links (token_hash);
CREATE INDEX idx_magic_links_email ON magic_links (email);

-- Down
-- DROP TABLE IF EXISTS magic_links;
