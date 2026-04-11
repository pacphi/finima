//! CSV/TSV parser with column inference, preview, and full parsing.

use chrono::NaiveDate;
use csv::ReaderBuilder;
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::{ColumnMapping, FileParser, IngestError, ParsePreview, RawTransaction, Result};

/// Parser for CSV and TSV files.
pub struct CsvParser {
    delimiter: u8,
}

impl CsvParser {
    pub fn new(delimiter: u8) -> Self {
        Self { delimiter }
    }

    /// Parse a preview: headers + first 20 rows, with inferred column mapping.
    pub fn parse_preview(data: &[u8], delimiter: u8) -> Result<ParsePreview> {
        let clean = strip_bom(data);
        let mut rdr = ReaderBuilder::new()
            .delimiter(delimiter)
            .flexible(true)
            .has_headers(true)
            .from_reader(clean);

        let headers: Vec<String> = rdr
            .headers()
            .map_err(|e| IngestError::ParseError(format!("Failed to read CSV headers: {}", e)))?
            .iter()
            .map(|h| h.trim().to_string())
            .collect();

        if headers.is_empty() {
            return Err(IngestError::ParseError("CSV has no headers".into()));
        }

        let inferred_mapping = infer_mapping(&headers);

        let mut rows = Vec::new();
        let mut total_count: usize = 0;
        for result in rdr.records() {
            total_count += 1;
            if rows.len() < 20 {
                let record =
                    result.map_err(|e| IngestError::ParseError(format!("CSV row error: {}", e)))?;
                let row: Vec<String> = record.iter().map(|f| f.to_string()).collect();
                rows.push(row);
            }
        }

        Ok(ParsePreview {
            headers,
            rows,
            inferred_mapping,
            row_count: total_count,
        })
    }

    /// Parse all rows using the provided column mapping.
    pub fn parse_all(
        data: &[u8],
        delimiter: u8,
        mapping: &ColumnMapping,
    ) -> Result<Vec<RawTransaction>> {
        let clean = strip_bom(data);
        let mut rdr = ReaderBuilder::new()
            .delimiter(delimiter)
            .flexible(true)
            .has_headers(true)
            .from_reader(clean);

        let mut transactions = Vec::new();

        for (row_idx, result) in rdr.records().enumerate() {
            let record = result
                .map_err(|e| IngestError::ParseError(format!("Row {}: {}", row_idx + 1, e)))?;

            // Skip empty rows
            if record.iter().all(|f| f.trim().is_empty()) {
                continue;
            }

            let date_str = record
                .get(mapping.date_col)
                .ok_or_else(|| {
                    IngestError::MissingColumn(format!("Row {}: date column missing", row_idx + 1))
                })?
                .trim();

            let amount_str = record
                .get(mapping.amount_col)
                .ok_or_else(|| {
                    IngestError::MissingColumn(format!(
                        "Row {}: amount column missing",
                        row_idx + 1
                    ))
                })?
                .trim();

            let description = record
                .get(mapping.description_col)
                .ok_or_else(|| {
                    IngestError::MissingColumn(format!(
                        "Row {}: description column missing",
                        row_idx + 1
                    ))
                })?
                .trim()
                .to_string();

            if date_str.is_empty() || amount_str.is_empty() {
                continue;
            }

            let date = parse_date(date_str).map_err(|_| {
                IngestError::InvalidDate(format!("Row {}: '{}'", row_idx + 1, date_str))
            })?;

            // Clean amount: strip currency symbols, commas, parens for negatives
            let amount = parse_amount(amount_str).map_err(|_| {
                IngestError::InvalidAmount(format!("Row {}: '{}'", row_idx + 1, amount_str))
            })?;

            let memo = mapping
                .memo_col
                .and_then(|col| record.get(col))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            let category = mapping
                .category_col
                .and_then(|col| record.get(col))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            transactions.push(RawTransaction {
                date,
                amount,
                original_description: description.clone(),
                description,
                memo,
                category,
            });
        }

        Ok(transactions)
    }
}

