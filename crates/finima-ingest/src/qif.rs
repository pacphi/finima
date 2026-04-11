//! QIF parser — line-oriented state machine.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::{ColumnMapping, FileParser, IngestError, ParsePreview, RawTransaction, Result};

/// Parser for QIF (Quicken Interchange Format) files.
pub struct QifParser;

impl QifParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse QIF data into raw transactions.
    pub fn parse(data: &[u8]) -> Result<Vec<RawTransaction>> {
        let text = String::from_utf8_lossy(data);
        let mut transactions = Vec::new();

        let mut date: Option<String> = None;
        let mut amount: Option<String> = None;
        let mut payee: Option<String> = None;
        let mut memo: Option<String> = None;
        let mut category: Option<String> = None;

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let code = line.as_bytes()[0];
            let value = &line[1..];

            match code {
                b'!' => {
                    // Header line, skip (e.g. "!Type:Bank")
                }
                b'D' => {
                    date = Some(value.trim().to_string());
                }
                b'T' | b'U' => {
                    // T = amount, U = amount (duplicate in some QIF files)
                    amount = Some(value.trim().to_string());
                }
                b'P' => {
                    payee = Some(value.trim().to_string());
                }
                b'M' => {
                    memo = Some(value.trim().to_string());
                }
                b'L' => {
                    category = Some(value.trim().to_string());
                }
                b'^' => {
                    // Record separator: emit transaction
                    if let (Some(d), Some(a)) = (&date, &amount) {
                        let parsed_date = parse_qif_date(d)?;
                        let cleaned_amount = a.replace(',', "");
                        let parsed_amount = Decimal::from_str(&cleaned_amount)
                            .map_err(|e| IngestError::InvalidAmount(format!("'{}': {}", a, e)))?;

                        let description = payee.clone().unwrap_or_default();
                        if !description.is_empty() {
                            transactions.push(RawTransaction {
                                date: parsed_date,
                                amount: parsed_amount,
                                original_description: description.clone(),
                                description,
                                memo: memo.clone().filter(|s| !s.is_empty()),
                                category: category.clone().filter(|s| !s.is_empty()),
                            });
                        }
                    }
                    // Reset state
                    date = None;
                    amount = None;
                    payee = None;
                    memo = None;
                    category = None;
                }
                _ => {
                    // Ignore unknown field codes (N, C, A, etc.)
                }
            }
        }

        // Handle case where file doesn't end with ^
        if let (Some(d), Some(a)) = (&date, &amount) {
            let parsed_date = parse_qif_date(d)?;
            let cleaned_amount = a.replace(',', "");
            let parsed_amount = Decimal::from_str(&cleaned_amount)
                .map_err(|e| IngestError::InvalidAmount(format!("'{}': {}", a, e)))?;
            let description = payee.unwrap_or_default();
            if !description.is_empty() {
                transactions.push(RawTransaction {
                    date: parsed_date,
                    amount: parsed_amount,
                    original_description: description.clone(),
                    description,
                    memo: memo.filter(|s| !s.is_empty()),
                    category: category.filter(|s| !s.is_empty()),
                });
            }
        }

        if transactions.is_empty() {
            return Err(IngestError::ParseError(
                "No transactions found in QIF data".into(),
            ));
        }

        Ok(transactions)
    }
}

impl Default for QifParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FileParser for QifParser {
    fn parse_preview(&self, data: &[u8]) -> Result<ParsePreview> {
        let txns = QifParser::parse(data)?;
        let headers = vec![
            "Date".into(),
            "Amount".into(),
            "Description".into(),
            "Memo".into(),
            "Category".into(),
        ];

        let row_count = txns.len();
        let rows: Vec<Vec<String>> = txns
            .iter()
            .take(20)
            .map(|t| {
                vec![
                    t.date.format("%Y-%m-%d").to_string(),
                    t.amount.to_string(),
                    t.description.clone(),
                    t.memo.clone().unwrap_or_default(),
                    t.category.clone().unwrap_or_default(),
                ]
            })
            .collect();

        Ok(ParsePreview {
            headers,
            rows,
            inferred_mapping: ColumnMapping {
                date_col: 0,
                amount_col: 1,
                description_col: 2,
                memo_col: Some(3),
                category_col: Some(4),
            },
            row_count,
        })
    }

