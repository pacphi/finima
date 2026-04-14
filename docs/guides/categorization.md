# Transaction Categorization

This guide explains how Finima categorizes transactions — the two-layer
pipeline, the category taxonomy, and how to trigger categorization on demand.

For domain modeling details see [DDD-004](../DDDs/DDD-004-intelligence.md).
For the architecture decision behind local LLM usage see
[ADR-003](../ADRs/ADR-003-local-llm-gemma4-categorization.md).

---

## Overview

Every imported transaction goes through a two-layer categorization pipeline:

1. **User Override Pattern Matching** — instant, no LLM required.
2. **LLM Structured Tool Calling** — batched inference with confidence scoring.

User overrides always take precedence. The LLM only processes transactions
that no override pattern matches.

---

## Layer 1: User Override Patterns

Users create override rules via `POST /api/overrides`. Each rule has:

| Field                 | Description                                                |
| --------------------- | ---------------------------------------------------------- |
| `description_pattern` | Substring to match (e.g., `"starbucks"`, `"WHOLEFDS MKT"`) |
| `category`            | Target category (e.g., `food_dining`)                      |
| `subcategory`         | Target subcategory (e.g., `coffee`)                        |

**Matching logic** (`finima-llm/src/categorizer.rs`):

- Case-insensitive substring match against the transaction `description`.
- First matching override wins.
- Matched transactions receive `confidence = 1.0` and skip the LLM entirely.

Example: if the user has an override with pattern `"starbucks"`, then a
transaction with description `"STARBUCKS #12345 NEW YORK"` will be
instantly categorized without any LLM call.

---

## Layer 2: LLM Batch Categorization

Transactions not matched by overrides are sent to the LLM in **batches of 20**.

### Prompt Structure

The LLM receives three components:

1. **System prompt** (`finima-llm/src/prompts.rs`) — instructs the model to
   act as a financial transaction categorizer and call the
   `categorize_transaction` tool once per transaction.

2. **User override examples** — injected as few-shot context:

   ```text
   The user has previously categorized "WHOLEFDS MKT" as food_dining > groceries.
   ```

3. **Transaction list** — each transaction as:

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
| `confidence`        | number  | 0.0–1.0 certainty score                |

### Backend-Specific Behavior

- **Candle backend** (default): Grammar-constrained decoding ensures the
  tool-call response is always valid JSON matching the schema.
- **Ollama backend**: Response parsed defensively; malformed output handled
  gracefully.
- **Stub backend**: Returns `category="other"`, `confidence=0.5` for all
  transactions. Used when no real LLM is available.

---

## Category Taxonomy

Finima ships with 18 top-level categories. These are **externalized to YAML
configuration** (`config/default.yaml`) rather than hardcoded, so they can be
modified without recompiling the application.

### System Categories (default.yaml)

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

| Key               | Label            | Typical Transactions                  |
| ----------------- | ---------------- | ------------------------------------- |
| `housing`         | Housing          | Rent, mortgage, property tax          |
| `transportation`  | Transportation   | Gas, parking, transit, car payment    |
| `food_dining`     | Food & Dining    | Groceries, restaurants, coffee shops  |
| `utilities`       | Utilities        | Electric, water, internet, phone      |
| `healthcare`      | Healthcare       | Doctor, pharmacy, dental              |
| `insurance`       | Insurance        | Auto, health, life, home insurance    |
| `entertainment`   | Entertainment    | Streaming, movies, concerts, gaming   |
| `shopping`        | Shopping         | Retail, Amazon, clothing              |
| `personal_care`   | Personal Care    | Haircuts, gym, spa                    |
| `education`       | Education        | Tuition, books, courses               |
| `travel`          | Travel           | Hotels, flights, car rental           |
| `gifts_donations` | Gifts & Donations| Charitable giving, gifts              |
| `income`          | Income           | Salary, freelance, refunds            |
| `transfer`        | Transfer         | Between own accounts, Venmo, Zelle    |
| `fees_charges`    | Fees & Charges   | Bank fees, ATM fees, late fees        |
| `investment`      | Investment       | Brokerage, crypto, 401k contributions |
| `debt_payment`    | Debt Payment     | Student loans, credit card payments   |
| `other`           | Other            | Anything that doesn't fit above       |

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

| Method   | Endpoint               | Description                                    |
| -------- | ---------------------- | ---------------------------------------------- |
| `GET`    | `/api/categories`      | List merged system + user custom categories    |
| `POST`   | `/api/categories`      | Create a custom category (`key` + `label`)     |
| `PUT`    | `/api/categories/{key}` | Update a category label (upserts override)    |
| `DELETE` | `/api/categories/{key}` | Delete a custom category or user override     |

The `GET` endpoint returns each category with an `is_system` flag so the UI
can distinguish system categories from user-created ones.

### How Categories Reach the LLM

The LLM tool schema (`finima-llm/src/tool_defs.rs`) defines the category enum
that constrains which values the model can return. The system categories from
`config/default.yaml` provide the canonical key list used in this enum. Custom
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
Transaction (uncategorized)
        |
        v
  Override Patterns
  (substring match)
        |
   +----+----+
   |         |
 Match     No match
   |         |
   v         v
 Apply    Batch (20)
 cat/sub    |
 conf=1.0   v
          LLM Tool Call
          (categorize_transaction)
            |
            v
          Parse results
          (category, subcategory,
           merchant_name, confidence)
            |
            v
       confidence < 0.7?
        |          |
       Yes        No
        |          |
        v          v
     Flagged    Auto-accepted
        |          |
        +----+-----+
             |
             v
       WS: categorization_progress
       (after each batch)
             |
             v
       Shutdown requested? ──Yes──> Save partial results, exit
             |
             No
             v
       Update DB
       (transactions table)
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

| File                                               | Purpose                                        |
| -------------------------------------------------- | ---------------------------------------------- |
| `config/default.yaml`                              | System category definitions (key + label)      |
| `crates/finima-llm/src/categorizer.rs`             | Orchestrates the two-layer pipeline            |
| `crates/finima-llm/src/prompts.rs`                 | System and user prompt construction            |
| `crates/finima-llm/src/tool_defs.rs`               | Tool schema (category enum, subcategory, etc.) |
| `crates/finima-llm/src/tool_calling.rs`            | Parses LLM tool-call responses                 |
| `crates/finima-llm/src/client.rs`                  | `LlmClient` trait and implementations          |
| `crates/finima-api/src/handlers/categorization.rs` | Shared pipeline used by upload + on-demand     |
| `crates/finima-api/src/handlers/categories.rs`     | Category CRUD endpoints                        |
| `crates/finima-api/src/handlers/transactions.rs`   | On-demand categorization endpoints             |
| `crates/finima-db/src/repos/transaction_repo.rs`   | `find_uncategorized`, `update_llm_results`     |
| `crates/finima-db/src/migrations/015_custom_categories.sql` | Custom categories table           |
| `frontend/src/hooks/useCategories.ts`              | Category map hook with cache + labels          |
| `frontend/src/routes/SettingsPage.tsx`              | Categories management tab UI                   |