impl FileParser for CsvParser {
    fn parse_preview(&self, data: &[u8]) -> Result<ParsePreview> {
        CsvParser::parse_preview(data, self.delimiter)
    }

    fn parse_all(
        &self,
        data: &[u8],
        mapping: Option<&ColumnMapping>,
    ) -> Result<Vec<RawTransaction>> {
        let mapping = mapping.ok_or_else(|| {
            IngestError::MissingColumn("Column mapping is required for CSV files".into())
        })?;
        CsvParser::parse_all(data, self.delimiter, mapping)
    }
}

/// Strip UTF-8 BOM bytes if present.
fn strip_bom(data: &[u8]) -> &[u8] {
    if data.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &data[3..]
    } else {
        data
    }
}

/// Auto-detect column assignments from common header names.
pub fn infer_mapping(headers: &[String]) -> ColumnMapping {
    let lower: Vec<String> = headers.iter().map(|h| h.to_lowercase()).collect();

    let date_col = lower
        .iter()
        .position(|h| {
            matches!(
                h.as_str(),
                "date"
                    | "transaction date"
                    | "trans date"
                    | "post date"
                    | "posting date"
                    | "posted date"
            )
        })
        .unwrap_or(0);

    let amount_col = lower
        .iter()
        .position(|h| {
            matches!(
                h.as_str(),
                "amount" | "debit" | "credit" | "transaction amount" | "sum"
            )
        })
        .unwrap_or(1.min(headers.len().saturating_sub(1)));

    let description_col = lower
        .iter()
        .position(|h| {
            matches!(
                h.as_str(),
                "description"
                    | "desc"
                    | "payee"
                    | "name"
                    | "memo"
                    | "details"
                    | "transaction description"
            )
        })
        .unwrap_or(2.min(headers.len().saturating_sub(1)));

    let memo_col = lower.iter().position(|h| {
        matches!(h.as_str(), "memo" | "notes" | "reference" | "check number")
            && Some(lower.iter().position(|x| x == h).unwrap_or(usize::MAX))
                != Some(description_col)
    });

    let category_col = lower
        .iter()
        .position(|h| matches!(h.as_str(), "category" | "type" | "class"));

    ColumnMapping {
        date_col,
        amount_col,
        description_col,
        memo_col,
        category_col,
    }
}

/// Try multiple common date formats.
fn parse_date(s: &str) -> std::result::Result<NaiveDate, ()> {
    let formats = [
        "%m/%d/%Y",
        "%Y-%m-%d",
        "%m-%d-%Y",
        "%d/%m/%Y",
        "%m/%d/%y",
        "%Y/%m/%d",
        "%d-%m-%Y",
        "%b %d, %Y",
        "%B %d, %Y",
    ];
    for fmt in &formats {
        if let Ok(d) = NaiveDate::parse_from_str(s, fmt) {
            return Ok(d);
        }
    }
    Err(())
}

