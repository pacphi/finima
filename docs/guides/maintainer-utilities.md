# Maintainer Utilities

One-stop reference for Finima's maintainer-only command-line tools.
These binaries live in `crates/finima-api/src/bin/`, load the same
YAML/env configuration as the API server, and are never exposed to end
users. They exist for backfills, recalculations, and data audits that
fall outside the normal ingest pipeline.

## Conventions

All utilities:

- Share the API's config loader (`config/*.yaml` plus `APP__`-prefixed
  environment variables and `.env`).
- Accept `--help` / `-h` for usage and `--dry-run` where the operation
  writes to the database.
- Are **idempotent** — rerunning them on already-correct data is a
  no-op or a cheap re-check.
- Log to stdout; no WebSocket or UI side effects.

Invoke them with `cargo run -p finima-api --bin <name> -- [args]`.

---

## `merchant-audit`

**Purpose:** Identify uncategorized merchants and surface candidates for
promotion into `crates/finima-categorize/data/seed_merchants.json` so
Tier 0 (merchant lookup) can categorize them instantly on subsequent
imports, avoiding the LLM.

**When to run:** After a batch of LLM categorizations, or any time you
want a view of which descriptions are still falling through to higher
tiers.

**Source:** `crates/finima-api/src/bin/merchant_audit.rs`

**Usage:**

```sh
cargo run -p finima-api --bin merchant-audit
```

Non-interactive; prints a report to stdout.

**Report sections:**

- Total, categorized, and uncategorized transaction counts.
- Tier distribution (merchant_lookup / pattern_engine / llm / etc.).
- Top uncategorized descriptions with occurrence counts.
- **Suggested new seed merchants** — LLM-categorized merchants not yet
  in the seed registry, printed as ready-to-paste JSON snippets.

**Example output:**

```text
Merchant Audit Report
=====================

Transactions: 816 total, 519 categorized (64%), 297 uncategorized

Tier Distribution:
  merchant_lookup       267 (51%)
  pattern_engine        116 (22%)
  llm                   136 (26%)

Top Uncategorized Descriptions:
    68x  External Withdrawal - CHASE CREDIT CRD  - EPAY
    33x  External Withdrawal - AMEX EPAYMENT ER AM - ACH PMT
    ...

Suggested Seed Merchants (from LLM results, not in current seed data):
  {"name": "Optum", "aliases": ["OPTUM"], "category": "healthcare",
   "subcategory": "health_insurance"},
  ...

To add these, append them to:
  crates/finima-categorize/data/seed_merchants.json
```

**Typical workflow:**

1. Run the audit.
2. Review suggestions for accuracy.
3. Append the JSON lines to `seed_merchants.json` (inside the top-level
   array).
4. Rebuild and restart; those merchants now match via Tier 0 on the
   next ingest, no LLM call needed.

