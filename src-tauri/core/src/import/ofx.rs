//! OFX / QFX statement import.
//!
//! OFX comes in two flavours: the old SGML one (unclosed leaf tags, headers
//! before `<OFX>`) and the newer XML one (closed tags). Both carry the same
//! `<STMTTRN>` records, so one tolerant scanner handles both: read a tag,
//! then the text up to the next `<`. A closing tag therefore reads as an
//! empty value and is skipped.
//!
//! Dates are OFX's `YYYYMMDDHHMMSS[+tz:name]`; only the first eight digits
//! matter. Balances are ledger-level, not per row, so rows carry no balance
//! and the preview relies on dates for ordering.

use chrono::NaiveDate;

use super::{ParsedRow, file_error};
use crate::db::DbError;
use crate::money::Money;

/// Does this text look like an OFX/QFX statement?
pub fn looks_like(text: &str) -> bool {
    let head = text.trim_start_matches('\u{feff}');
    head.contains("OFXHEADER") || head.contains("<OFX>") || head.contains("<STMTTRN>")
}

/// Parse a whole statement. Any bad transaction fails the file, with its line.
pub fn parse(text: &str) -> Result<Vec<ParsedRow>, DbError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if !looks_like(text) {
        return Err(file_error(
            "This doesn't look like an OFX or QFX statement.",
        ));
    }

    let mut rows = Vec::new();
    let mut fields: Vec<(String, String, usize)> = Vec::new();
    let mut in_transaction = false;
    let mut line = 1usize;

    for (tag, value, at_line) in leaves(text) {
        match tag.as_str() {
            "STMTTRN" => {
                if in_transaction {
                    rows.push(transaction(&fields, at_line)?);
                }
                fields.clear();
                in_transaction = true;
                line = at_line;
            }
            "CURDEF" => {
                let currency = value.trim().to_uppercase();
                if !currency.is_empty() && currency != "AUD" {
                    return Err(file_error(format!(
                        "This statement is in {currency}; Plutus only handles AUD."
                    )));
                }
            }
            _ if in_transaction => fields.push((tag, value, at_line)),
            _ => {}
        }
    }
    if in_transaction {
        rows.push(transaction(&fields, line)?);
    }
    if rows.is_empty() {
        return Err(file_error("No transactions found in this statement."));
    }
    Ok(rows)
}

/// `(tag, text, line)` for every leaf tag, in file order. Closing tags come
/// back with empty text and are ignored by the caller.
fn leaves(text: &str) -> Vec<(String, String, usize)> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    let mut line = 1usize;
    while let Some(open) = text[pos..].find('<') {
        let open = pos + open;
        line += text[pos..open].matches('\n').count();
        let Some(close) = text[open..].find('>') else {
            break;
        };
        let close = open + close;
        let tag = text[open + 1..close].trim().to_string();
        let value_start = close + 1;
        let value_end = text[value_start..]
            .find('<')
            .map(|i| value_start + i)
            .unwrap_or(text.len());
        let value = text[value_start..value_end].trim().to_string();
        if !tag.is_empty() && !tag.starts_with('/') && !tag.starts_with('?') {
            out.push((tag, value, line));
        }
        pos = value_end;
    }
    out
}

fn transaction(fields: &[(String, String, usize)], line: usize) -> Result<ParsedRow, DbError> {
    let get = |name: &str| {
        fields
            .iter()
            .find(|(t, _, _)| t.eq_ignore_ascii_case(name))
            .map(|(_, v, _)| v.as_str())
            .unwrap_or("")
    };
    let err = |msg: String| file_error(format!("Line {line}: {msg}"));

    let raw_date = get("DTPOSTED");
    let date = parse_date(raw_date)
        .ok_or_else(|| err(format!("\"{raw_date}\" isn't a date in YYYYMMDD form.")))?;

    let raw_amount = get("TRNAMT");
    if raw_amount.is_empty() {
        return Err(err("transaction has no amount.".into()));
    }
    let amount = Money::parse(raw_amount)
        .map_err(|_| err(format!("amount \"{raw_amount}\" isn't a number.")))?;

    let name = get("NAME");
    let payee = get("PAYEE");
    let memo = get("MEMO");
    let description = [name, payee, memo]
        .iter()
        .map(|s| s.trim())
        .find(|s| !s.is_empty())
        .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_default();
    if description.is_empty() {
        return Err(err("transaction has no description.".into()));
    }
    let payee = [name, payee]
        .iter()
        .map(|s| s.trim())
        .find(|s| !s.is_empty())
        .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "));

    Ok(ParsedRow {
        line,
        date,
        amount,
        description,
        payee,
        balance: None,
    })
}

