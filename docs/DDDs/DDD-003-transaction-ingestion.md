# DDD-003: Transaction Ingestion Bounded Context

**Date:** 2026-04-10  
**Crate:** `finima-ingest`

---

## 1. Purpose

Handles the complete lifecycle of importing financial data from external files into the system. Owns file parsing, format detection, column mapping, duplicate detection, and the upload tracking workflow.

## 2. Ubiquitous Language

| Term | Definition |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --- | ------ | --- | ----------------------------------------------------------------------------------------- |
| **Upload** | A file submitted by the user for transaction import. Tracked through states: `pending` -> `processing` -> `complete` or `error`. |
| **File Format** | The detected type of an uploaded file: OFX, QFX, QBO, QIF, CSV, TSV, XLS, XLSX. |
| **Column Mapping** | User-defined assignment of CSV/XLS columns to transaction fields (date, amount, description). Required for unstructured formats. |
| **Preview** | The first 20 rows of a parsed file, shown to the user before final import. Includes inferred column assignments. |
| **Dedup Hash** | `SHA-256(date                                                                                                                                                                                                                                                                                                                      |     | amount |     | description)` scoped to an account. Used to detect duplicate transactions across imports. |
| **Raw Transaction** | A parsed row from a file before it becomes a persisted Transaction entity. |
| **Transaction Direction** | Canonical `inflow` or `outflow` relative to the account. Set at import time by `SignNormalizer` (see ADR-018). Consumed by all downstream analytics together with the canonical `amount` sign. |
| **Sign Convention** | Whether a positive _raw_ `amount` (as it appeared in the source file) on a given account represents an inflow or an outflow. Resolved at import via the chain: per-account override -> per-institution YAML rule -> autodetection -> account-type default. After resolution, both `direction` and `amount` sign are canonicalized. |
| **Canonical Amount** | The stored `transactions.amount` sign after normalization: positive = inflow, negative = outflow, regardless of source-file convention. `SUM(amount)` thus has a single, institution-agnostic meaning (net cash position). Invariant: sign of `amount` always agrees with `direction`. |

## 3. Aggregates

### Upload (Aggregate Root)

```text
Upload
  id: UUID
  account_id: UUID (FK -> Account)
  filename: String
  format: FileFormat (enum: ofx, qfx, qbo, qif, csv, tsv, xls, xlsx)
  row_count: Integer (total rows parsed)
  imported_count: Integer (rows successfully imported)
  duplicate_count: Integer (rows skipped as duplicates)
  status: UploadStatus (pending | previewing | confirmed | processing | complete | error)
  column_mapping: ColumnMapping? (JSON, for CSV/XLS only)
  error_message: String?
  uploaded_at: DateTime
```

**Invariants:**

- An upload always targets exactly one account.
- Status transitions are linear: `pending -> previewing -> confirmed -> processing -> complete` or any state -> `error`.
- `column_mapping` is required before transitioning from `previewing` to `confirmed` for CSV/XLS formats.
- `column_mapping` is auto-populated (no user input) for OFX/QFX/QIF formats.

### RawTransaction (Value Object)

```text
RawTransaction
  date: NaiveDate
  amount: Decimal
  description: String
  original_description: String (preserved, never modified)
  memo: String?
  category: String? (from source file, e.g. QIF 'L' field)
  type: String? (debit/credit indicator from OFX)
```

**Invariants:**

- `date` must be parseable from the source format.
- `amount` is the raw decimal as it appeared in the source file; its sign
  follows the _source institution's_ convention, not a canonical one. The
  import pipeline canonicalizes it via `SignNormalizer::to_canonical_amount`
  before persistence (see ADR-018).
- `description` must be non-empty.

## 4. Domain Services

### FileDetector

- `detect(filename, first_bytes) -> Result<FileFormat>` — Identifies format from extension + magic bytes.

### Parser (trait, one impl per format)

```rust
trait FileParser: Send + Sync {
    fn parse_preview(&self, data: &[u8]) -> Result<ParsePreview>;
    fn parse_all(&self, data: &[u8], mapping: Option<&ColumnMapping>) -> Result<Vec<RawTransaction>>;
}
```

Implementations: `OfxParser`, `QifParser`, `CsvParser`, `XlsxParser`.

### SignNormalizer

Institution-aware direction + canonical-amount resolver applied once per import batch. See ADR-018.

- `direction_for(ctx: AccountContext, amount: Decimal) -> TransactionDirection`
  — maps a raw `amount` to canonical `Inflow` or `Outflow` based on the configured `SignConvention` for the account.