/// Parse an amount string, handling currency symbols, commas, and parentheses.
fn parse_amount(s: &str) -> std::result::Result<Decimal, ()> {
    let mut cleaned = s.to_string();
    // Remove currency symbols
    cleaned = cleaned.replace(['$', '€', '£', '¥'], "");
    // Remove commas
    cleaned = cleaned.replace(',', "");
    // Remove whitespace
    cleaned = cleaned.trim().to_string();
    // Handle parenthetical negatives: (123.45) -> -123.45
    if cleaned.starts_with('(') && cleaned.ends_with(')') {
        cleaned = format!("-{}", &cleaned[1..cleaned.len() - 1]);
    }
    Decimal::from_str(&cleaned).map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    const STANDARD_CSV: &[u8] = b"Date,Amount,Description,Memo,Category\n\
        01/15/2024,-45.99,Grocery Store,Weekly groceries,Food\n\
        01/16/2024,2500.00,Payroll Deposit,January salary,Income\n\
        01/17/2024,-12.50,Coffee Shop,,Dining\n";

    #[test]
    fn parse_standard_csv_preview() {
        let preview = CsvParser::parse_preview(STANDARD_CSV, b',').unwrap();
        assert_eq!(
            preview.headers,
            vec!["Date", "Amount", "Description", "Memo", "Category"]
        );
        assert_eq!(preview.rows.len(), 3);
        assert_eq!(preview.row_count, 3);
    }

    #[test]
    fn infer_common_column_names() {
        let headers = vec![
            "Transaction Date".into(),
            "Amount".into(),
            "Description".into(),
            "Memo".into(),
            "Category".into(),
        ];
        let mapping = infer_mapping(&headers);
        assert_eq!(mapping.date_col, 0);
        assert_eq!(mapping.amount_col, 1);
        assert_eq!(mapping.description_col, 2);
        assert_eq!(mapping.category_col, Some(4));
    }

    #[test]
    fn parse_csv_with_mapping() {
        let mapping = ColumnMapping {
            date_col: 0,
            amount_col: 1,
            description_col: 2,
            memo_col: Some(3),
            category_col: Some(4),
        };
        let txns = CsvParser::parse_all(STANDARD_CSV, b',', &mapping).unwrap();
        assert_eq!(txns.len(), 3);
        assert_eq!(txns[0].description, "Grocery Store");
        assert_eq!(txns[0].amount, Decimal::from_str("-45.99").unwrap());
        assert_eq!(txns[0].memo.as_deref(), Some("Weekly groceries"));
        assert_eq!(txns[0].category.as_deref(), Some("Food"));
        assert_eq!(txns[1].amount, Decimal::from_str("2500.00").unwrap());
        // Third row has empty memo
        assert!(txns[2].memo.is_none());
    }

    #[test]
    fn parse_csv_with_bom() {
        let mut data = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
        data.extend_from_slice(STANDARD_CSV);
        let preview = CsvParser::parse_preview(&data, b',').unwrap();
        assert_eq!(preview.headers[0], "Date");
    }

    #[test]
    fn parse_csv_with_quoted_fields() {
        let data = b"Date,Amount,Description\n\
            01/15/2024,-45.99,\"Grocery Store, Inc.\"\n\
            01/16/2024,100.00,\"Simple\"\n";
        let mapping = ColumnMapping {
            date_col: 0,
            amount_col: 1,
            description_col: 2,
            memo_col: None,
            category_col: None,
        };
        let txns = CsvParser::parse_all(data, b',', &mapping).unwrap();
        assert_eq!(txns.len(), 2);
        assert_eq!(txns[0].description, "Grocery Store, Inc.");
    }

    #[test]
    fn parse_csv_skips_empty_rows() {
        let data = b"Date,Amount,Description\n\
            01/15/2024,-10.00,Test\n\
            ,,\n\
            01/17/2024,-20.00,Test2\n";
        let mapping = ColumnMapping {
            date_col: 0,
            amount_col: 1,
            description_col: 2,
            memo_col: None,
            category_col: None,
        };
        let txns = CsvParser::parse_all(data, b',', &mapping).unwrap();
        assert_eq!(txns.len(), 2);
    }

    #[test]
    fn parse_tsv() {
        let data = b"Date\tAmount\tDescription\n\
            2024-01-15\t-45.99\tGrocery Store\n";
        let mapping = ColumnMapping {
            date_col: 0,
            amount_col: 1,
            description_col: 2,
            memo_col: None,
            category_col: None,
        };
        let txns = CsvParser::parse_all(data, b'\t', &mapping).unwrap();
        assert_eq!(txns.len(), 1);
        assert_eq!(txns[0].description, "Grocery Store");
    }

    #[test]
    fn parse_amount_with_currency_symbols() {
        assert_eq!(
            parse_amount("$1,234.56").unwrap(),
            Decimal::from_str("1234.56").unwrap()
        );
        assert_eq!(
            parse_amount("(50.00)").unwrap(),
            Decimal::from_str("-50.00").unwrap()
        );
        assert_eq!(
            parse_amount("€100.00").unwrap(),
            Decimal::from_str("100.00").unwrap()
        );
    }

    #[test]
    fn file_parser_trait_csv() {
        let parser = CsvParser::new(b',');
        let preview = parser.parse_preview(STANDARD_CSV).unwrap();
        assert_eq!(preview.headers.len(), 5);
    }
}
