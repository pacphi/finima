# finima-ingest

Multi-format file parsers for importing financial transactions from CSV, TSV, OFX, QFX, QBO, QIF, XLS, and XLSX files.

## Purpose

This crate handles the full file-import pipeline: detecting the file format, parsing raw bytes into structured `RawTransaction` records, generating user-facing previews with column mapping, and computing deduplication hashes to prevent duplicate imports. It sits between the upload handler in `finima-api` and the database persistence in `finima-db`.

## Key Types / Modules

| Module | Description |
| --------------- | -------------------------------------------------------------------------------------------------------- | --- | ------ | --- | ------------------------------------ |
| `lib.rs` | Core types: `RawTransaction`, `ColumnMapping`, `ParsePreview`, `IngestError`, and the `FileParser` trait |
| `detect.rs` | `detect_format()` -- infers `FileFormat` from file extension and content heuristics |
| `csv_parser.rs` | `CsvParser` -- handles CSV and TSV with BOM stripping and encoding normalization |
| `ofx.rs` | `OfxParser` -- parses OFX, QFX, and QBO formats (SGML-based financial interchange) |
| `qif.rs` | `QifParser` -- parses Quicken Interchange Format files |
| `xlsx.rs` | `XlsxParser` -- parses XLS and XLSX spreadsheets via the `calamine` crate |
| `preview.rs` | `generate_preview()` -- produces a `ParsePreview` with headers, sample rows, and inferred column mapping |
| `dedup.rs` | `compute_dedup_hash()` -- SHA-256 hash of `date                                                          |     | amount |     | description` for duplicate detection |

## Dependencies

Depends on **finima-core** for the `FileFormat` enum. Uses `csv` for CSV parsing, `quick-xml` for OFX/QFX XML parsing, `calamine` for Excel spreadsheet reading, `sha2` for dedup hashing, and `rust_decimal` for monetary amounts.

## Developer Top-of-Mind

- **BOM and encoding edge cases**: the CSV parser strips UTF-8 BOM markers and handles common encoding issues. When adding new format support, always test with real bank export files.
- **Deduplication uses SHA-256 hashing**: `compute_dedup_hash()` produces a deterministic hash from date, amount, and description. The API layer uses this to skip rows that already exist in the database.
- **Content truncation must respect char boundaries**: when truncating description or memo fields, use `.chars().take(n)` rather than byte slicing to avoid panics on multi-byte UTF-8.
- **The `FileParser` trait** provides a uniform interface (`parse_preview` + `parse_all`) across all formats. New format parsers should implement this trait.
- **`ColumnMapping`** lets users override auto-detected column positions. The preview flow shows inferred mappings that the user can adjust before confirming the import.

## Testing

```sh
cargo test -p finima-ingest
```

Tests cover format detection, CSV/OFX/QIF/XLSX parsing with sample data, dedup hash determinism and uniqueness, and edge cases like empty files and malformed input. No external services required.

## Import Flow

1. User uploads a file via the API (`POST /api/uploads`)
2. `detect_format()` identifies the file type from extension/content
3. The appropriate `FileParser` implementation generates a `ParsePreview`
4. The user reviews headers, sample rows, and adjusts column mapping if needed
5. On confirmation, `parse_all()` extracts all `RawTransaction` records
6. `compute_dedup_hash()` is called per row to detect duplicates against existing data
7. New (non-duplicate) transactions are persisted via `finima-db`

## Supported Formats

| Format      | Extensions             | Parser       | Notes                               |
| ----------- | ---------------------- | ------------ | ----------------------------------- |
| CSV/TSV     | `.csv`, `.tsv`         | `CsvParser`  | Handles BOM, configurable delimiter |
| OFX/QFX/QBO | `.ofx`, `.qfx`, `.qbo` | `OfxParser`  | SGML-based bank interchange format  |
| QIF         | `.qif`                 | `QifParser`  | Quicken legacy format               |
| XLS/XLSX    | `.xls`, `.xlsx`        | `XlsxParser` | Via `calamine` crate                |