- `direction_for_with_detection(..., detected: Option<SignConvention>) -> TransactionDirection`
  — variant that slots the `SignAutodetector` verdict into the resolution chain when neither the per-account override nor the per-institution YAML rule applies.
- `normalize(ctx, raw_amount) -> (TransactionDirection, Decimal)` /
  `normalize_with_detection(ctx, raw_amount, detected) -> (TransactionDirection, Decimal)`
  — returns both the direction and the canonical-convention amount (positive = inflow regardless of source). The import pipeline persists the canonical amount as `transactions.amount`.
- `to_canonical_amount(convention, raw_amount) -> Decimal` — low-level helper:
  passes `raw_amount` through when the source convention is already `PositiveMeansInflow`, negates it when the source is `PositiveMeansOutflow`.
- Resolution order (strongest to weakest): per-account override stored on `accounts.sign_convention_override` → `config.sign_conventions.by_institution[name]` (case-insensitive) → autodetected convention → account-type default.
- Pure function; no I/O. Built from `AppConfig.sign_conventions` at request time so per-account overrides can be merged into `by_account_id` before use.

### SignAutodetector

Fallback convention inference from the uploaded file itself; consulted only when the configured rules do not resolve a convention.

- `detect(account_type: AccountType, rows: &[RawRow]) -> AutodetectResult { verdict, confidence, reason }`
- **Liability inference:** inspects `debt_payment` category rows and payment-keyword descriptions (`"PAYMENT - THANK YOU"`, `"AUTOPAY"`, etc.). Sign of payments reveals the convention: positive payments ⇒ `PositiveMeansInflow` (Chase-style); negative payments ⇒ `PositiveMeansOutflow` (Amex/Discover-style).
- **Asset inference:** inspects payroll/deposit-keyword descriptions and `income`/`paycheck`/`payroll` categories.
- `verdict: None` when no signal is present; caller falls back to the account-type default.

### DedupService

- `compute_hash(date, amount, description) -> String` — SHA-256 hash.
- `find_duplicates(account_id, hashes) -> Vec<String>` — Returns hashes already in DB.
- `filter_new(raw_txns, existing_hashes) -> (Vec<RawTransaction>, usize)` — Returns new transactions and duplicate count.

### ImportOrchestrator

Coordinates the full import pipeline:

1. Receive file -> detect format -> create Upload record (`pending`).
2. Parse preview -> return to frontend (`previewing`).
3. Receive column mapping confirmation -> store mapping (`confirmed`).
4. Parse all rows -> **normalize via `SignNormalizer`** — resolves the account's effective `SignConvention`, flips each raw `amount` into the canonical (positive = inflow) convention, and emits the matching `direction` — with `SignAutodetector` fallback for unknown institutions -> dedup on the _raw_ amount hash so re-uploads still dedupe -> bulk insert transactions with both `direction` and canonical `amount` populated -> update Upload status (`processing` -> `complete`).
5. Queue LLM categorization for new transactions (handoff to Intelligence context).
6. Push WebSocket progress events throughout.

## 5. Domain Events

| Event                  | Triggered By                      | Consumed By                                                                                |
| ---------------------- | --------------------------------- | ------------------------------------------------------------------------------------------ |
| `UploadStarted`        | File received and format detected | WebSocket (notify client)                                                                  |
| `PreviewReady`         | First 20 rows parsed              | Frontend (show column mapping UI)                                                          |
| `ImportConfirmed`      | User confirms column mapping      | ImportOrchestrator (begin full parse)                                                      |
| `TransactionsImported` | Bulk insert complete              | Intelligence context (queue categorization), Analysis context (update recurring detection) |
| `ImportFailed`         | Parse or DB error                 | WebSocket (notify client), Upload status -> error                                          |

## 6. Context Boundaries

**This context provides to other contexts:**

- Persisted `Transaction` entities (inserted into the shared `transactions` table).
- `TransactionsImported` event consumed by Intelligence (categorization) and Analysis (recurring detection, flow detection).

**This context does NOT know about:**

- LLM categorization logic or model details.
- Budget calculations, dashboard aggregation, or flow detection.
- User authentication (receives `account_id` from the API layer, which has already validated ownership).

## 7. Error Handling Strategy

| Error                          | Behavior                                                                    |
| ------------------------------ | --------------------------------------------------------------------------- |
| Unsupported file format        | Return 400 immediately with descriptive message.                            |
| Malformed file (parse failure) | Return 400 with row-level error details (e.g., "Row 47: date field empty"). |
| Partial parse (some rows fail) | Import successful rows, report failures in Upload metadata.                 |
| Database insert failure        | Rollback transaction, set Upload status to `error`.                         |
| File too large (>50 MB)        | Reject at API layer before parsing.                                         |
