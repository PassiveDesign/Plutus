//! QIF (Quicken Interchange Format) import.
//!
//! A QIF file is a list of records separated by `^`, one field per line where
//! the first character names the field: `D` date, `T` amount, `P` payee,
//! `M` memo, `N` number, `C` cleared, `L` category. `!Type:Bank` and similar
//! header lines are ignored.
//!
//! QIF dates are written day/month in whatever order the exporting program
//! used, with no way to tell — so the owner picks the format explicitly,
//! exactly as for generic CSV. Splits (`S`/`$` lines) are ignored: the total
//! `T` amount is what the bank actually moved.

use chrono::NaiveDate;

use super::generic::DateFormat;
use super::{ParsedRow, file_error};
use crate::db::DbError;
use crate::money::Money;

/// Does this text look like a QIF file?
pub fn looks_like(text: &str) -> bool {
    text.trim_start_matches('\u{feff}')
        .lines()
        .take(20)
        .any(|l| l.trim_start().starts_with("!Type:"))
}

/// Parse a whole file. Any bad record fails the file, with its line number.
pub fn parse(text: &str, format: DateFormat) -> Result<Vec<ParsedRow>, DbError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if !looks_like(text) {
        return Err(file_error("This doesn't look like a QIF file."));
    }

    let mut rows = Vec::new();
    let mut record: Vec<(char, String, usize)> = Vec::new();
    let mut start_line = 1usize;
    let mut saw_any = false;

    for (i, raw) in text.lines().enumerate() {
        let line = i + 1;
        let trimmed = raw.trim_end_matches('\r');
        if trimmed.trim().is_empty() {
            continue;
        }
        if trimmed.starts_with('!') {
            // `!Type:Bank` starts a section; `!Account` etc. are headers.
            continue;
        }
        if trimmed.trim() == "^" {
            if !record.is_empty() {
                // `!Account` blocks end in `^` too, but carry no date; only
                // records with a date are transactions.
                if has_date(&record) {
                    rows.push(transaction(&record, start_line, format)?);
                    saw_any = true;
                }
                record.clear();
            }
            continue;
        }
        let mut chars = trimmed.chars();
        let Some(code) = chars.next() else { continue };
        if record.is_empty() {
            start_line = line;
        }
        record.push((code, chars.as_str().trim().to_string(), line));
    }
    if !record.is_empty() && has_date(&record) {
        rows.push(transaction(&record, start_line, format)?);
        saw_any = true;
    }
    if !saw_any {
        return Err(file_error("No transactions found in this file."));
    }
    Ok(rows)
}

fn has_date(fields: &[(char, String, usize)]) -> bool {
    fields.iter().any(|(c, v, _)| *c == 'D' && !v.is_empty())
}

fn transaction(
    fields: &[(char, String, usize)],
    line: usize,
    format: DateFormat,
) -> Result<ParsedRow, DbError> {
    let get = |code: char| {
        fields
            .iter()
            .find(|(c, _, _)| *c == code)
            .map(|(_, v, _)| v.as_str())
            .unwrap_or("")
    };
    let err = |msg: String| file_error(format!("Line {line}: {msg}"));

    let raw_date = get('D');
    let date = parse_date(raw_date, format).ok_or_else(|| {
        err(format!(
            "\"{raw_date}\" isn't a date in {} form.",
            format.example()
        ))
    })?;

    let raw_amount = get('T');
    if raw_amount.is_empty() {
        return Err(err("record has no amount.".into()));
    }
    let amount = Money::parse(raw_amount)
        .map_err(|_| err(format!("amount \"{raw_amount}\" isn't a number.")))?;

    let payee = get('P').trim();
    let memo = get('M').trim();
    let description = [payee, memo]
        .iter()
        .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
        .find(|s| !s.is_empty())
        .unwrap_or_default();
    if description.is_empty() {
        return Err(err("record has no payee or memo.".into()));
    }

    Ok(ParsedRow {
        line,
        date,
        amount,
        description,
        payee: (!payee.is_empty()).then(|| payee.split_whitespace().collect::<Vec<_>>().join(" ")),
        balance: None,
    })
}

