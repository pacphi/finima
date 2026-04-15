# Transaction Categorization

This guide explains how Finima categorizes transactions -- the four-tier
cascade engine, the category taxonomy, and how to trigger categorization on
demand.

For domain modeling details see [DDD-004](../DDDs/DDD-004-intelligence.md).
For the architecture decision behind local LLM usage see
[ADR-003](../ADRs/ADR-003-local-llm-gemma4-categorization.md).
For the tiered categorization engine design see
[ADR-012](../ADRs/ADR-012-tiered-categorization-engine.md).

---

## Overview

Every imported transaction flows through a **four-tier categorization
cascade**. Each tier handles the subset of transactions that previous tiers
could not categorize, so expensive tiers (like LLM inference) only process
the long tail of ambiguous descriptions.

> **LLM is optional.** Tiers 0-2 (merchant lookup, pattern engine, and
> semantic search) handle 80-95% of transactions without any LLM
> configured. Transactions that remain uncategorized can be manually
> categorized via the category dropdown in the Transactions page. The
> LLM tier is opt-in -- enable it with `make start LLM=ollama` or
> `make start LLM=candle` for higher accuracy on the remaining long
> tail.

| Tier | Name            | Speed   | Typical Coverage | Confidence |
| ---- | --------------- | ------- | ---------------- | ---------- |
| 0    | Merchant Lookup | < 1 ms  | 50-60%           | 0.80-0.95  |
| 1    | Pattern Engine  | < 1 ms  | 15-20%           | 0.65-0.95  |
| 2    | Semantic Search | < 10 ms | 10-15% (planned) | 0.85+      |
| 3    | LLM Inference   | 1-5 s   | 3-8%             | 0.50-0.99  |

Tiers 0 and 1 handle **65-80% of transactions instantly** (sub-millisecond),
dramatically reducing the number of expensive LLM calls. User overrides are
applied by the LLM categorizer (Tier 3) and always take precedence over any
tier.

### Source Tracking

The `source_tier` column on the `transactions` table records which tier
assigned each transaction's category. Values: `merchant_lookup`,
`pattern_engine`, `semantic_search`, `llm`, `user`.

---

## Tier 0: Merchant Lookup

The merchant registry provides instant O(1) categorization for known
merchants. It is loaded once at startup and cached on `AppState` for the
lifetime of the process.

**Data sources:**

- **Seed merchants** (`data/seed_merchants.json`) -- ~500 curated common
  merchants loaded at startup.
- **LLM-learned** -- high-confidence (>= 0.9) LLM results are automatically
  promoted to the registry via the feedback loop.
- **MCC codes** -- ISO 18245 Merchant Category Codes (when available).

**Algorithm:**

1. Normalize description (lowercase, strip digits/punctuation, collapse whitespace).
2. Exact match against the registry -- O(1) HashMap lookup.
3. If no exact match: fuzzy match via prefix index using Jaro-Winkler
   similarity (threshold >= 0.88).
4. If MCC code is available: direct category mapping.

**Implementation:** `finima-categorize/src/tier0/merchant_db.rs`

