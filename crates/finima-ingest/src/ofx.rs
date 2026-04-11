//! OFX/QFX/QBO parser supporting both SGML and XML styles.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::{ColumnMapping, FileParser, IngestError, ParsePreview, RawTransaction, Result};

/// Parser for OFX, QFX, and QBO files.
pub struct OfxParser;

impl OfxParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse OFX data (SGML or XML) into raw transactions.
    pub fn parse(data: &[u8]) -> Result<Vec<RawTransaction>> {
        let text = String::from_utf8_lossy(data);
        let text = text.trim();

        if text.is_empty() {
            return Err(IngestError::ParseError("Empty OFX data".into()));
        }

        Self::extract_transactions(text)
    }

    fn extract_transactions(text: &str) -> Result<Vec<RawTransaction>> {
        let mut transactions = Vec::new();
        let upper = text.to_uppercase();

        // Find all <STMTTRN> ... </STMTTRN> blocks
        let mut search_from = 0;
        while let Some(pos) = upper[search_from..].find("<STMTTRN>") {
            let start = search_from + pos;
            let end = match upper[start..].find("</STMTTRN>") {
                Some(pos) => start + pos + "</STMTTRN>".len(),
                None => {
                    // Lenient: try to find next <STMTTRN> or end of data
                    match upper[start + 9..].find("<STMTTRN>") {
                        Some(pos) => start + 9 + pos,
                        None => text.len(),
                    }
                }
            };

            let block = &text[start..end];
            if let Ok(txn) = Self::parse_transaction_block(block) {
                transactions.push(txn);
            }
            search_from = end;
        }

        if transactions.is_empty() {
            return Err(IngestError::ParseError(
                "No <STMTTRN> elements found in OFX data".into(),
            ));
        }

        Ok(transactions)
    }

    fn parse_transaction_block(block: &str) -> Result<RawTransaction> {
        let dtposted = Self::extract_tag_value(block, "DTPOSTED")
            .ok_or_else(|| IngestError::InvalidDate("Missing DTPOSTED".into()))?;
        let trnamt = Self::extract_tag_value(block, "TRNAMT")
            .ok_or_else(|| IngestError::InvalidAmount("Missing TRNAMT".into()))?;

        let name = Self::extract_tag_value(block, "NAME").unwrap_or_default();
        let memo = Self::extract_tag_value(block, "MEMO");
        let _trntype = Self::extract_tag_value(block, "TRNTYPE");

        let date = Self::parse_ofx_date(&dtposted)?;
        let amount = Decimal::from_str(trnamt.trim())
            .map_err(|e| IngestError::InvalidAmount(format!("'{}': {}", trnamt, e)))?;

        let description = if name.is_empty() {
            memo.clone().unwrap_or_default()
        } else {
            name.clone()
        };

        if description.is_empty() {
            return Err(IngestError::ParseError(
                "Transaction has no NAME or MEMO".into(),
            ));
        }

        Ok(RawTransaction {
            date,
            amount,
            description: description.clone(),
            original_description: description,
            memo,
            category: None,
        })
    }

    /// Extract value for an OFX tag. Handles both SGML style `<TAG>value` and
    /// XML style `<TAG>value</TAG>`.
    fn extract_tag_value(block: &str, tag: &str) -> Option<String> {
        let upper_block = block.to_uppercase();
        let open_tag = format!("<{}>", tag.to_uppercase());
        let close_tag = format!("</{}>", tag.to_uppercase());

        let start = upper_block.find(&open_tag)?;
        let value_start = start + open_tag.len();

        // Find end: either closing tag, next opening tag, or newline
        let remaining = &block[value_start..];
        let upper_remaining = &upper_block[value_start..];

        let end = if let Some(pos) = upper_remaining.find(&close_tag) {
            pos
        } else if let Some(pos) = upper_remaining.find('<') {
            pos
        } else if let Some(pos) = remaining.find('\n') {
            pos
        } else {
            remaining.len()
        };

        let value = remaining[..end].trim().to_string();
        if value.is_empty() {
            None
        } else {
            Some(value)
        }
    }

    /// Parse OFX date format: YYYYMMDD or YYYYMMDDHHMMSS[.XXX:TZ]
    fn parse_ofx_date(s: &str) -> Result<NaiveDate> {
        let clean = s.trim();
        // Take just the date portion (first 8 chars)
        if clean.len() < 8 {
            return Err(IngestError::InvalidDate(format!(
                "OFX date too short: '{}'",
                clean
            )));
        }

        let date_part = &clean[..8];
        NaiveDate::parse_from_str(date_part, "%Y%m%d").map_err(|e| {
            IngestError::InvalidDate(format!("Cannot parse OFX date '{}': {}", clean, e))
        })
    }
}

