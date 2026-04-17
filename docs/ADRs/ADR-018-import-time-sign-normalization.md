# ADR-018: Import-Time Sign Normalization for Institution-Variant Exports

**Status:** Accepted
**Date:** 2026-04-16 (initial) · 2026-04-17 (canonical-amount amendment)
**Related:** ADR-005 (Multi-Format File Import), ADR-008 (Inter-Account Flow Detection), ADR-009 (YAML Configuration)

## Context

Different financial institutions export the same business event with
opposite signs. American Express and Discover credit-card statements
show charges as positive amounts and payments as negative. Chase and
Citi (for some products) export the inverse — charges negative,
payments positive. The original `flows.rs` implementation hard-coded a
single convention per `account_type` inside the visualization handler:

```rust
let is_spending = if is_credit_card {
    row.amount > Decimal::ZERO   // assumes Amex/Discover convention
} else {
    row.amount < Decimal::ZERO
};
```

This silently misclassified spending on Chase-issued cards. A direct
DB audit found that United Explorer Mastercard had 14 fully-categorized
outflow transactions per month that the handler rejected as "not
spending" — leaving the cardholder's spending categories invisible in
the Money Flow Sankey. Amazon Prime Visa exhibited the same pattern.

The defect was a **business-logic-in-presentation-layer** bug. There
is no algorithmic way to detect an institution's sign convention from
the sign of `amount` alone; it requires either configuration or
inspection of the row's surrounding context (e.g. category
=`debt_payment`).

## Decision

Every transaction row stores **both** a canonical `direction` value
(`inflow` | `outflow`) **and** a canonical `amount` sign
(`positive_means_inflow`) — computed once at import time by the
`SignNormalizer` service
(`finima_core::services::sign_normalizer`).

The stored `amount` invariant for non-zero rows is:

```text
direction = 'inflow'   <=>   amount > 0
direction = 'outflow'  <=>   amount < 0
```

Downstream consumers — Sankey, reports, cash-flow analysis, balance
computation (`accounts.current_balance = opening_balance + SUM(amount)`),
queries — can trust `SUM(amount)` to have a single, institution-
agnostic meaning (net cash position) and can trust `direction` for
fast index-friendly filtering. None need to know how the source
institution signed a given row.

### The two canonical stores

| Column                   | Meaning (post-import)                                                                  |
| ------------------------ | -------------------------------------------------------------------------------------- |
| `transactions.amount`    | Signed canonical amount. Positive == inflow, negative == outflow.                      |
| `transactions.direction` | Redundant but indexed: `'inflow'` or `'outflow'`. Always agrees with sign of `amount`. |

`direction` is kept for the `(account_id, direction, date)` composite
index (see migration 023), which lets Sankey / spending queries scan
only outflow rows without evaluating `amount > 0` per row.

### Domain types

- `TransactionDirection` (`finima_core::types`) — `Inflow | Outflow`.
  Persisted on `transactions.direction` (TEXT with CHECK).
- `AccountRole` (`finima_core::types`) — `Asset | Liability`. Derived
  from `AccountType`; not stored.
- `SignConvention` (`finima_core::services::sign_normalizer`) —
  `PositiveMeansInflow | PositiveMeansOutflow`. Persisted on
  `accounts.sign_convention_override` when the user pins a per-account
  override.

### Resolution chain

When determining the convention for an account, `SignNormalizer`
consults rules in this order, returning the first match:

1. **Per-account override** (`accounts.sign_convention_override`) — set
   by the end user via the "Flip this account" button on the Account
   detail page. Highest precedence. Never edited via YAML or CLI.
2. **Per-institution rule** (`config.sign_conventions.by_institution`)
   — maintainer-curated in `sankey.yaml`. Case-insensitive match on the
   account's institution name.
3. **Autodetected convention** (`SignAutodetector::detect`) — inspects
   the uploaded transaction file for `debt_payment` or
   payment-keyword rows on liabilities, deposit/payroll-keyword rows
   on assets, and infers the convention from their signs. Only
   consulted when no per-account or per-institution rule exists.
4. **Account-type default** (`SignConventions::with_builtin_defaults`)
   — assets default to `PositiveMeansInflow`, liabilities default to
   `PositiveMeansOutflow` (Amex/Discover convention).

### Built-in defaults

| account_type          | convention             |
| --------------------- | ---------------------- |
| checking              | positive_means_inflow  |
| savings               | positive_means_inflow  |
| cash                  | positive_means_inflow  |
| crypto                | positive_means_inflow  |
| investment_brokerage  | positive_means_inflow  |
| investment_retirement | positive_means_inflow  |
| credit_card           | positive_means_outflow |
| loan_mortgage         | positive_means_outflow |
| loan_auto             | positive_means_outflow |
| loan_student          | positive_means_outflow |
| loan_personal         | positive_means_outflow |
| other                 | positive_means_inflow  |

### YAML configuration

Maintainers add an entry to `config/sankey.yaml` only when an
institution exports the OPPOSITE polarity from the account-type
default and you want the rule shipped for ALL users of that
institution. Per-account corrections never live in YAML.

```yaml
sign_conventions:
  by_institution:
    chase: positive_means_inflow # inverse of Amex/Discover
  by_account_id: {} # rare; see schema for syntax
```

### Migration path

- **Migration 023** (`023_transactions_direction.sql`) — adds nullable
  `transactions.direction TEXT CHECK (...)` plus a composite index
  `(account_id, direction, date)` for fast Sankey queries. Nullable
  so the migration is non-blocking; downstream consumers treat NULL
  as "unknown — exclude".