/// `1/15/2026`, `15/01/2026`, `2026-01-15`, `1/15'26`, `15.01.2026`.
fn parse_date(raw: &str, format: DateFormat) -> Option<NaiveDate> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    // Two-digit years: `1/15'26` means 2026.
    let normalised = match raw.split_once('\'') {
        Some((left, year)) if year.len() == 2 => format!("{left}/{}{year}", century(year)),
        _ => raw.to_string(),
    };
    let normalised = normalised.replace('.', "/");
    match format {
        DateFormat::Ymd => NaiveDate::parse_from_str(&normalised, "%Y-%m-%d")
            .or_else(|_| NaiveDate::parse_from_str(&normalised, "%Y/%m/%d"))
            .ok(),
        DateFormat::Mdy => NaiveDate::parse_from_str(&normalised, "%m/%d/%Y").ok(),
        DateFormat::Dmy => NaiveDate::parse_from_str(&normalised, "%d/%m/%Y").ok(),
    }
}

/// `26` → `2026`, `99` → `1999` (QIF files don't reach back further in practice).
fn century(year: &str) -> &'static str {
    match year.parse::<u32>() {
        Ok(y) if y >= 70 => "19",
        _ => "20",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BANK: &str = "!Type:Bank\r\n\
D1/15/2026\r\nT-45.90\r\nPWOOLWORTHS  MARKET\r\nMCard purchase\r\nN1234\r\n^\r\n\
D1/16/2026\r\nT2500.00\r\nPSALARY ACME PTY LTD\r\n^\r\n";

    #[test]
    fn parses_mdy_records() {
        let rows = parse(BANK, DateFormat::Mdy).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].date, NaiveDate::from_ymd_opt(2026, 1, 15).unwrap());
        assert_eq!(rows[0].amount.cents(), -4590);
        assert_eq!(rows[0].description, "WOOLWORTHS MARKET");
        assert_eq!(rows[0].payee.as_deref(), Some("WOOLWORTHS MARKET"));
        assert_eq!(rows[1].amount.cents(), 250_000);
    }

    #[test]
    fn dmy_reading_differs() {
        let text = "!Type:Bank\nD15/01/2026\nT-4.50\nPCoffee\n^\n";
        let rows = parse(text, DateFormat::Dmy).unwrap();
        assert_eq!(rows[0].date, NaiveDate::from_ymd_opt(2026, 1, 15).unwrap());
        // The same text read as month-first is a different (invalid) date.
        assert!(parse(text, DateFormat::Mdy).is_err());
    }

    #[test]
    fn two_digit_years_and_dots() {
        let text = "!Type:Bank\nD1/15'26\nT-4.50\nPCoffee\n^\n";
        let rows = parse(text, DateFormat::Mdy).unwrap();
        assert_eq!(rows[0].date, NaiveDate::from_ymd_opt(2026, 1, 15).unwrap());
        let dotted = "!Type:Bank\nD15.01.2026\nT-4.50\nPCoffee\n^\n";
        let rows = parse(dotted, DateFormat::Dmy).unwrap();
        assert_eq!(rows[0].date, NaiveDate::from_ymd_opt(2026, 1, 15).unwrap());
    }

    #[test]
    fn rejects_missing_amount_or_payee() {
        let no_amount = "!Type:Bank\nD1/15/2026\nPCoffee\n^\n";
        assert!(parse(no_amount, DateFormat::Mdy).is_err());
        let no_payee = "!Type:Bank\nD1/15/2026\nT-4.50\n^\n";
        assert!(parse(no_payee, DateFormat::Mdy).is_err());
    }

    #[test]
    fn rejects_files_that_are_not_qif() {
        assert!(!looks_like("Date,Description\n"));
        assert!(parse("hello", DateFormat::Mdy).is_err());
    }

    #[test]
    fn ignores_headers_and_split_lines() {
        let text = "!Type:Bank\n!Account\nNEveryday\n^\n\
D1/15/2026\nT-10.00\nPSupermarket\nSFood\n$-7.00\nSHousehold\n$-3.00\n^\n";
        let rows = parse(text, DateFormat::Mdy).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].amount.cents(), -1000);
    }
}