/// `20260115`, `20260115120000`, `20260115120000[+11:EST]`, `2026-01-15`.
fn parse_date(raw: &str) -> Option<NaiveDate> {
    let raw = raw.trim();
    let digits: String = raw.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 8 {
        return NaiveDate::parse_from_str(&digits[..8], "%Y%m%d").ok();
    }
    NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SGML: &str = "OFXHEADER:100\r\nDATA:OFXSGML\r\nVERSION:102\r\n\r\n\
<OFX>\r\n<BANKMSGSRSV1>\r\n<STMTTRNRS>\r\n<STMTRS>\r\n<CURDEF>AUD\r\n\
<BANKTRANLIST>\r\n<DTSTART>20260101\r\n<DTEND>20260131\r\n\
<STMTTRN>\r\n<TRNTYPE>DEBIT\r\n<DTPOSTED>20260115120000[+11:EST]\r\n\
<TRNAMT>-45.90\r\n<FITID>202601150001\r\n<NAME>WOOLWORTHS   MARKET\r\n\
<MEMO>Card purchase\r\n</STMTTRN>\r\n\
<STMTTRN>\r\n<TRNTYPE>CREDIT\r\n<DTPOSTED>20260116\r\n<TRNAMT>2500.00\r\n\
<FITID>202601160001\r\n<NAME>SALARY ACME PTY LTD\r\n</STMTTRN>\r\n\
</BANKTRANLIST>\r\n<LEDGERBAL><BALAMT>2454.10<DTASOF>20260131\r\n</LEDGERBAL>\r\n\
</STMTRS>\r\n</STMTTRNRS>\r\n</BANKMSGSRSV1>\r\n</OFX>\r\n";

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<OFX><BANKMSGSRSV1><STMTTRNRS><STMTRS><CURDEF>AUD</CURDEF>
<BANKTRANLIST>
<STMTTRN><TRNTYPE>DEBIT</TRNTYPE><DTPOSTED>20260120</DTPOSTED>
<TRNAMT>-12.50</TRNAMT><FITID>9</FITID><NAME>COFFEE SHOP</NAME>
<MEMO>Flat white</MEMO></STMTTRN>
</BANKTRANLIST></STMTRS></STMTTRNRS></BANKMSGSRSV1></OFX>"#;

    #[test]
    fn parses_sgml_statements() {
        let rows = parse(SGML).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].date, NaiveDate::from_ymd_opt(2026, 1, 15).unwrap());
        assert_eq!(rows[0].amount.cents(), -4590);
        assert_eq!(rows[0].description, "WOOLWORTHS MARKET");
        assert_eq!(rows[0].payee.as_deref(), Some("WOOLWORTHS MARKET"));
        assert_eq!(rows[1].amount.cents(), 250_000);
        assert_eq!(rows[1].payee.as_deref(), Some("SALARY ACME PTY LTD"));
    }

    #[test]
    fn parses_xml_statements() {
        let rows = parse(XML).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].amount.cents(), -1250);
        assert_eq!(rows[0].description, "COFFEE SHOP");
    }

    #[test]
    fn rejects_other_currencies() {
        let text = SGML.replace("<CURDEF>AUD", "<CURDEF>USD");
        let err = parse(&text).unwrap_err();
        assert!(err.to_string().contains("USD"), "{err}");
    }

    #[test]
    fn rejects_missing_amount_and_bad_date() {
        let no_amount = SGML.replace("<TRNAMT>-45.90\r\n", "");
        assert!(parse(&no_amount).is_err());
        let bad_date = SGML.replace("<DTPOSTED>20260116", "<DTPOSTED>not-a-date");
        assert!(parse(&bad_date).is_err());
    }

    #[test]
    fn rejects_files_that_are_not_ofx() {
        assert!(!looks_like("Date,Description,Debit,Credit,Balance\n"));
        assert!(parse("hello world").is_err());
    }

    #[test]
    fn date_forms() {
        let d = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        assert_eq!(parse_date("20260115"), Some(d));
        assert_eq!(parse_date("20260115120000[+11:EST]"), Some(d));
        assert_eq!(parse_date("2026-01-15"), Some(d));
        assert_eq!(parse_date("nope"), None);
    }
}
