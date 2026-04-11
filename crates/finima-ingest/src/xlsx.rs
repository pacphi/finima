//! XLS/XLSX parser using the calamine crate.

use calamine::{open_workbook_auto_from_rs, Data, Reader};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::io::Cursor;
use std::str::FromStr;

use crate::{
    csv_parser::infer_mapping, ColumnMapping, FileParser, IngestError, ParsePreview,
    RawTransaction, Result,
};

/// Parser for XLS and XLSX spreadsheet files.
pub struct XlsxParser {
    sheet: Option<String>,
}

impl XlsxParser {
    pub fn new(sheet: Option<String>) -> Self {
        Self { sheet }
    }

    /// List all sheet names in the workbook.
    pub fn parse_sheets(data: &[u8]) -> Result<Vec<String>> {
        let cursor = Cursor::new(data);
        let workbook = open_workbook_auto_from_rs(cursor)
            .map_err(|e| IngestError::SpreadsheetError(format!("Cannot open workbook: {}", e)))?;
        Ok(workbook.sheet_names().to_vec())
    }

    /// Parse a preview of the selected sheet.
    pub fn parse_preview_sheet(data: &[u8], sheet: &str) -> Result<ParsePreview> {
        let cursor = Cursor::new(data);
        let mut workbook = open_workbook_auto_from_rs(cursor)
            .map_err(|e| IngestError::SpreadsheetError(format!("Cannot open workbook: {}", e)))?;

        let range = workbook
            .worksheet_range(sheet)
            .map_err(|e| IngestError::SpreadsheetError(format!("Sheet '{}': {}", sheet, e)))?;

        let mut row_iter = range.rows();

        // First row = headers
        let headers: Vec<String> = match row_iter.next() {
            Some(row) => row.iter().map(cell_to_string).collect(),
            None => return Err(IngestError::ParseError("Sheet is empty".into())),
        };

        let inferred_mapping = infer_mapping(&headers);

        let mut rows = Vec::new();
        let mut total_count: usize = 0;
        for row in row_iter {
            total_count += 1;
            if rows.len() < 20 {
                let values: Vec<String> = row.iter().map(cell_to_string).collect();
                if !values.iter().all(|v| v.is_empty()) {
                    rows.push(values);
                }
            }
        }

        Ok(ParsePreview {
            headers,
            rows,
            inferred_mapping,
            row_count: total_count,
        })
    }

