//! Bank of Melbourne CSV exports.
//!
//! Format (see `tests/fixtures/bank_of_melbourne/README.md`): header
//! `Date,Description,Debit,Credit,Balance`, dates MM/DD/YYYY, debit = money
//! out, credit = money in, trailing `,,,,` rows.

use chrono::NaiveDate;

use super::{ParsedRow, file_error};
use crate::db::DbError;
use crate::money::Money;

const HEADER: [&str; 5] = ["date", "description", "debit", "credit", "balance"];

/// Parse a whole export. Any bad row fails the file, with its line number.
pub fn parse(text: &str) -> Result<Vec<ParsedRow>, DbError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());

    let mut rows = Vec::new();
    let mut saw_header = false;
    // Line numbers count records (BoM descriptions never span lines; csv's
    // own position is unreliable with CRLF endings).
    for (i, record) in reader.records().enumerate() {
        let line = i + 1;
        let record = record.map_err(|e| file_error(format!("Line {line}: {e}.")))?;
        let fields: Vec<&str> = record.iter().map(str::trim).collect();
        if fields.iter().all(|f| f.is_empty()) {
            continue;
        }
        if !saw_header {
            let ok = fields.len() == HEADER.len()
                && fields
                    .iter()
                    .zip(HEADER)
                    .all(|(f, h)| f.eq_ignore_ascii_case(h));
            if !ok {
                return Err(file_error(
                    "This doesn't look like a Bank of Melbourne export. Expected the columns \
                     Date, Description, Debit, Credit, Balance.",
                ));
            }
            saw_header = true;
            continue;
        }
        rows.push(row(line, &fields)?);
    }
    if !saw_header {
        return Err(file_error("The file is empty."));
    }
    if rows.is_empty() {
        return Err(file_error("No transactions found in this file."));
    }
    Ok(rows)
}

fn row(line: usize, f: &[&str]) -> Result<ParsedRow, DbError> {
    let err = |msg: String| file_error(format!("Line {line}: {msg}"));
    if f.len() != HEADER.len() {
        return Err(err(format!("expected 5 columns but found {}.", f.len())));
    }
    let date = NaiveDate::parse_from_str(f[0], "%m/%d/%Y")
        .map_err(|_| err(format!("\"{}\" isn't a date in MM/DD/YYYY form.", f[0])))?;
    let money = |s: &str, what: &str| -> Result<Option<Money>, DbError> {
        if s.is_empty() {
            return Ok(None);
        }
        Money::parse(s)
            .map(Some)
            .map_err(|_| err(format!("{what} \"{s}\" isn't an amount.")))
    };
    let debit = money(f[2], "debit")?;
    let credit = money(f[3], "credit")?;
    let amount = match (debit, credit) {
        (Some(d), None) => Money::from_cents(-d.cents().abs()),
        (None, Some(c)) => Money::from_cents(c.cents().abs()),
        (None, None) => return Err(err("no debit or credit amount.".into())),
        (Some(_), Some(_)) => return Err(err("has both a debit and a credit.".into())),
    };
    let description = f[1].split_whitespace().collect::<Vec<_>>().join(" ");
    if description.is_empty() {
        return Err(err("the description is empty.".into()));
    }
    Ok(ParsedRow {
        line,
        date,
        amount,
        payee: guess_payee(&description),
        description,
        balance: money(f[4], "balance")?,
    })
}

/// Card purchases look like `Eftpos Debit 20Nov15:37 Membership \ Melbourne Au`
/// (whitespace already collapsed): the merchant follows the card date/time
/// token and runs up to the backslash before the suburb.
pub fn guess_payee(description: &str) -> Option<String> {
    let tokens: Vec<&str> = description.split(' ').collect();
    let at = tokens.iter().position(|t| is_card_timestamp(t))?;
    let rest = tokens[at + 1..].join(" ");
    let merchant = rest.split('\\').next().unwrap_or("").trim();
    (!merchant.is_empty()).then(|| merchant.to_string())
}