**Maintaining seed data:** Run `cargo run --bin merchant-audit` to identify
LLM-categorized merchants that are not yet in the seed data. The tool prints
JSON snippets that can be appended directly to `seed_merchants.json`. See the
[Maintainer Guide](maintainer-guide.md#merchant-audit-tool) for details.

---

## Tier 1: Pattern Engine

A regex-based pattern engine that evaluates ~35 built-in rules in a single
pass using `RegexSet`. Covers common transaction types like streaming
services, rideshare, payroll, and ATM withdrawals.

**Algorithm:**

1. Compile all patterns into a `RegexSet` (evaluated in a single pass).
2. For each unmatched transaction, test against `RegexSet`.
3. First matching pattern wins (priority-ordered).
4. Amount-range heuristics for ambiguous matches (e.g., positive amounts with
   payroll keywords -> income/salary).

**Implementation:** `finima-categorize/src/tier1/mod.rs`

---

## Tier 2: Semantic Search (Planned)

RuVector-based HNSW semantic search will handle another 10-15% of
transactions by finding similar previously-categorized descriptions.

- Embed transaction descriptions using an ONNX model (< 10 ms).
- Query the HNSW index for the 5 nearest neighbors.
- Weighted majority vote with confidence threshold >= 0.85.
- SONA self-learning adapts weights based on prediction accuracy.

**Status:** Interface defined in `finima-categorize/src/tier2/mod.rs`.
Implementation in a future phase.

---

## Tier 3: LLM Batch Inference

Transactions not matched by Tiers 0-2 are sent to the LLM. Because earlier
tiers handle the majority, typically only 3-8% of transactions reach Tier 3.

### User Override Patterns

Users create override rules via `POST /api/overrides`. Each rule has:

| Field                 | Description                                                |
| --------------------- | ---------------------------------------------------------- |
| `description_pattern` | Substring to match (e.g., `"starbucks"`, `"WHOLEFDS MKT"`) |
| `category`            | Target category (e.g., `food_dining`)                      |
| `subcategory`         | Target subcategory (e.g., `coffee`)                        |

**Matching logic** (`finima-llm/src/categorizer.rs`):

- Case-insensitive substring match against the transaction `description`.
- First matching override wins.
- Matched transactions receive `confidence = 1.0` and skip LLM inference.

### Prompt Structure

The LLM receives three components:

1. **System prompt** (`finima-llm/src/prompts.rs`) -- instructs the model to
   act as a financial transaction categorizer and call the
   `categorize_transaction` tool once per transaction.

2. **User override examples** -- injected as few-shot context:

   ```text
   The user has previously categorized "WHOLEFDS MKT" as food_dining > groceries.
   ```

3. **Transaction list** -- each transaction as:

   ```text
   1. date=2026-04-08, amount=-87.42, description="WHOLEFDS MKT #10432"
   ```

### Tool Definition

The LLM calls a structured tool (`finima-llm/src/tool_defs.rs`) with:

| Field               | Type    | Description                            |
| ------------------- | ------- | -------------------------------------- |
| `transaction_index` | integer | 1-based index matching the input list  |
| `category`          | enum    | One of 18 fixed categories (see below) |
| `subcategory`       | string  | Free-text finer classification         |
| `merchant_name`     | string  | Normalized merchant name               |
| `confidence`        | number  | 0.0-1.0 certainty score                |

### Backend-Specific Behavior

- **Candle backend** (default): Grammar-constrained decoding ensures the
  tool-call response is always valid JSON matching the schema.
- **Ollama backend**: Response parsed defensively; malformed output handled
  gracefully.
- **No LLM (`provider = "none"`)**: Tiers 0-2 still categorize transactions;
  the rest remain uncategorized (category = NULL) until a real LLM is
  configured or the user manually categorizes them.

---

## Self-Learning Feedback Loop

The categorization engine improves over time through a self-learning feedback
loop. After each LLM categorization run:

1. **LLM results with confidence >= 0.9** are automatically promoted to
   the Tier 0 merchant registry. This means the next time a transaction
   from the same merchant appears, it will be categorized instantly without
   any LLM call.

2. **User corrections** (manual category edits, payee rules) override all
   tiers and are applied with the highest priority.

3. The merchant registry grows over the lifetime of the process. After
   processing ~50,000 transactions, Tier 0 typically handles ~70% of
   transactions and Tier 3 (LLM) drops to ~5%.

**Cold-start bootstrap:**

- Minute 0: Seed merchants loaded -- Tier 0 works immediately.
- Minutes 1-10: First batch processed. Tier 3 handles most. Results feed
  back to Tier 0.
- Subsequent batches: Tier 0 coverage increases. LLM calls decrease.

---

## Category Taxonomy

Finima ships with 18 top-level categories. These are **externalized to YAML
configuration** (`config/categories.yaml`) rather than hardcoded, so they can be
modified without recompiling the application.

### System Categories (categories.yaml)

Each entry has a machine-readable `key` (used in the database and LLM tool
schema) and a human-readable `label` (shown in the UI):

```yaml
categories:
  - key: housing
    label: Housing
  - key: food_dining
    label: Food & Dining
  # ... 16 more entries
```

The full default set:

| Key               | Label             | Typical Transactions                  |
| ----------------- | ----------------- | ------------------------------------- |
| `housing`         | Housing           | Rent, mortgage, property tax          |
| `transportation`  | Transportation    | Gas, parking, transit, car payment    |
| `food_dining`     | Food & Dining     | Groceries, restaurants, coffee shops  |
| `utilities`       | Utilities         | Electric, water, internet, phone      |
| `healthcare`      | Healthcare        | Doctor, pharmacy, dental              |
| `insurance`       | Insurance         | Auto, health, life, home insurance    |
| `entertainment`   | Entertainment     | Streaming, movies, concerts, gaming   |
| `shopping`        | Shopping          | Retail, Amazon, clothing              |
| `personal_care`   | Personal Care     | Haircuts, gym, spa                    |
| `education`       | Education         | Tuition, books, courses               |
| `travel`          | Travel            | Hotels, flights, car rental           |
| `gifts_donations` | Gifts & Donations | Charitable giving, gifts              |
| `income`          | Income            | Salary, freelance, refunds            |
| `transfer`        | Transfer          | Between own accounts, Venmo, Zelle    |
| `fees_charges`    | Fees & Charges    | Bank fees, ATM fees, late fees        |
| `investment`      | Investment        | Brokerage, crypto, 401k contributions |
| `debt_payment`    | Debt Payment      | Student loans, credit card payments   |
| `other`           | Other             | Anything that doesn't fit above       |

### Custom User Categories

Users can extend the taxonomy through the **Settings > Categories** management
UI. Custom categories are stored per-user in the `custom_categories` database
table and are merged with the system categories at runtime.

The category management UI supports:

- **Adding** custom categories with a unique key and display label.
- **Editing** labels for both system and custom categories. Editing a system
  category creates a user-level override without modifying the YAML config.
- **Deleting** custom categories. System categories cannot be deleted, but
  user overrides can be removed to restore the default label.

#### API Endpoints

| Method   | Endpoint                | Description                                 |
| -------- | ----------------------- | ------------------------------------------- |
| `GET`    | `/api/categories`       | List merged system + user custom categories |
| `POST`   | `/api/categories`       | Create a custom category (`key` + `label`)  |
| `PUT`    | `/api/categories/{key}` | Update a category label (upserts override)  |
| `DELETE` | `/api/categories/{key}` | Delete a custom category or user override   |

The `GET` endpoint returns each category with an `is_system` flag so the UI
can distinguish system categories from user-created ones.

### How Categories Reach the LLM

The LLM tool schema (`finima-llm/src/tool_defs.rs`) defines the category enum
that constrains which values the model can return. The system categories from
`config/categories.yaml` provide the canonical key list used in this enum. Custom
user categories are included in the prompt context so the LLM can assign them,
but they do not modify the tool schema enum at runtime.

**Subcategories** are free-text, determined by the LLM (e.g., `food_dining` >
`groceries`, `food_dining` > `restaurants`).

---

## Confidence Scoring

The LLM assigns a confidence score (0.0–1.0) to each categorization:

| Score    | Meaning                             | Action                  |
| -------- | ----------------------------------- | ----------------------- |
| >= 0.9   | Well-known merchant, high certainty | Auto-accepted           |
| 0.7–0.89 | Reasonable certainty                | Auto-accepted           |
| < 0.7    | Ambiguous description               | Flagged for user review |

The threshold is defined in `finima-llm/src/categorizer.rs` (line 97).

---

## When Categorization Runs

### Automatic (after upload)

When a user confirms a file upload (`POST /api/uploads/{id}/confirm`), the
server:

1. Inserts transactions into the database.
2. Sets the upload status to `categorizing` so the frontend shows progress.
3. Spawns an async categorization task.
4. Sends WebSocket progress events after each batch (`categorization_progress`)
   and on completion (`categorization_complete`).
5. Triggers recurring payment detection.
6. Sets the upload status to `complete`.

The frontend polls `GET /api/uploads/{id}/status` every 2 seconds and shows a
"Categorizing transactions..." spinner while the status is `categorizing`.

### On Demand

Users can trigger categorization of uncategorized transactions via the
**Categorize Uncategorized** button on the Transactions page, or
programmatically:

```http
POST /api/transactions/categorize
Content-Type: application/json

{ "account_id": "<uuid>" }
```

Response: `202 Accepted`

```json
{
  "message": "Categorization started",
  "account_id": "...",
  "uncategorized_count": 42
}
```

Poll for completion:

```http
GET /api/transactions/categorize/status?account_id=<uuid>
```

Response (while running):

```json
{ "status": "running" }
```

Response (on completion):

```json
{
  "status": "complete",
  "total": 42,
  "flagged": 3,
  "categories": [
    { "category": "food_dining", "count": 15 },
    { "category": "shopping", "count": 12 },
    { "category": "transportation", "count": 8 }
  ]
}
```

The frontend displays a summary banner showing which categories were assigned
and how many transactions each received.

---

## Pipeline Flow Diagram

```text
Transactions (uncategorized)
        |
        v
  ┌─ Tier 0: Merchant Lookup ─┐
  │  (exact, fuzzy, MCC)       │
  │  ~50-60% matched           │
  └────────────┬───────────────┘
               |
        Remaining unmatched
               |
               v
  ┌─ Tier 1: Pattern Engine ──┐
  │  (RegexSet + heuristics)   │
  │  ~15-20% matched           │
  └────────────┬───────────────┘
               |
        Remaining unmatched
               |
               v
       Persist cascade results
       (source_tier = merchant_lookup | pattern_engine)
               |
               v
  ┌─ Tier 3: LLM Inference ──┐
  │  Override patterns applied │
  │  Batched tool-calling      │
  │  ~3-8% remaining           │
  └────────────┬───────────────┘
               |
               v
       Persist LLM results
       (source_tier = llm)
               |
               v
  ┌─ Feedback Loop ───────────┐
  │  confidence >= 0.9?        │
  │  Yes -> add to Tier 0      │
  │         merchant registry  │
  └────────────┬───────────────┘
               |
               v
       WS: categorization_complete
               |
               v
       Recurring Detection
```

---

## Graceful Shutdown

When the server receives a shutdown signal (Ctrl-C / SIGINT), in-flight
categorization tasks are cancelled cooperatively between LLM batches:

1. The shutdown handler sets an `AtomicBool` flag on `AppState`.
2. The categorizer's progress callback checks this flag after each batch of 20
   transactions. If set, it returns `false` to stop the batch loop.
3. **Partial results are persisted** — transactions already categorized are
   saved to the database. Remaining uncategorized transactions can be picked up
   on the next run via the on-demand categorize endpoint.
4. Recurring detection is skipped during shutdown to exit promptly.

At most one batch worth of LLM inference (~20 transactions) is in-flight when
the cancellation signal arrives. The upload status is set to `complete` after
the task exits so it does not remain stuck in `categorizing`.

### Key files

| File                                               | Role                                              |
| -------------------------------------------------- | ------------------------------------------------- |
| `crates/finima-api/src/state.rs`                   | `signal_shutdown()` / `is_shutting_down()`        |
| `crates/finima-api/src/main.rs`                    | Calls `signal_shutdown()` on Ctrl-C               |
| `crates/finima-llm/src/categorizer.rs`             | Progress callback returns `bool` for cancellation |
| `crates/finima-api/src/handlers/categorization.rs` | Checks shutdown flag in callback                  |

---

## Key Source Files

| File                                                          | Purpose                                                       |
| ------------------------------------------------------------- | ------------------------------------------------------------- |
| `config/categories.yaml`                                      | System category definitions (key + label)                     |
| `crates/finima-categorize/src/lib.rs`                         | Cascade engine entry point + public API                       |
| `crates/finima-categorize/src/tier0/merchant_db.rs`           | Tier 0: Merchant registry with fuzzy matching                 |
| `crates/finima-categorize/src/tier1/mod.rs`                   | Tier 1: RegexSet pattern engine + amount heuristics           |
| `crates/finima-categorize/src/tier2/mod.rs`                   | Tier 2: Semantic search trait (planned)                       |
| `crates/finima-categorize/src/engine.rs`                      | CascadeEngine orchestrating tiers 0-1                         |
| `crates/finima-categorize/data/seed_merchants.json`           | ~500 curated merchant entries for Tier 0                      |
| `crates/finima-llm/src/categorizer.rs`                        | Tier 3: LLM batch categorization with overrides               |
| `crates/finima-llm/src/prompts.rs`                            | System and user prompt construction                           |
| `crates/finima-llm/src/tool_defs.rs`                          | Tool schema (category enum, subcategory, etc.)                |
| `crates/finima-llm/src/tool_calling.rs`                       | Parses LLM tool-call responses                                |
| `crates/finima-llm/src/client.rs`                             | `LlmClient` trait and implementations                         |
| `crates/finima-api/src/handlers/categorization.rs`            | Cascade + LLM pipeline with feedback loop                     |
| `crates/finima-api/src/state.rs`                              | AppState with cached MerchantRegistry                         |
| `crates/finima-api/src/handlers/categories.rs`                | Category CRUD endpoints                                       |
| `crates/finima-api/src/handlers/transactions.rs`              | On-demand categorization endpoints                            |
| `crates/finima-db/src/repos/transaction_repo.rs`              | `find_uncategorized`, `update_llm_results`, `set_source_tier` |
| `crates/finima-db/src/migrations/015_custom_categories.sql`   | Custom categories table                                       |
| `crates/finima-db/src/migrations/017_categorization_tier.sql` | `source_tier` column on transactions                          |
| `frontend/src/hooks/useCategories.ts`                         | Category map hook with cache + labels                         |
| `frontend/src/routes/SettingsPage.tsx`                        | Categories management tab UI                                  |
