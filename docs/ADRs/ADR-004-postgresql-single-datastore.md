# ADR-004: PostgreSQL as the Single Data Store

**Status:** Accepted  
**Date:** 2026-04-10  
**Deciders:** Chris Phillipson

---

## Context

Finima stores user accounts, portfolios, transactions (potentially 50K+ per user), recurring groups, budgets, savings goals, inter-account flows, and user preferences. The system needs:

- Relational integrity (foreign keys between portfolios, accounts, transactions, flows).
- JSONB support for flexible user preferences and metadata.
- Precise decimal arithmetic for financial amounts.
- Full-text search for transaction descriptions.
- Strong aggregate query performance for dashboard computations.
- Encryption-at-rest capability for sensitive financial data.

## Decision

Use **PostgreSQL 16** as the sole persistent data store, accessed via **SQLx** (async, compile-time checked queries).

**Key schema design decisions:**

- `DECIMAL` type for all monetary amounts (never floating point).
- `JSONB` columns for `users.preferences` and `recurring_groups.metadata` — flexible, queryable, no schema migration needed for preference additions.
- `TEXT[]` arrays for transaction tags.
- `pgcrypto` extension available for field-level encryption of sensitive data.
- Indexes: B-tree on foreign keys, GIN on `tags` array, GIN on `description` for full-text search, composite index on `(account_id, date)` for range queries.
- `dedup_hash` column with unique constraint per account for duplicate detection.

**SQLx usage patterns:**

- Compile-time query checking via `sqlx::query!` macros (catches SQL errors at build time).
- `sqlx::offline` mode for CI builds without a running database.
- Migration files in `crates/finima-db/src/migrations/`, managed by `sqlx migrate`.

## Consequences

**Positive:**

- Single technology to operate, backup, and monitor.
- PostgreSQL's MVCC handles concurrent reads/writes from the household use case (1-10 users).
- JSONB eliminates need for a separate key-value store for preferences.
- Full-text search eliminates need for Elasticsearch/Meilisearch at this scale.
- `pgcrypto` provides encryption without additional infrastructure.
- SQLx compile-time checking eliminates an entire class of runtime SQL errors.

**Negative:**

- PostgreSQL is heavier than SQLite for truly single-user deployments. Mitigated: Docker Compose abstracts this; resource usage is minimal for the expected data volume.
- No built-in time-series optimization for net worth / balance history queries. Mitigated: materialized views or periodic aggregation if performance becomes an issue.
- Requires Docker or native installation, unlike embedded SQLite. Accepted trade-off for the relational integrity and JSONB features.

## Alternatives Considered

1. **SQLite (+ SQLCipher)** — Simpler deployment, embedded. But weaker concurrent write handling, no JSONB, no native full-text search, harder to scale to household (multi-user) use. Deferred: could revisit for a single-user "lite" mode.
2. **SQLite + PostgreSQL hybrid** — Use SQLite for single-user, PostgreSQL for household. Too much complexity for initial release. Rejected.
3. **MongoDB** — Good for flexible schemas but poor relational integrity. Financial data benefits from strong foreign key constraints. Rejected.
4. **DuckDB** — Excellent for analytics but not designed for OLTP writes and concurrent access. Rejected.
