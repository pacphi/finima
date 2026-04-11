# ADR-005: Multi-Format File Import Strategy

**Status:** Accepted  
**Date:** 2026-04-10  
**Deciders:** Chris Phillipson

---

## Context

Finima's core differentiator over aggregator-dependent apps (Mint, Monarch) is direct file import. Banks worldwide export statements in various formats. Users need to import historical data from multiple institutions without reformatting.

## Decision

Support **5 file format families** with a prioritized parser pipeline in `finima-ingest`:

| Priority      | Format      | Extensions             | Parser Strategy                                         | Rust Crate                 |
| ------------- | ----------- | ---------------------- | ------------------------------------------------------- | -------------------------- |
| 1 (preferred) | OFX/QFX/QBO | `.ofx`, `.qfx`, `.qbo` | SGML/XML parse, extract `<STMTTRN>` elements            | `quick-xml` or `roxmltree` |
| 2             | QIF         | `.qif`                 | Line-oriented parser, field codes (`D`/`T`/`P`/`M`/`L`) | Custom parser              |
| 3             | CSV/TSV     | `.csv`, `.tsv`         | Column-mapping wizard; user confirms columns            | `csv` crate                |
| 4             | Excel       | `.xls`, `.xlsx`        | Sheet selector + column-mapping wizard                  | `calamine` crate           |

**File type detection:**

- Primary: file extension.
- Secondary: magic bytes (e.g., `OFXHEADER:` for OFX, `PK` for XLSX ZIP).
- Prevents misclassification from incorrect extensions.

**Import flow:**

1. Upload file (multipart POST) → backend detects type and parses headers.
2. For OFX/QFX/QIF: auto-map fields, return preview. User confirms.
3. For CSV/XLS: return column headers + first 20 rows. User maps date, amount, description columns.
4. On confirmation: parse all rows, compute `dedup_hash = SHA-256(date || amount || description)`.
5. Insert transactions, skip duplicates (by hash + account), queue LLM categorization.

**Duplicate detection:**

- Hash-based: `SHA-256(date || amount || description)` per account.
- User choice on collision: skip (default) or import anyway (e.g., two identical charges on same day).
- Hash stored in `transactions.dedup_hash` with a unique constraint per `(account_id, dedup_hash)`.

## Consequences

**Positive:**

- Covers the vast majority of bank export formats worldwide.
- OFX/QFX/QIF provide structured data with no user mapping needed — best UX.
- Column-mapping wizard for CSV/XLS handles arbitrary bank-specific formats.
- Dedup hash prevents accidental double-imports with minimal user friction.

**Negative:**

- OFX/SGML parsing is notoriously inconsistent across banks. Some files use invalid SGML that strict parsers reject. Mitigated: lenient parsing with fallback heuristics.
- CSV column mapping adds a manual step. Mitigated: auto-inference of common column names ("Date", "Amount", "Description", "Transaction Date", etc.).
- QIF is a legacy format with no formal specification. Edge cases will arise. Mitigated: test fixtures from multiple banks.
- File size limit (50 MB) may be insufficient for users importing years of data in a single file. Mitigated: chunked upload for large files.

## Alternatives Considered

1. **CSV only** — Simpler but forces users to convert OFX/QIF files manually. Bad UX for the majority of banks that offer OFX. Rejected.
2. **Plaid/aggregator API** — Automatic bank connections but violates privacy-first design, costs money, breaks frequently. Rejected for core functionality.
3. **PDF statement parsing (OCR)** — Would cover the widest range of banks but OCR accuracy is unreliable for financial data, and compute cost is high. Deferred to a future enhancement.
