# DDD-003: Transaction Ingestion Bounded Context

**Date:** 2026-04-10  
**Crate:** `finima-ingest`

---

## 1. Purpose

Handles the complete lifecycle of importing financial data from external files into the system. Owns file parsing, format detection, column mapping, duplicate detection, and the upload tracking workflow.

## 2. Ubiquitous Language

| Term                | Definition                                                                                                                       |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------------- | --- | ------ | --- | ----------------------------------------------------------------------------------------- |
| **Upload**          | A file submitted by the user for transaction import. Tracked through states: `pending` -> `processing` -> `complete` or `error`. |
| **File Format**     | The detected type of an uploaded file: OFX, QFX, QBO, QIF, CSV, TSV, XLS, XLSX.                                                  |
| **Column Mapping**  | User-defined assignment of CSV/XLS columns to transaction fields (date, amount, description). Required for unstructured formats. |
| **Preview**         | The first 20 rows of a parsed file, shown to the user before final import. Includes inferred column assignments.                 |
| **Dedup Hash**      | `SHA-256(date                                                                                                                    |     | amount |     | description)` scoped to an account. Used to detect duplicate transactions across imports. |
| **Raw Transaction** | A parsed row from a file before it becomes a persisted Transaction entity.                                                       |

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
- `amount` must be a valid decimal. Sign convention: negative = outflow, positive = inflow.
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

### DedupService

- `compute_hash(date, amount, description) -> String` — SHA-256 hash.
- `find_duplicates(account_id, hashes) -> Vec<String>` — Returns hashes already in DB.
- `filter_new(raw_txns, existing_hashes) -> (Vec<RawTransaction>, usize)` — Returns new transactions and duplicate count.

### ImportOrchestrator

Coordinates the full import pipeline:

1. Receive file -> detect format -> create Upload record (`pending`).
2. Parse preview -> return to frontend (`previewing`).
3. Receive column mapping confirmation -> store mapping (`confirmed`).
4. Parse all rows -> dedup -> bulk insert transactions -> update Upload status (`processing` -> `complete`).
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
