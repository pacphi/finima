# DDD-004: Intelligence Bounded Context

**Date:** 2026-04-10  
**Crates:** `finima-llm` + `finima-analysis` (recurring detection)

---

## 1. Purpose

Owns all LLM-powered features: transaction categorization, merchant name normalization, recurring payment detection and enrichment, and insight generation. This context transforms raw transaction data into structured, actionable financial intelligence. It includes hardware-aware model selection (GPU, Apple Silicon, CPU) and multi-backend support (Candle/mistral.rs, Ollama, stub fallback) to run inference optimally on any deployment target.

## 2. Ubiquitous Language

| Term                             | Definition                                                                                                                                                                                                                                                            |
| -------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Categorization**               | Assigning a `category` and `subcategory` to a transaction using the LLM's structured tool-calling capability.                                                                                                                                                         |
| **Category**                     | A top-level spending classification. Fixed enum: housing, transportation, food_dining, utilities, healthcare, insurance, entertainment, shopping, personal_care, education, travel, gifts_donations, income, transfer, fees_charges, investment, debt_payment, other. |
| **Subcategory**                  | A finer classification within a category (e.g., food_dining > restaurants, food_dining > groceries). Free-text, LLM-determined.                                                                                                                                       |
| **Confidence Score**             | A 0.0-1.0 value from the LLM indicating certainty of categorization. Threshold: >= 0.7 auto-accepted, < 0.7 flagged for review.                                                                                                                                       |
| **User Override**                | When a user manually changes a transaction's category. Stored in `user_category_overrides` and used as few-shot examples in future prompts.                                                                                                                           |
| **Batch**                        | A group of up to 20 transactions sent to the LLM in a single call for throughput.                                                                                                                                                                                     |
| **Recurring Group**              | A cluster of transactions from the same merchant at a regular interval (daily, weekly, monthly, etc.).                                                                                                                                                                |
| **Frequency**                    | The detected interval of a recurring group: daily, weekly, biweekly, monthly, quarterly, semiannual, annual, or variable.                                                                                                                                             |
| **Backend / Provider**           | The inference engine used to run the LLM. Options: `candle` (in-process via mistral.rs), `ollama` (HTTP to external Ollama process), `stub` (canned fallback).                                                                                                        |
| **Hardware Profile**             | Detected hardware capabilities (CUDA GPU, Metal/Apple Silicon, CPU with SIMD) used to auto-select optimal model variant and quantization.                                                                                                                             |
| **Model Resolution**             | The process of mapping a hardware profile to a specific Gemma 4 variant (26B-A4B, E4B, or E2B) and quantization level (Q4_K_M).                                                                                                                                       |
| **Grammar-Constrained Decoding** | A technique (used by the Candle/mistral.rs backend) that forces LLM token generation to produce valid JSON matching the tool schema, guaranteeing structural correctness.                                                                                             |

## 3. Aggregates

### CategorizationJob (short-lived, not persisted as aggregate)

```text
CategorizationJob
  upload_id: UUID (source)
  account_id: UUID
  transactions: Vec<TransactionRef> (id, date, amount, description)
  user_overrides: Vec<OverridePattern> (from user_category_overrides table)
  batches: Vec<Batch> (chunked into groups of 20)
  progress: (completed: usize, total: usize, flagged: usize)
```

**Invariants:**

- Only uncategorized transactions (`category IS NULL`) are included.
- Batch size never exceeds 20 transactions.
- User overrides are loaded once per job and injected into every batch prompt.

### RecurringGroup (Aggregate Root)

```text
RecurringGroup
  id: UUID
  portfolio_id: UUID
  merchant_name: String (normalized by LLM)
  category: String
  frequency: Frequency (enum)
  avg_amount: Decimal
  is_confirmed: bool (false until user reviews)
  next_expected_date: Date?
  metadata: JSONB { annual_cost, is_subscription, is_bill, is_income, enrichment_confidence }
```

**Invariants:**

- A recurring group requires at least 2 matching transactions.
- `frequency` is computed from inter-transaction intervals with defined tolerances per frequency type.
- `next_expected_date` is projected from the last occurrence + frequency interval.
- `avg_amount` is the mean of all transactions in the group.

### UserCategoryOverride (Entity)

```text
UserCategoryOverride
  id: UUID
  user_id: UUID
  description_pattern: String (e.g., "WHOLEFDS MKT", "SQ *GREENLEAF")
  category: String
  subcategory: String
```

**Invariants:**

- Patterns are matched as substring (case-insensitive) against transaction descriptions.
- A user can have at most one override per pattern (upsert on conflict).

## 4. Domain Services

### LlmClient (trait)

```rust
#[async_trait]
trait LlmClient: Send + Sync {
    async fn categorize_batch(&self, batch: &CategorizationBatch) -> Result<Vec<CategorizationResult>>;
    async fn enrich_recurring(&self, group: &RecurringGroupCandidate) -> Result<RecurringEnrichment>;
    async fn summarize_article(&self, title: &str, content: &str) -> Result<ArticleSummary>;
    async fn generate_flow_insight(&self, flow_data: &FlowAnalysis) -> Result<String>;
}
```

Implementations:

- `CandleClient` (in-process via mistral.rs/Candle; grammar-constrained tool calling, hardware auto-detection). **Primary/default.**
- `OllamaClient` (HTTP to Ollama `/api/chat`; external process required). **Alternative.**
- `StubLlmClient` (returns `category="other"`, `confidence=0.5`; no LLM). **Fallback when no backend available.**

### Categorizer