- **Migration 024** (`024_accounts_sign_override.sql`) — adds nullable
  `accounts.sign_convention_override TEXT CHECK (...)` for per-account
  user pins.
- **Re-import** populates `direction` and canonical `amount` for
  fresh rows.
- **`finima-normalize-directions` CLI** (maintainer-only) has two
  passes, both idempotent:
  - Default mode: backfills `direction` for legacy rows where it is
    `NULL`, using the configured `SignNormalizer` rules.
  - `--canonicalize-amounts`: one-time pass that negates `amount` on
    every row of every account whose effective convention resolves to
    `PositiveMeansOutflow` (Amex/Discover-style), so the stored sign
    matches the canonical invariant. Detects and skips already-
    canonicalized accounts by checking whether any row on the account
    violates the `direction`/`amount` sign invariant.
- A future migration MAY add `NOT NULL` to `transactions.direction`
  once all live data is normalized.

### "Flip this account" (per-account UI override)

Setting or clearing `accounts.sign_convention_override` runs a
re-normalization pass server-side (`PUT /api/accounts/:id/sign-override`,
see `crates/finima-api/src/handlers/accounts.rs::set_sign_override`).
Because stored amounts are already canonical, the pass is a single
bulk SQL update:

```sql
UPDATE transactions
SET amount    = -amount,
    direction = CASE direction WHEN 'inflow' THEN 'outflow'
                               WHEN 'outflow' THEN 'inflow'
                               ELSE direction END
WHERE account_id = $1;
```

…gated by a resolution-chain comparison: if the new effective
convention equals the old one (e.g. the user pinned what was already
the default) no row-level work runs. The whole request is wrapped in
a DB transaction so a partial failure cannot leave rows in a mixed
state.

### Layered roles

| Layer                                           | Audience        | Mechanism                                                                                                                                                                                   | Purpose                                                                                                             |
| ----------------------------------------------- | --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `accounts.sign_convention_override`             | End user        | "Flip this account" button on the Account detail page (writes via PUT /api/accounts/:id/sign-override)                                                                                      | One-click correction when an import looks reversed. Never requires YAML or CLI.                                     |
| `sankey.yaml` `sign_conventions.by_institution` | Maintainer      | Git-tracked YAML, PR-reviewed, shipped with releases                                                                                                                                        | Ship sensible institution defaults so 95% of users never see a misclassification. Tax-bracket-table-style registry. |
| `SignAutodetector`                              | Nobody — silent | Runs during import when no institution rule matches                                                                                                                                         | Inspects payment / deposit signals in the file itself. Falls back to account-type default if inconclusive.          |
| `with_builtin_defaults`                         | Maintainer      | Code (`sign_normalizer.rs`)                                                                                                                                                                 | Last-resort sensible default by account_type.                                                                       |
| `finima-normalize-directions`                   | Maintainer/op   | Shell command: `cargo run -p finima-api --bin finima-normalize-directions -- [--dry-run]` (see [Maintainer Utilities Guide](../guides/maintainer-utilities.md#finima-normalize-directions)) | Backfill / re-normalize after a YAML rule change. Never user-facing.                                                |

## Consequences

**Positive**

- Handlers are free of institution-specific or account-type sign
  conditionals. The `flows.rs` spending-link aggregation collapsed to
  a one-line predicate (`direction == Outflow`).
- `SUM(transactions.amount)` has a single meaning (net cash
  position) regardless of the source institution, so balance,
  net-worth, and cashflow calculations work without per-row
  convention awareness.
- New institutions are onboarded by adding one YAML line. Wrong
  classifications become a configuration bug (visible, correctable)
  rather than a code bug.
- End users never see "sign convention" jargon. The Flip button
  Just Works and its server-side re-normalization keeps historical
  rows consistent — no re-import required after a Flip.
- Autodetection covers the long tail without requiring exhaustive YAML
  curation.

**Negative**

- Adds a `direction` column and two backfill steps (direction +
  amount canonicalization). Existing dev/prod data needs one run of
  `finima-normalize-directions` followed by one run of
  `finima-normalize-directions --canonicalize-amounts` (or a
  re-import).
- Users must correctly identify an account's institution for the YAML
  rule to apply. The per-account override and autodetection cover the
  cases where they don't.
- Opening balances are user-entered and must be in the canonical
  convention too — for a credit card, enter debt as a negative
  opening balance, not a positive one. Defaults to 0 so most users
  never think about it.

## Alternatives considered

- **Per-account autodetection from historical data only.** Fragile:
  requires a representative `debt_payment` row to infer sign. The
  chosen design uses autodetection as one layer in the chain, not the
  only mechanism.
- **Normalize at query time.** Spreads the logic across every consumer
  and re-pays the cost per query. Also makes the Sankey vulnerable to
  rule changes between page loads.
- **Hard-code institution rules in Rust.** Same maintenance burden as
  hard-coding sign by account_type, but moved to a different file. No
  improvement.

## Implementation references

- `crates/finima-core/src/services/sign_normalizer.rs`
- `crates/finima-core/src/services/sign_autodetector.rs`
- `crates/finima-ingest/src/normalize.rs`
- `crates/finima-api/src/handlers/uploads.rs` (import wiring)
- `crates/finima-api/src/handlers/accounts.rs` (`set_sign_override`)
- `crates/finima-api/src/bin/normalize_directions.rs` (CLI)
- `crates/finima-db/src/migrations/023_transactions_direction.sql`
- `crates/finima-db/src/migrations/024_accounts_sign_override.sql`
- `frontend/src/components/accounts/AccountSignCard.tsx`
- `config/sankey.yaml`