    fn parse_all(
        &self,
        data: &[u8],
        _mapping: Option<&ColumnMapping>,
    ) -> Result<Vec<RawTransaction>> {
        QifParser::parse(data)
    }
}

/// Parse QIF date formats. Tries multiple common formats:
/// - MM/DD/YYYY
/// - MM/DD'YY (Quicken shorthand)
/// - DD/MM/YYYY
/// - MM-DD-YYYY
/// - YYYY-MM-DD
fn parse_qif_date(s: &str) -> Result<NaiveDate> {
    let s = s.trim();

    // Handle MM/DD'YY format (Quicken shorthand with apostrophe)
    if s.contains('\'') {
        let cleaned = s.replace('\'', "/");
        // Try MM/DD/YY
        if let Ok(d) = NaiveDate::parse_from_str(&cleaned, "%m/%d/%y") {
            return Ok(d);
        }
    }

    let formats = ["%m/%d/%Y", "%m/%d/%y", "%Y-%m-%d", "%m-%d-%Y", "%d/%m/%Y"];

    for fmt in &formats {
        if let Ok(d) = NaiveDate::parse_from_str(s, fmt) {
            return Ok(d);
        }
    }

    Err(IngestError::InvalidDate(format!(
        "Cannot parse QIF date: '{}'",
        s
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    const QIF_DATA: &str = "!Type:Bank\n\
        D01/15/2024\n\
        T-45.99\n\
        PGrocery Store\n\
        MWeekly groceries\n\
        LFood\n\
        ^\n\
        D01/20/2024\n\
        T2500.00\n\
        PPayroll Deposit\n\
        MJanuary salary\n\
        LIncome\n\
        ^\n\
        D01/25/2024\n\
        T-12.50\n\
        PCoffee Shop\n\
        ^\n";

    #[test]
    fn parse_qif_basic() {
        let txns = QifParser::parse(QIF_DATA.as_bytes()).unwrap();
        assert_eq!(txns.len(), 3);
    }

    #[test]
    fn parse_qif_fields() {
        let txns = QifParser::parse(QIF_DATA.as_bytes()).unwrap();
        assert_eq!(txns[0].date, NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
        assert_eq!(txns[0].amount, Decimal::from_str("-45.99").unwrap());
        assert_eq!(txns[0].description, "Grocery Store");
        assert_eq!(txns[0].memo.as_deref(), Some("Weekly groceries"));
        assert_eq!(txns[0].category.as_deref(), Some("Food"));
    }

    #[test]
    fn parse_qif_no_memo() {
        let txns = QifParser::parse(QIF_DATA.as_bytes()).unwrap();
        assert!(txns[2].memo.is_none());
        assert!(txns[2].category.is_none());
    }

    #[test]
    fn parse_qif_record_separator() {
        // Each ^ starts a new record
        let txns = QifParser::parse(QIF_DATA.as_bytes()).unwrap();
        assert_eq!(txns[1].description, "Payroll Deposit");
        assert_eq!(txns[1].amount, Decimal::from_str("2500.00").unwrap());
    }

    #[test]
    fn parse_qif_date_apostrophe_format() {
        let data = "!Type:Bank\nD01/15'24\nT-10.00\nPTest\n^\n";
        let txns = QifParser::parse(data.as_bytes()).unwrap();
        assert_eq!(txns[0].date, NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
    }

    #[test]
    fn parse_qif_without_trailing_separator() {
        let data = "!Type:Bank\nD01/15/2024\nT-10.00\nPTest Store\n";
        let txns = QifParser::parse(data.as_bytes()).unwrap();
        assert_eq!(txns.len(), 1);
        assert_eq!(txns[0].description, "Test Store");
    }

    #[test]
    fn parse_qif_empty_error() {
        let result = QifParser::parse(b"!Type:Bank\n^\n");
        assert!(result.is_err());
    }

    #[test]
    fn qif_file_parser_preview() {
        let parser = QifParser::new();
        let preview = parser.parse_preview(QIF_DATA.as_bytes()).unwrap();
        assert_eq!(preview.row_count, 3);
        assert_eq!(
            preview.headers,
            vec!["Date", "Amount", "Description", "Memo", "Category"]
        );
    }
}
