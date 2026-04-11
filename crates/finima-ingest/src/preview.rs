//! Preview generation: routes to the correct parser based on file format.

use finima_core::FileFormat;

use crate::{
    csv_parser::CsvParser, ofx::OfxParser, qif::QifParser, xlsx::XlsxParser, IngestError,
    ParsePreview, Result,
};

/// Generate a preview for the given file data, routing to the correct parser
/// based on the detected format. Returns headers, first 20 rows, and an
/// inferred column mapping.
pub fn generate_preview(data: &[u8], format: FileFormat) -> Result<ParsePreview> {
    use crate::FileParser;

    match format {
        FileFormat::Csv => {
            let parser = CsvParser::new(b',');
            parser.parse_preview(data)
        }
        FileFormat::Tsv => {
            let parser = CsvParser::new(b'\t');
            parser.parse_preview(data)
        }
        FileFormat::Ofx | FileFormat::Qfx | FileFormat::Qbo => {
            let parser = OfxParser::new();
            parser.parse_preview(data)
        }
        FileFormat::Qif => {
            let parser = QifParser::new();
            parser.parse_preview(data)
        }
        FileFormat::Xls | FileFormat::Xlsx => {
            let parser = XlsxParser::default();
            parser.parse_preview(data)
        }
    }
}

/// Generate a preview specifically for a named sheet in a spreadsheet file.
pub fn generate_preview_sheet(
    data: &[u8],
    format: FileFormat,
    sheet: &str,
) -> Result<ParsePreview> {
    match format {
        FileFormat::Xls | FileFormat::Xlsx => XlsxParser::parse_preview_sheet(data, sheet),
        _ => Err(IngestError::ParseError(
            "Sheet selection is only supported for XLS/XLSX files".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_csv() {
        let data = b"Date,Amount,Description\n01/15/2024,-10.00,Test\n";
        let preview = generate_preview(data, FileFormat::Csv).unwrap();
        assert_eq!(preview.headers, vec!["Date", "Amount", "Description"]);
        assert_eq!(preview.row_count, 1);
    }

    #[test]
    fn preview_qif() {
        let data = b"!Type:Bank\nD01/15/2024\nT-10.00\nPTest Store\n^\n";
        let preview = generate_preview(data, FileFormat::Qif).unwrap();
        assert_eq!(preview.row_count, 1);
        assert_eq!(preview.rows[0][2], "Test Store");
    }
}