**Related docs:**
[Categorization Guide](categorization.md#tier-0-merchant-lookup)

---

## `finima-normalize-directions`

**Purpose:** Backfill and canonicalize `transactions.direction` and
`transactions.amount` so every row obeys the canonical sign invariant
(positive = inflow, negative = outflow) defined in ADR-018. The normal
ingest pipeline already does this for new rows; this tool fixes legacy
rows and re-processes rows after a YAML rule change.

**When to run:**

- After adding or editing an institution rule in
  `config/sankey.yaml` `sign_conventions.by_institution` that
  invalidates previously computed directions (e.g. adding a Bank of
  America rule after BofA rows already imported under the wrong
  default).
- After a schema change that introduces the `direction` column (initial
  migration to the ADR-018 world).
- Once, per database, to canonicalize amounts for rows imported before
  the canonical-amount convention was enforced.

**Source:** `crates/finima-api/src/bin/normalize_directions.rs`

**Usage:**

```sh
cargo run -p finima-api --bin finima-normalize-directions -- \
    [--institution NAME] [--account-id UUID] \
    [--canonicalize-amounts] [--force] [--dry-run]
```

**Modes:**

| Flag                     | Effect                                                                                                                                                   |
| ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| _(default)_              | Backfills `direction` for rows where it is `NULL`, using the current `SignNormalizer` rules. Idempotent; already-populated rows are skipped.             |
| `--force`                | Extends the default mode to **every** selected row, not just `NULL` ones. Use after a YAML rule change that invalidates stored directions.               |
| `--canonicalize-amounts` | Iterates every account whose effective sign convention is `PositiveMeansOutflow` (Amex/Discover-style) and negates `amount` to match the canonical sign. |
| `--institution NAME`     | Scope the pass to one institution.                                                                                                                       |
| `--account-id UUID`      | Scope the pass to one account.                                                                                                                           |
| `--dry-run`              | Report what would change without writing.                                                                                                                |

**Idempotency guarantees:**

- Direction backfill skips rows with a non-NULL `direction` unless
  `--force` is passed; even with `--force`, rows whose recomputed
  direction equals the stored value pay one UPDATE but no logical
  change.
- Amount canonicalization uses the `(account_id, direction, amount)`
  sign invariant to detect already-canonical accounts and leave them
  alone.

**Initial rollout (one-time):**

```sh
# 1. Backfill directions for legacy NULL rows.
cargo run -p finima-api --bin finima-normalize-directions

# 2. Canonicalize amounts on Amex/Discover-style accounts.
cargo run -p finima-api --bin finima-normalize-directions -- --canonicalize-amounts
```

**Related docs:**
[ADR-018: Import-Time Sign Normalization](../ADRs/ADR-018-import-time-sign-normalization.md)

---

## `finima-redetect-recurring`

**Purpose:** Re-run the recurring-transaction detector across one or all
portfolios and repopulate `recurring_groups`. Recurring detection
normally runs only as the last step of the categorization pipeline
(`handlers/categorization.rs`), so **restarting the server does not
re-derive recurring groups** — it just re-serves whatever the table
already contains. After a classifier change the stored rows reflect the
old algorithm until this binary (or a fresh ingest) runs.

**When to run:**

- After any change to the recurring classifier in
  `crates/finima-analysis/src/recurring.rs` (e.g. ADR-019 removed the
  Daily band and raised the fixed-cadence floor).
- After editing `config/recurring.yaml` thresholds
  (`min_occurrences_for_variable`, `variable_window_months`,
  `min_occurrences_for_fixed`).
- Any time you want a definitive re-derivation without waiting for the
  next file upload.

**Source:** `crates/finima-api/src/bin/redetect_recurring.rs`

**Usage:**

```sh
cargo run -p finima-api --bin finima-redetect-recurring -- \
    [--portfolio-id UUID] [--dry-run]
```

**Flags:**

| Flag                  | Effect                                                                                                |
| --------------------- | ----------------------------------------------------------------------------------------------------- |
| _(default)_           | Iterates every portfolio, wipes unconfirmed rows, upserts the detector's output using current config. |
| `--portfolio-id UUID` | Scope the pass to a single portfolio.                                                                 |
| `--dry-run`           | Print what would be written (first 20 candidates per portfolio) without touching the database.        |

**Idempotency:**

- **User-confirmed** recurring groups (`is_confirmed = true`) are
  preserved. Only unconfirmed rows are wiped before the upsert pass, so
  users never lose curated state.
- The upsert uses `(portfolio_id, merchant_name)` as the natural key;
  running the binary twice in a row is a no-op.

**Example output:**

```text
Redetecting recurring groups across 1 portfolio(s)
Config: RecurringDetectorConfig { min_occurrences_for_variable: 3,
        variable_window_months: 6, min_occurrences_for_fixed: 3 }
  portfolio 7b2c…: 1842 transactions → 57 candidates

Done. Deleted 63 unconfirmed row(s); upserted 57 candidate(s).
```

**Related docs:**
[ADR-019: Recurring Detection Align with Plaid](../ADRs/ADR-019-recurring-detection-align-with-plaid.md)

---

## Adding a New Utility

Follow the pattern of the binaries above:

1. Create `crates/finima-api/src/bin/<name>.rs` with a module-level
   doc comment describing purpose, usage, and idempotency guarantees.
2. Import the shared config loader via `#[path = "../config.rs"] mod config;`.
3. Register the binary in `crates/finima-api/Cargo.toml` under a new
   `[[bin]]` section.
4. Document it here (in this guide) with:
   - Purpose and "when to run" guidance
   - Complete flag table
   - Idempotency guarantees
   - A link to the ADR or guide that motivates the tool
5. If the ADR that motivated the tool pre-existed, add a reference to
   this guide from that ADR's Implementation / Layered-roles section.