/// `20Nov15:37` / `5Nov09:02`.
fn is_card_timestamp(t: &str) -> bool {
    let b = t.as_bytes();
    let digits = b.iter().take_while(|c| c.is_ascii_digit()).count();
    if !(1..=2).contains(&digits) || b.len() != digits + 8 {
        return false;
    }
    let r = &b[digits..];
    r[..3].iter().all(u8::is_ascii_alphabetic)
        && r[3..5].iter().all(u8::is_ascii_digit)
        && r[5] == b':'
        && r[6..8].iter().all(u8::is_ascii_digit)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str =
        include_str!("../../tests/fixtures/bank_of_melbourne/synthetic_export.csv");

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn msg(e: DbError) -> String {
        match e {
            DbError::Invalid {
                field: "file",
                message,
            } => message,
            other => panic!("expected a file error, got {other:?}"),
        }
    }

    #[test]
    fn parses_the_fixture() {
        let rows = parse(FIXTURE).unwrap();
        assert_eq!(rows.len(), 5, "trailing ,,,, rows are skipped");
        let r = &rows[0];
        assert_eq!((r.line, r.date), (2, d("2026-11-18")));
        assert_eq!(r.amount, Money::from_cents(-650));
        assert_eq!(r.balance, Some(Money::from_cents(241_275)));
        assert_eq!(
            r.description,
            "Eftpos Debit 18Nov09:12 Corner Bakery \\ Richmond Au"
        );
        assert_eq!(r.payee.as_deref(), Some("Corner Bakery"));
        let salary = &rows[1];
        assert_eq!(salary.amount, Money::from_cents(250_000));
        assert_eq!(salary.payee, None);
        assert_eq!(rows[2].balance, Some(Money::from_cents(-8_075)));
        assert_eq!(rows[4].amount, Money::from_cents(-123_456));
    }

    #[test]
    fn the_real_export_shape_parses() {
        // Same layout as the owner's sample, with a made-up amount/balance.
        let text = "Date,Description,Debit,Credit,Balance\r\n\
            11/20/2026,Eftpos Debit                  20Nov15:37  Gym Co    \\     Melbourne    Au,7.99,,100.00\r\n\
            ,,,,\r\n,,,,\r\n";
        let rows = parse(text).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].date, d("2026-11-20"));
        assert_eq!(rows[0].payee.as_deref(), Some("Gym Co"));
    }

    #[test]
    fn dates_are_month_first_and_padding_is_optional() {
        let text = "Date,Description,Debit,Credit,Balance\n3/4/2026,X,1.00,,\n";
        assert_eq!(parse(text).unwrap()[0].date, d("2026-03-04"));
        let bad = "Date,Description,Debit,Credit,Balance\n20/11/2026,X,1.00,,\n";
        assert!(msg(parse(bad).unwrap_err()).starts_with("Line 2: \"20/11/2026\""));
    }

    #[test]
    fn header_bom_case_and_quoted_commas() {
        let text = "\u{feff}DATE, description ,Debit,Credit,Balance\n\
                    11/02/2026,\"Smith, J  Osko payment\",,50.00,50.00\n";
        let r = &parse(text).unwrap()[0];
        assert_eq!(r.description, "Smith, J Osko payment");
        assert_eq!(r.amount, Money::from_cents(5_000));
    }

    #[test]
    fn friendly_file_errors() {
        assert!(msg(parse("").unwrap_err()).contains("empty"));
        assert!(
            msg(parse("Date,Amount,Description\n1/1/2026,5,x\n").unwrap_err())
                .contains("Bank of Melbourne")
        );
        let h = "Date,Description,Debit,Credit,Balance\n";
        assert!(msg(parse(&format!("{h},,,,\n")).unwrap_err()).contains("No transactions"));
        for (row, want) in [
            ("11/02/2026,X,,,1.00", "no debit or credit"),
            ("11/02/2026,X,1.00,2.00,1.00", "both"),
            ("11/02/2026,X,abc,,1.00", "debit \"abc\""),
            ("11/02/2026,   ,1.00,,1.00", "description is empty"),
            ("11/02/2026,X,1.00", "expected 5 columns"),
        ] {
            let m = msg(parse(&format!("{h}{row}\n")).unwrap_err());
            assert!(m.starts_with("Line 2: ") && m.contains(want), "{row}: {m}");
        }
    }

    #[test]
    fn payee_guessing() {
        assert_eq!(
            guess_payee("Eftpos Debit 5Nov09:02 Fresh Mart \\ Carlton Au").as_deref(),
            Some("Fresh Mart")
        );
        assert_eq!(
            guess_payee("Visa Purchase 20Nov15:37 Online Shop").as_deref(),
            Some("Online Shop")
        );
        assert_eq!(guess_payee("Direct Credit 123456 Acme Salary"), None);
        assert_eq!(guess_payee("Eftpos Debit 20Nov15:37 \\ Melbourne Au"), None);
        assert_eq!(guess_payee("Account Fee"), None);
    }
}
