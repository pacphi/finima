//! File ingestion and parsing for Finima.
//!
//! Supports CSV, TSV, OFX, QFX, QBO, QIF, XLS, and XLSX formats.

pub mod csv_parser;
pub mod dedup;
pub mod detect;
pub mod normalize;
pub mod ofx;
pub mod preview;
pub mod qif;
pub mod xlsx;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ── Error type ───────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum IngestError {
    #[error("Unsupported file format: {0}")]
    UnsupportedFormat(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Invalid date: {0}")]
    InvalidDate(String),

    #[error("Invalid amount: {0}")]
    InvalidAmount(String),

    #[error("Missing required column: {0}")]
    MissingColumn(String),

    #[error("CSV error: {0}")]
    CsvError(#[from] csv::Error),

    #[error("XML error: {0}")]
    XmlError(#[from] quick_xml::Error),

    #[error("Spreadsheet error: {0}")]
    SpreadsheetError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, IngestError>;

// ── Core types ───────────────────────────────────────────────────────

/// A parsed row from a file before it becomes a persisted Transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawTransaction {
    pub date: NaiveDate,
    pub amount: Decimal,
    pub description: String,
    pub original_description: String,
    pub memo: Option<String>,
    pub category: Option<String>,
}

/// Mapping of CSV/XLSX columns to transaction fields.
///
/// Supports two amount modes:
/// - **Single amount**: `amount_col` is `Some(idx)` — one signed column.
/// - **Split debit/credit**: both `debit_col` and `credit_col` are `Some` —
///   debit values become negative (outflow), credit values positive (inflow).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ColumnMapping {
    pub date_col: usize,
    pub amount_col: Option<usize>,
    pub debit_col: Option<usize>,
    pub credit_col: Option<usize>,
    pub description_col: usize,
    pub memo_col: Option<usize>,
    pub category_col: Option<usize>,
}

impl ColumnMapping {
    /// Check that the mapping has a valid amount configuration.
    pub fn validate(&self) -> Result<()> {
        let has_amount = self.amount_col.is_some();
        let has_debit_credit = self.debit_col.is_some() && self.credit_col.is_some();
        if !has_amount && !has_debit_credit {
            return Err(IngestError::MissingColumn(
                "Must map either Amount or both Debit and Credit columns".into(),
            ));
        }
        Ok(())
    }
}

/// Preview of parsed file data shown to the user before final import.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsePreview {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub inferred_mapping: ColumnMapping,
    pub row_count: usize,
}

/// Trait implemented by each format-specific parser.
pub trait FileParser: Send + Sync {
    fn parse_preview(&self, data: &[u8]) -> Result<ParsePreview>;
    fn parse_all(
        &self,
        data: &[u8],
        mapping: Option<&ColumnMapping>,
    ) -> Result<Vec<RawTransaction>>;
}

// ── Re-exports ───────────────────────────────────────────────────────

pub use csv_parser::CsvParser;
pub use dedup::compute_dedup_hash;
pub use detect::detect_format;
pub use normalize::{normalize_batch, NormalizationResult};
pub use ofx::OfxParser;
pub use preview::generate_preview;
pub use qif::QifParser;
pub use xlsx::XlsxParser;