    /// Parse all rows from the selected sheet using column mapping.
    pub fn parse_all_sheet(
        data: &[u8],
        sheet: &str,
        mapping: &ColumnMapping,
    ) -> Result<Vec<RawTransaction>> {
        let cursor = Cursor::new(data);
        let mut workbook = open_workbook_auto_from_rs(cursor)
            .map_err(|e| IngestError::SpreadsheetError(format!("Cannot open workbook: {}", e)))?;

        let range = workbook
            .worksheet_range(sheet)
            .map_err(|e| IngestError::SpreadsheetError(format!("Sheet '{}': {}", sheet, e)))?;

        let mut row_iter = range.rows();
        // Skip header row
        row_iter.next();

        let mut transactions = Vec::new();

        for (row_idx, row) in row_iter.enumerate() {
            // Skip empty rows
            if row.iter().all(|c| matches!(c, Data::Empty)) {
                continue;
            }

            let date_cell = row.get(mapping.date_col).ok_or_else(|| {
                IngestError::MissingColumn(format!("Row {}: date column missing", row_idx + 1))
            })?;
            let amount_cell = row.get(mapping.amount_col).ok_or_else(|| {
                IngestError::MissingColumn(format!("Row {}: amount column missing", row_idx + 1))
            })?;
            let desc_cell = row.get(mapping.description_col).ok_or_else(|| {
                IngestError::MissingColumn(format!(
                    "Row {}: description column missing",
                    row_idx + 1
                ))
            })?;

            let date = parse_cell_date(date_cell).map_err(|_| {
                IngestError::InvalidDate(format!(
                    "Row {}: '{}'",
                    row_idx + 1,
                    cell_to_string(date_cell)
                ))
            })?;

            let amount = parse_cell_amount(amount_cell).map_err(|_| {
                IngestError::InvalidAmount(format!(
                    "Row {}: '{}'",
                    row_idx + 1,
                    cell_to_string(amount_cell)
                ))
            })?;

            let description = cell_to_string(desc_cell);
            if description.is_empty() {
                continue;
            }

            let memo = mapping
                .memo_col
                .and_then(|col| row.get(col))
                .map(cell_to_string)
                .filter(|s| !s.is_empty());

            let category = mapping
                .category_col
                .and_then(|col| row.get(col))
                .map(cell_to_string)
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

impl Default for XlsxParser {
    fn default() -> Self {
        Self::new(None)
    }
}

impl FileParser for XlsxParser {
    fn parse_preview(&self, data: &[u8]) -> Result<ParsePreview> {
        let sheet = match &self.sheet {
            Some(s) => s.clone(),
            None => {
                let sheets = XlsxParser::parse_sheets(data)?;
                sheets
                    .into_iter()
                    .next()
                    .ok_or_else(|| IngestError::ParseError("Workbook has no sheets".into()))?
            }
        };
        XlsxParser::parse_preview_sheet(data, &sheet)
    }

    fn parse_all(
        &self,
        data: &[u8],
        mapping: Option<&ColumnMapping>,
    ) -> Result<Vec<RawTransaction>> {
        let mapping = mapping.ok_or_else(|| {
            IngestError::MissingColumn("Column mapping is required for XLSX files".into())
        })?;
        let sheet = match &self.sheet {
            Some(s) => s.clone(),
            None => {
                let sheets = XlsxParser::parse_sheets(data)?;
                sheets
                    .into_iter()
                    .next()
                    .ok_or_else(|| IngestError::ParseError("Workbook has no sheets".into()))?
            }
        };
        XlsxParser::parse_all_sheet(data, &sheet, mapping)
    }
}

/// Convert an Excel cell to a display string.
fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => {
            // Check if it looks like a whole number
            if (*f).fract() == 0.0 && *f >= i64::MIN as f64 && *f <= i64::MAX as f64 {
                format!("{}", *f as i64)
            } else {
                format!("{}", f)
            }
        }
        Data::Int(i) => format!("{}", i),
        Data::Bool(b) => format!("{}", b),
        Data::Error(e) => format!("{:?}", e),
        Data::DateTime(dt) => {
            if let Some(ndt) = dt.as_datetime() {
                let d = ndt.date();
                if ndt.time() == chrono::NaiveTime::MIN {
                    d.format("%Y-%m-%d").to_string()
                } else {
                    ndt.format("%Y-%m-%d %H:%M:%S").to_string()
                }
            } else {
                format!("{}", dt.as_f64())
            }
        }
        Data::DateTimeIso(s) => s.clone(),
        Data::DurationIso(s) => s.clone(),
    }
}

/// Parse a cell value as a NaiveDate. Handles Excel serial date numbers and strings.
fn parse_cell_date(cell: &Data) -> std::result::Result<NaiveDate, ()> {
    match cell {
        Data::DateTime(dt) => dt.as_datetime().map(|ndt| ndt.date()).ok_or(()),
        Data::Float(f) => {
            // Excel serial date number: days since 1899-12-30
            excel_serial_to_date(*f as i64)
        }
        Data::Int(i) => excel_serial_to_date(*i),
        Data::String(s) | Data::DateTimeIso(s) => parse_date_string(s),
        _ => Err(()),
    }
}

/// Parse a cell value as a Decimal amount.
fn parse_cell_amount(cell: &Data) -> std::result::Result<Decimal, ()> {
    match cell {
        Data::Float(f) => {
            // Convert through string to preserve precision
            Decimal::from_str(&format!("{:.2}", f)).map_err(|_| ())
        }
        Data::Int(i) => Ok(Decimal::from(*i)),
        Data::String(s) => {
            let cleaned = s.replace(['$', ','], "").trim().to_string();
            Decimal::from_str(&cleaned).map_err(|_| ())
        }
        _ => Err(()),
    }
}

/// Convert Excel serial date number to NaiveDate.
/// Excel epoch: 1899-12-30 (with the Lotus 1-2-3 Feb 29 1900 bug).
fn excel_serial_to_date(serial: i64) -> std::result::Result<NaiveDate, ()> {
    if serial < 1 {
        return Err(());
    }
    // Match calamine's ExcelDateTime::as_datetime behavior:
    // Epoch is 1899-12-30. For serial < 60, add 1 (because serial 1 = Jan 1, 1900).
    // For serial >= 60, use as-is (the phantom Feb 29, 1900 from Lotus bug is absorbed).
    let epoch = NaiveDate::from_ymd_opt(1899, 12, 30).unwrap();
    let days = if serial >= 60 { serial } else { serial + 1 };
    epoch
        .checked_add_signed(chrono::Duration::days(days))
        .ok_or(())
}

fn parse_date_string(s: &str) -> std::result::Result<NaiveDate, ()> {
    let formats = ["%Y-%m-%d", "%m/%d/%Y", "%m/%d/%y", "%d/%m/%Y", "%m-%d-%Y"];
    for fmt in &formats {
        if let Ok(d) = NaiveDate::parse_from_str(s.trim(), fmt) {
            return Ok(d);
        }
    }
    Err(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excel_serial_date_conversion() {
        // Jan 1, 2024 = serial 45292 (>= 60, so days = 45292 from epoch 1899-12-30)
        let d = excel_serial_to_date(45292).unwrap();
        assert_eq!(d, NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
    }

    #[test]
    fn excel_serial_date_early() {
        // Serial 1 (< 60): days = 1 + 1 = 2 from epoch 1899-12-30 = 1900-01-01
        let d = excel_serial_to_date(1).unwrap();
        assert_eq!(d, NaiveDate::from_ymd_opt(1900, 1, 1).unwrap());
    }

    #[test]
    fn excel_serial_date_invalid() {
        assert!(excel_serial_to_date(0).is_err());
        assert!(excel_serial_to_date(-1).is_err());
    }

    #[test]
    fn cell_to_string_types() {
        assert_eq!(cell_to_string(&Data::String("hello".into())), "hello");
        assert_eq!(cell_to_string(&Data::Float(42.0)), "42");
        assert_eq!(cell_to_string(&Data::Float(42.5)), "42.5");
        assert_eq!(cell_to_string(&Data::Int(7)), "7");
        assert_eq!(cell_to_string(&Data::Empty), "");
        assert_eq!(cell_to_string(&Data::Bool(true)), "true");
    }

    #[test]
    fn parse_cell_amount_types() {
        assert_eq!(
            parse_cell_amount(&Data::Float(45.99)).unwrap(),
            Decimal::from_str("45.99").unwrap()
        );
        assert_eq!(
            parse_cell_amount(&Data::Int(100)).unwrap(),
            Decimal::from(100)
        );
        assert_eq!(
            parse_cell_amount(&Data::String("$1,234.56".into())).unwrap(),
            Decimal::from_str("1234.56").unwrap()
        );
    }
}