- `categorize_new_transactions(account_id, transaction_ids) -> Result<CategorizationReport>`
  1. Load uncategorized transactions.
  2. Load user overrides — apply pattern matches first (instant, no LLM needed).
  3. Batch remaining transactions (max 20 per batch).
  4. For each batch: construct prompt with tool definition + few-shot override examples, send to LLM.
  5. Parse tool-call responses, update transactions, flag low-confidence results.
  6. Emit progress events via WebSocket.

### RecurringDetector

- `detect_recurring(portfolio_id) -> Result<Vec<RecurringGroupCandidate>>`
  1. Group transactions by normalized `merchant_name`.
  2. For groups with >= 2 transactions, compute inter-date intervals.
  3. Match intervals to frequency patterns with tolerances:
     - Monthly: 28-31 days +/- 3
     - Weekly: 7 days +/- 1
     - Biweekly: 14 days +/- 2
     - Quarterly: 85-95 days +/- 5
     - Semiannual: 175-190 days +/- 10
     - Annual: 355-375 days +/- 15
  4. Enrich each candidate via LLM (merchant name, subscription/bill/income type, annual cost).
  5. Create or update `recurring_groups` records.

### Hardware Detection

- `detect_hardware() -> HardwareProfile`
  1. Probe for CUDA GPU via cudarc (device count, VRAM, compute capability).
  2. Detect Apple Silicon Metal (aarch64 + macOS + unified memory).
  3. Detect CPU SIMD features (AVX2, AVX-512, NEON).
  4. Report system RAM.
- `resolve_model(profile: &HardwareProfile, user_config: &str) -> ModelSelection`
  - If `user_config != "auto"`: use explicit model.
  - If 16+ GB available: `gemma-4-26B-A4B-it` Q4_K_M (MoE, best quality).
  - If 8-16 GB: `gemma-4-E4B-it` Q4_K_M (PLE, good balance).
  - If < 8 GB: `gemma-4-E2B-it` Q4_K_M (PLE, resource-constrained).

## 5. Domain Events

| Event                     | Triggered By                    | Consumed By                                        |
| ------------------------- | ------------------------------- | -------------------------------------------------- |
| `CategorizationRequested` | TransactionsImported event      | Categorizer service                                |
| `BatchCategorized`        | One batch of 20 processed       | WebSocket (progress), Categorizer (next batch)     |
| `CategorizationComplete`  | All batches done                | WebSocket (final event), Dashboard (refresh)       |
| `CategoryOverridden`      | User manually changes category  | Categorizer (reload overrides), retroactive update |
| `RecurringDetected`       | Recurring detection completes   | WebSocket (badge notification), Recurring page     |
| `RecurringConfirmed`      | User confirms a detected group  | Upcoming bills widget                              |
| `RecurringDismissed`      | User dismisses a false positive | RecurringDetector (exclude in future runs)         |

## 6. Context Boundaries

**This context provides to other contexts:**

- Categorized transactions (category, subcategory, merchant_name, confidence) written to the shared `transactions` table.
- Recurring group data consumed by Dashboard (upcoming bills), Budget (auto-suggest), and Flows (transfer identification).

**This context consumes from other contexts:**

- `TransactionsImported` event from Ingestion context.
- User override data from the shared database.
- Account and portfolio IDs for scoping queries.

**This context does NOT know about:**

- File parsing, column mapping, or dedup logic.
- Dashboard rendering, chart generation, or UI concerns.
- Budget targets or savings goals.

## 7. LLM Interaction Pattern

```text
System Prompt:
  "You are a financial transaction categorizer. Use the categorize_transaction
   tool to classify each transaction."

Tool Definition:
  { name: "categorize_transaction", parameters: { category, subcategory, merchant_name, confidence } }

User Overrides (few-shot):
  "The user has previously categorized 'WHOLEFDS MKT' as food_dining > groceries."
  "The user has previously categorized 'SQ *GREENLEAF' as food_dining > restaurants."

Transactions:
  [{ date: "2026-04-08", amount: -87.42, description: "WHOLEFDS MKT #10432" }, ...]

Expected Response:
  Tool call with structured JSON per transaction.
```

**Backend-specific behavior:**

- **Candle backend:** Grammar-constrained decoding ensures the tool-call response is always valid JSON matching the schema. Malformed output is structurally impossible.
- **Ollama backend:** The response is parsed defensively; malformed output is handled gracefully (see Section 8 — Resilience).

## 8. Resilience

- **LLM timeout:** 60-second timeout per batch. Retry once after 5 seconds. On second failure, mark batch as `categorization_failed`.
- **LLM unavailable:** Queue jobs. Alert user that categorization is paused. Transactions remain uncategorized until model comes online.
- **Malformed LLM output:** Parse defensively. If a transaction's result can't be extracted, flag it for review rather than crashing the batch.

## 9. Test vs. Production Boundaries

- **Mock LLM server:** A mock HTTP server returning canned tool-call responses is used **only in `test` and `development` environments** (`APP_ENV=test` or `APP_ENV=development`). It enables fast, deterministic integration tests without requiring a GPU or real model.
- **Production (`APP_ENV=production`):** Always connects to the real Candle or Ollama endpoint configured in `config/production.yaml`. No mock fallbacks, no canned responses, no test data.
- The `LlmClient` trait implementation is selected at startup based on configuration (`llm.provider`):
  - `"candle"` (default) → `CandleClient` with hardware auto-detection.
  - `"ollama"` → `OllamaClient` pointing at configured URL.
  - `"stub"` or misconfigured → `StubLlmClient`.
    There is no runtime mock switching in production.
