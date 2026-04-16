-- Migration 023: Add canonical direction (inflow/outflow) to every transaction.
--
-- The column is populated by the SignNormalizer service at import time
-- (see ADR-018: Import-Time Sign Normalization).
--
-- The column is intentionally NULLABLE for the inaugural rollout because
-- existing rows have no direction value yet. Two paths populate it:
--   1. Re-import via the standard CSV/OFX/QIF/XLSX flow (every new row
--      gets `direction` set by the parser).
--   2. The `finima-normalize-directions` CLI command (maintainer-only)
--      backfills NULL rows using the configured SignNormalizer rules.
--
-- A follow-up migration MAY add NOT NULL once all live data is normalized.
-- Until then, downstream queries (Sankey, reports) treat NULL as
-- "unknown — exclude" so partial normalization does not produce wrong
-- spending totals.

ALTER TABLE transactions
    ADD COLUMN direction TEXT;

ALTER TABLE transactions
    ADD CONSTRAINT transactions_direction_chk
    CHECK (direction IS NULL OR direction IN ('inflow', 'outflow'));

CREATE INDEX idx_transactions_account_direction_date
    ON transactions (account_id, direction, date);

COMMENT ON COLUMN transactions.direction IS
    'Canonical direction relative to the account: inflow or outflow. '
    'Set at import time by the SignNormalizer service (see ADR-018). '
    'NULL means a legacy row not yet normalized; Sankey queries skip NULLs.';