impl Default for OfxParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FileParser for OfxParser {
    fn parse_preview(&self, data: &[u8]) -> Result<ParsePreview> {
        let txns = OfxParser::parse(data)?;
        let headers = vec![
            "Date".into(),
            "Amount".into(),
            "Description".into(),
            "Memo".into(),
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
                category_col: None,
            },
            row_count,
        })
    }

    fn parse_all(
        &self,
        data: &[u8],
        _mapping: Option<&ColumnMapping>,
    ) -> Result<Vec<RawTransaction>> {
        OfxParser::parse(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OFX_SGML: &str = r#"OFXHEADER:100
DATA:OFXSGML
VERSION:102
SECURITY:NONE
ENCODING:USASCII
CHARSET:1252
COMPRESSION:NONE
OLDFILEUID:NONE
NEWFILEUID:NONE

<OFX>
<SIGNONMSGSRSV1>
<SONRS>
<STATUS><CODE>0<SEVERITY>INFO</STATUS>
<DTSERVER>20240120120000
<LANGUAGE>ENG
</SONRS>
</SIGNONMSGSRSV1>
<BANKMSGSRSV1>
<STMTTRNRS>
<TRNUID>1001
<STATUS><CODE>0<SEVERITY>INFO</STATUS>
<STMTRS>
<CURDEF>USD
<BANKACCTFROM>
<BANKID>123456789
<ACCTID>9876543210
<ACCTTYPE>CHECKING
</BANKACCTFROM>
<BANKTRANLIST>
<DTSTART>20240101
<DTEND>20240131
<STMTTRN>
<TRNTYPE>DEBIT
<DTPOSTED>20240105
<TRNAMT>-45.99
<NAME>GROCERY STORE
<MEMO>Weekly groceries
</STMTTRN>
<STMTTRN>
<TRNTYPE>CREDIT
<DTPOSTED>20240110
<TRNAMT>2500.00
<NAME>PAYROLL DEPOSIT
<MEMO>January salary
</STMTTRN>
<STMTTRN>
<TRNTYPE>DEBIT
<DTPOSTED>20240112
<TRNAMT>-12.50
<NAME>COFFEE SHOP
</STMTTRN>
<STMTTRN>
<TRNTYPE>DEBIT
<DTPOSTED>20240115120000
<TRNAMT>-89.99
<NAME>ELECTRIC COMPANY
<MEMO>Monthly bill
</STMTTRN>
<STMTTRN>
<TRNTYPE>DEBIT
<DTPOSTED>20240120
<TRNAMT>-25.00
<NAME>STREAMING SERVICE
<MEMO>Monthly subscription
</STMTTRN>
</BANKTRANLIST>
<LEDGERBAL>
<BALAMT>5326.52
<DTASOF>20240131
</LEDGERBAL>
</STMTRS>
</STMTTRNRS>
</BANKMSGSRSV1>
</OFX>"#;

    const OFX_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<?OFX OFXHEADER="200" VERSION="220" SECURITY="NONE" OLDFILEUID="NONE" NEWFILEUID="NONE"?>
<OFX>
<SIGNONMSGSRSV1>
<SONRS>
<STATUS><CODE>0</CODE><SEVERITY>INFO</SEVERITY></STATUS>
<DTSERVER>20240120120000</DTSERVER>
<LANGUAGE>ENG</LANGUAGE>
</SONRS>
</SIGNONMSGSRSV1>
<BANKMSGSRSV1>
<STMTTRNRS>
<TRNUID>1001</TRNUID>
<STATUS><CODE>0</CODE><SEVERITY>INFO</SEVERITY></STATUS>
<STMTRS>
<CURDEF>USD</CURDEF>
<BANKTRANLIST>
<DTSTART>20240101</DTSTART>
<DTEND>20240131</DTEND>
<STMTTRN>
<TRNTYPE>DEBIT</TRNTYPE>
<DTPOSTED>20240105</DTPOSTED>
<TRNAMT>-45.99</TRNAMT>
<NAME>GROCERY STORE</NAME>
<MEMO>Weekly groceries</MEMO>
</STMTTRN>
<STMTTRN>
<TRNTYPE>CREDIT</TRNTYPE>
<DTPOSTED>20240110</DTPOSTED>
<TRNAMT>2500.00</TRNAMT>
<NAME>PAYROLL DEPOSIT</NAME>
<MEMO>January salary</MEMO>
</STMTTRN>
<STMTTRN>
<TRNTYPE>DEBIT</TRNTYPE>
<DTPOSTED>20240115</DTPOSTED>
<TRNAMT>-9.99</TRNAMT>
<NAME>SUBSCRIPTION SVC</NAME>
</STMTTRN>
</BANKTRANLIST>
</STMTRS>
</STMTTRNRS>
</BANKMSGSRSV1>
</OFX>"#;

    #[test]
    fn parse_ofx_sgml_transactions() {
        let txns = OfxParser::parse(OFX_SGML.as_bytes()).unwrap();
        assert_eq!(txns.len(), 5);
    }

    #[test]
    fn parse_ofx_sgml_first_transaction() {
        let txns = OfxParser::parse(OFX_SGML.as_bytes()).unwrap();
        assert_eq!(txns[0].date, NaiveDate::from_ymd_opt(2024, 1, 5).unwrap());
        assert_eq!(txns[0].amount, Decimal::from_str("-45.99").unwrap());
        assert_eq!(txns[0].description, "GROCERY STORE");
        assert_eq!(txns[0].memo.as_deref(), Some("Weekly groceries"));
    }

    #[test]
    fn parse_ofx_sgml_date_with_time() {
        let txns = OfxParser::parse(OFX_SGML.as_bytes()).unwrap();
        // Fourth transaction has DTPOSTED with time: 20240115120000
        assert_eq!(txns[3].date, NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
    }

    #[test]
    fn parse_ofx_xml_transactions() {
        let txns = OfxParser::parse(OFX_XML.as_bytes()).unwrap();
        assert_eq!(txns.len(), 3);
        assert_eq!(txns[0].description, "GROCERY STORE");
        assert_eq!(txns[1].amount, Decimal::from_str("2500.00").unwrap());
        assert_eq!(txns[2].description, "SUBSCRIPTION SVC");
    }

    #[test]
    fn parse_ofx_date_yyyymmdd() {
        let d = OfxParser::parse_ofx_date("20240315").unwrap();
        assert_eq!(d, NaiveDate::from_ymd_opt(2024, 3, 15).unwrap());
    }

    #[test]
    fn parse_ofx_date_with_timestamp() {
        let d = OfxParser::parse_ofx_date("20240315120000.000[-5:EST]").unwrap();
        assert_eq!(d, NaiveDate::from_ymd_opt(2024, 3, 15).unwrap());
    }

    #[test]
    fn parse_ofx_empty_data_error() {
        let result = OfxParser::parse(b"");
        assert!(result.is_err());
    }

    #[test]
    fn parse_ofx_no_transactions_error() {
        let result = OfxParser::parse(b"OFXHEADER:100\n<OFX></OFX>");
        assert!(result.is_err());
    }

    #[test]
    fn ofx_file_parser_preview() {
        let parser = OfxParser::new();
        let preview = parser.parse_preview(OFX_SGML.as_bytes()).unwrap();
        assert_eq!(preview.row_count, 5);
        assert_eq!(preview.rows.len(), 5);
        assert_eq!(
            preview.headers,
            vec!["Date", "Amount", "Description", "Memo"]
        );
    }
}
