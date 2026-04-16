-- Prevent duplicate flow detection: each source transaction can only
-- produce one account_flow record. The partial index (WHERE NOT NULL)
-- allows flows without a linked source transaction.
CREATE UNIQUE INDEX IF NOT EXISTS idx_account_flows_source_txn_unique
ON account_flows (source_transaction_id)
WHERE source_transaction_id IS NOT NULL;
