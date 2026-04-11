//! File type detection via extension and magic bytes.

use finima_core::FileFormat;

use crate::{IngestError, Result};

/// Detect the file format from its filename extension and leading bytes.
///
/// Extension matching is tried first; magic byte validation is used as a
/// secondary signal to prevent misclassification from incorrect extensions.
pub fn detect_format(filename: &str, first_bytes: &[u8]) -> Result<FileFormat> {
    // Try extension first
    if let Some(format) = detect_by_extension(filename) {
        return Ok(format);
    }

    // Fall back to magic bytes
    if let Some(format) = detect_by_magic_bytes(first_bytes) {
        return Ok(format);
    }

    Err(IngestError::UnsupportedFormat(format!(
        "Cannot determine format for '{}'",
        filename
    )))
}

fn detect_by_extension(filename: &str) -> Option<FileFormat> {
    let lower = filename.to_lowercase();
    let ext = lower.rsplit('.').next()?;
    match ext {
        "ofx" => Some(FileFormat::Ofx),
        "qfx" => Some(FileFormat::Qfx),
        "qbo" => Some(FileFormat::Qbo),
        "qif" => Some(FileFormat::Qif),
        "csv" => Some(FileFormat::Csv),
        "tsv" => Some(FileFormat::Tsv),
        "xls" => Some(FileFormat::Xls),
        "xlsx" => Some(FileFormat::Xlsx),
        _ => None,
    }
}

fn detect_by_magic_bytes(bytes: &[u8]) -> Option<FileFormat> {
    if bytes.is_empty() {
        return None;
    }

    // OFX SGML: starts with "OFXHEADER:"
    if bytes.starts_with(b"OFXHEADER:") {
        return Some(FileFormat::Ofx);
    }

    // OFX XML: starts with "<?xml" and contains "<OFX>"
    if bytes.starts_with(b"<?xml") {
        let text = String::from_utf8_lossy(&bytes[..bytes.len().min(2048)]);
        let upper = text.to_uppercase();
        if upper.contains("<OFX>") || upper.contains("<OFX ") {
            return Some(FileFormat::Ofx);
        }
    }

    // QIF: starts with "!" (e.g. "!Type:Bank")
    if bytes.starts_with(b"!") {
        return Some(FileFormat::Qif);
    }

    // XLSX ZIP container: starts with "PK" (0x50 0x4B)
    if bytes.len() >= 2 && bytes[0] == 0x50 && bytes[1] == 0x4B {
        return Some(FileFormat::Xlsx);
    }

    // XLS OLE2 compound document: starts with 0xD0 0xCF
    if bytes.len() >= 2 && bytes[0] == 0xD0 && bytes[1] == 0xCF {
        return Some(FileFormat::Xls);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_csv_by_extension() {
        let fmt = detect_format("transactions.csv", b"Date,Amount").unwrap();
        assert_eq!(fmt, FileFormat::Csv);
    }

    #[test]
    fn detect_tsv_by_extension() {
        let fmt = detect_format("data.tsv", b"Date\tAmount").unwrap();
        assert_eq!(fmt, FileFormat::Tsv);
    }

    #[test]
    fn detect_ofx_by_extension() {
        let fmt = detect_format("bank.ofx", b"OFXHEADER:100").unwrap();
        assert_eq!(fmt, FileFormat::Ofx);
    }

    #[test]
    fn detect_qfx_by_extension() {
        let fmt = detect_format("export.qfx", b"OFXHEADER:100").unwrap();
        assert_eq!(fmt, FileFormat::Qfx);
    }

    #[test]
    fn detect_qbo_by_extension() {
        let fmt = detect_format("quickbooks.qbo", b"OFXHEADER:100").unwrap();
        assert_eq!(fmt, FileFormat::Qbo);
    }

    #[test]
    fn detect_qif_by_extension() {
        let fmt = detect_format("export.qif", b"!Type:Bank").unwrap();
        assert_eq!(fmt, FileFormat::Qif);
    }

    #[test]
    fn detect_xlsx_by_extension() {
        let fmt = detect_format("report.xlsx", b"PK\x03\x04").unwrap();
        assert_eq!(fmt, FileFormat::Xlsx);
    }

    #[test]
    fn detect_xls_by_extension() {
        let fmt = detect_format("old_report.xls", b"\xd0\xcf\x11\xe0").unwrap();
        assert_eq!(fmt, FileFormat::Xls);
    }

    #[test]
    fn detect_ofx_sgml_by_magic_bytes() {
        let fmt = detect_format("unknown.dat", b"OFXHEADER:100\nDATA:OFXSGML").unwrap();
        assert_eq!(fmt, FileFormat::Ofx);
    }

    #[test]
    fn detect_ofx_xml_by_magic_bytes() {
        let data = b"<?xml version=\"1.0\"?>\n<OFX>\n<SIGNONMSGSRSV1>";
        let fmt = detect_format("unknown.dat", data).unwrap();
        assert_eq!(fmt, FileFormat::Ofx);
    }

    #[test]
    fn detect_qif_by_magic_bytes() {
        let fmt = detect_format("unknown.dat", b"!Type:Bank\nD01/15/2024").unwrap();
        assert_eq!(fmt, FileFormat::Qif);
    }

    #[test]
    fn detect_xlsx_by_magic_bytes() {
        let fmt = detect_format("unknown.dat", b"PK\x03\x04\x14\x00").unwrap();
        assert_eq!(fmt, FileFormat::Xlsx);
    }

    #[test]
    fn detect_xls_by_magic_bytes() {
        let fmt = detect_format("unknown.dat", b"\xd0\xcf\x11\xe0\xa1\xb1").unwrap();
        assert_eq!(fmt, FileFormat::Xls);
    }

    #[test]
    fn detect_unknown_format_error() {
        let result = detect_format("photo.jpg", b"\xff\xd8\xff\xe0");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Cannot determine format"));
    }

    #[test]
    fn detect_case_insensitive_extension() {
        let fmt = detect_format("BANK.CSV", b"header").unwrap();
        assert_eq!(fmt, FileFormat::Csv);
    }
}
