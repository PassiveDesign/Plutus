//! Generic CSV imports: the owner maps the file's columns to Plutus's fields.
//!
//! Bank of Melbourne gets its own parser ([`super::bom`]); this module covers
//! every other bank by letting the owner say which column is which. A mapping
//! is saved as a preset so the same bank only has to be mapped once.

use std::collections::HashMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::bom::guess_payee;
use super::{ParsedRow, file_error};
use crate::db::DbError;
use crate::money::Money;

/// How the file writes dates. Two-digit years and other formats are not
/// supported on purpose: guessing the wrong order silently corrupts history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateFormat {
    /// `11/20/2026` — month first.
    Mdy,
    /// `20/11/2026` — day first.
    Dmy,
    /// `2026-11-20`.
    Ymd,
}

impl DateFormat {
    fn pattern(self) -> &'static str {
        match self {
            DateFormat::Mdy => "%m/%d/%Y",
            DateFormat::Dmy => "%d/%m/%Y",
            DateFormat::Ymd => "%Y-%m-%d",
        }
    }

    /// An example date, for the UI to show next to the choice.
    pub fn example(self) -> &'static str {
        match self {
            DateFormat::Mdy => "11/20/2026",
            DateFormat::Dmy => "20/11/2026",
            DateFormat::Ymd => "2026-11-20",
        }
    }
}

/// Which columns hold what. Names are matched against the header row,
/// ignoring case and surrounding spaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mapping {
    pub date: String,
    #[serde(default = "default_format")]
    pub date_format: DateFormat,
    pub description: String,
    /// Optional merchant column; when empty the description is used to guess.
    #[serde(default)]
    pub payee: Option<String>,
    /// One signed amount column (`-12.50` out, `12.50` in).
    #[serde(default)]
    pub amount: Option<String>,
    /// …or two columns: money out and money in.
    #[serde(default)]
    pub debit: Option<String>,
    #[serde(default)]
    pub credit: Option<String>,
    #[serde(default)]
    pub balance: Option<String>,
}

fn default_format() -> DateFormat {
    DateFormat::Mdy
}

impl Mapping {
    /// True when amounts come from a debit/credit pair rather than one column.
    pub fn uses_debit_credit(&self) -> bool {
        self.debit.is_some() || self.credit.is_some()
    }
}

/// The header row of a CSV file, trimmed and BOM-stripped.
pub fn headers(text: &str) -> Result<Vec<String>, DbError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut records = reader.records();
    let Some(first) = records.next() else {
        return Err(file_error("The file is empty."));
    };
    let first = first.map_err(|e| file_error(format!("Couldn't read the header row: {e}.")))?;
    let names: Vec<String> = first.iter().map(|f| f.trim().to_string()).collect();
    if names.iter().all(|n| n.is_empty()) {
        return Err(file_error(
            "The first row is empty; there's no header to map.",
        ));
    }
    Ok(names)
}

/// Check a mapping is complete enough to parse with. Used when saving a
/// preset, where there is no file to test against.
pub fn validate(mapping: &Mapping) -> Result<(), DbError> {
    if mapping.date.trim().is_empty() {
        return Err(DbError::Invalid {
            field: "mapping",
            message: "Choose the date column.".to_string(),
        });
    }
    if mapping.description.trim().is_empty() {
        return Err(DbError::Invalid {
            field: "mapping",
            message: "Choose the description column.".to_string(),
        });
    }
    if mapping.amount.is_none() && mapping.debit.is_none() && mapping.credit.is_none() {
        return Err(DbError::Invalid {
            field: "mapping",
            message: "Choose an amount column, or a debit and credit pair.".to_string(),
        });
    }
    Ok(())
}

/// The headers and a first guess at the mapping, for the mapping form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileShape {
    pub headers: Vec<String>,
    pub mapping: Mapping,
}

pub fn shape(text: &str) -> Result<FileShape, DbError> {
    let headers = headers(text)?;
    let mapping = guess_mapping(&headers);
    Ok(FileShape { headers, mapping })
}

/// A first guess at the mapping, from the header names. The owner confirms or
/// corrects it in the UI.
pub fn guess_mapping(headers: &[String]) -> Mapping {
    let find = |wanted: &[&str]| -> Option<String> {
        headers
            .iter()
            .find(|h| {
                let lower = h.to_lowercase();
                wanted.iter().any(|w| lower == *w)
            })
            .cloned()
    };
    let find_any = |wanted: &[&str]| -> Option<String> {
        headers
            .iter()
            .find(|h| {
                let lower = h.to_lowercase();
                wanted.iter().any(|w| lower.contains(w))
            })
            .cloned()
    };
    let date = find(&["date", "transaction date", "posted date", "value date"])
        .or_else(|| find_any(&["date"]))
        .unwrap_or_default();
    let description = find(&["description", "details", "narrative", "memo", "transaction"])
        .or_else(|| find_any(&["description", "detail", "narrative"]))
        .unwrap_or_default();
    let amount = find(&["amount", "value", "transaction amount"]);
    let debit = find(&["debit", "withdrawal", "money out", "debit amount"]);
    let credit = find(&["credit", "deposit", "money in", "credit amount"]);
    let balance = find(&["balance", "running balance", "closing balance"]);
    let payee = find(&["payee", "merchant", "name", "cardholder"]);
    Mapping {
        date,
        date_format: DateFormat::Mdy,
        description,
        payee,
        amount: if debit.is_some() || credit.is_some() {
            None
        } else {
            amount
        },
        debit,
        credit,
        balance,
    }
}

/// Parse a whole file with the given mapping. Any bad row fails the file, with
/// its line number, so nothing is imported half-right.
pub fn parse(text: &str, mapping: &Mapping) -> Result<Vec<ParsedRow>, DbError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());

    let mut records = reader.records();
    let Some(header) = records.next() else {
        return Err(file_error("The file is empty."));
    };
    let header = header.map_err(|e| file_error(format!("Couldn't read the header row: {e}.")))?;
    let index: HashMap<String, usize> = header
        .iter()
        .enumerate()
        .map(|(i, name)| (name.trim().to_lowercase(), i))
        .collect();
    let width = header.len();

    // Resolve every column once; a missing name is a mapping mistake, not a
    // file mistake, so the message names the column.
    let column = |name: &str, what: &str| -> Result<usize, DbError> {
        let key = name.trim().to_lowercase();
        index.get(&key).copied().ok_or_else(|| DbError::Invalid {
            field: "mapping",
            message: format!("The {what} column \"{name}\" isn't in this file."),
        })
    };
    let date_col = column(&mapping.date, "date")?;
    let desc_col = column(&mapping.description, "description")?;
    let payee_col = mapping
        .payee
        .as_deref()
        .filter(|p| !p.trim().is_empty())
        .map(|p| column(p, "payee"))
        .transpose()?;
    let amount_col = mapping
        .amount
        .as_deref()
        .filter(|a| !a.trim().is_empty())
        .map(|a| column(a, "amount"))
        .transpose()?;
    let debit_col = mapping
        .debit
        .as_deref()
        .filter(|d| !d.trim().is_empty())
        .map(|d| column(d, "debit"))
        .transpose()?;
    let credit_col = mapping
        .credit
        .as_deref()
        .filter(|c| !c.trim().is_empty())
        .map(|c| column(c, "credit"))
        .transpose()?;
    let balance_col = mapping
        .balance
        .as_deref()
        .filter(|b| !b.trim().is_empty())
        .map(|b| column(b, "balance"))
        .transpose()?;

    if amount_col.is_none() && debit_col.is_none() && credit_col.is_none() {
        return Err(DbError::Invalid {
            field: "mapping",
            message: "Choose an amount column, or a debit and credit column.".to_string(),
        });
    }

    let mut rows = Vec::new();
    for (i, record) in records.enumerate() {
        // Line 1 is the header, so the first data row is line 2.
        let line = i + 2;
        let record = record.map_err(|e| file_error(format!("Line {line}: {e}.")))?;
        let fields: Vec<&str> = record.iter().map(str::trim).collect();
        if fields.iter().all(|f| f.is_empty()) {
            continue;
        }
        if fields.len() < width {
            // Short rows are allowed (trailing empty columns are dropped by
            // some exports); a missing column is treated as empty.
        }
        let at = |i: usize| -> &str { fields.get(i).copied().unwrap_or("") };

        let err = |msg: String| file_error(format!("Line {line}: {msg}"));
        let date_text = at(date_col);
        if date_text.is_empty() {
            return Err(err("the date is empty.".into()));
        }
        let date =
            NaiveDate::parse_from_str(date_text, mapping.date_format.pattern()).map_err(|_| {
                err(format!(
                    "\"{date_text}\" isn't a date in the chosen format."
                ))
            })?;

        let amount = match (amount_col, debit_col, credit_col) {
            (Some(a), _, _) => {
                let text = at(a);
                if text.is_empty() {
                    return Err(err("the amount is empty.".into()));
                }
                Money::parse(text).map_err(|_| err(format!("\"{text}\" isn't an amount.")))?
            }
            (None, debit, credit) => {
                let debit = debit.map(at).unwrap_or("");
                let credit = credit.map(at).unwrap_or("");
                match (debit.is_empty(), credit.is_empty()) {
                    (true, true) => return Err(err("no debit or credit amount.".into())),
                    (false, false) => {
                        return Err(err("both a debit and a credit amount; expected one.".into()));
                    }
                    (false, true) => Money::parse(debit)
                        .map(|m| Money::from_cents(-m.cents().abs()))
                        .map_err(|_| err(format!("\"{debit}\" isn't an amount.")))?,
                    (true, false) => Money::parse(credit)
                        .map(|m| Money::from_cents(m.cents().abs()))
                        .map_err(|_| err(format!("\"{credit}\" isn't an amount.")))?,
                }
            }
        };

        let description = at(desc_col)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let payee = payee_col
            .map(|c| at(c).trim().to_string())
            .filter(|p| !p.is_empty())
            .or_else(|| guess_payee(&description));
        if description.is_empty() && payee.is_none() {
            return Err(err("the description is empty.".into()));
        }

        let balance = match balance_col {
            Some(c) => {
                let text = at(c);
                if text.is_empty() {
                    None
                } else {
                    Some(
                        Money::parse(text)
                            .map_err(|_| err(format!("\"{text}\" isn't a balance.")))?,
                    )
                }
            }
            None => None,
        };

        rows.push(ParsedRow {
            line,
            date,
            amount,
            description,
            payee,
            balance,
        });
    }

    if rows.is_empty() {
        return Err(file_error("No transactions found in this file."));
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE: &str = "\u{feff}Date,Description,Amount,Balance\n\
        11/20/2026,WOOLWORTHS 1234,-79.90,1660.99\n\
        11/21/2026,SALARY ACME,2500.00,4160.99\n";

    const TWO_COLUMNS: &str = "Date,Details,Withdrawal,Deposit,Balance\n\
        20/11/2026,COLES 99,45.10,,1615.89\n\
        21/11/2026,PAYROLL,,2000.00,3615.89\n";

    #[test]
    fn reads_headers_and_guesses_the_mapping() {
        let h = headers(SIMPLE).unwrap();
        assert_eq!(h, ["Date", "Description", "Amount", "Balance"]);
        let m = guess_mapping(&h);
        assert_eq!(m.date, "Date");
        assert_eq!(m.description, "Description");
        assert_eq!(m.amount.as_deref(), Some("Amount"));
        assert_eq!(m.balance.as_deref(), Some("Balance"));
        assert!(!m.uses_debit_credit());
    }

    #[test]
    fn guesses_debit_and_credit_when_both_are_present() {
        let h = headers(TWO_COLUMNS).unwrap();
        let m = guess_mapping(&h);
        assert_eq!(m.debit.as_deref(), Some("Withdrawal"));
        assert_eq!(m.credit.as_deref(), Some("Deposit"));
        assert_eq!(m.amount, None);
        assert!(m.uses_debit_credit());
    }

    #[test]
    fn parses_a_signed_amount_column() {
        let m = Mapping {
            date: "Date".into(),
            date_format: DateFormat::Mdy,
            description: "Description".into(),
            payee: None,
            amount: Some("Amount".into()),
            debit: None,
            credit: None,
            balance: Some("Balance".into()),
        };
        let rows = parse(SIMPLE, &m).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].line, 2);
        assert_eq!(rows[0].date, NaiveDate::from_ymd_opt(2026, 11, 20).unwrap());
        assert_eq!(rows[0].amount.cents(), -7990);
        assert_eq!(rows[0].balance.unwrap().cents(), 166_099);
        assert_eq!(rows[1].amount.cents(), 250_000);
        // No card timestamp in this description, so there is no guessed payee.
        assert_eq!(rows[0].payee, None);
    }

    #[test]
    fn uses_an_explicit_payee_column() {
        let text = "Date,Description,Merchant,Amount\n\
            11/20/2026,EFTPOS 1234,WOOLWORTHS,-79.90\n";
        let m = Mapping {
            date: "Date".into(),
            date_format: DateFormat::Mdy,
            description: "Description".into(),
            payee: Some("Merchant".into()),
            amount: Some("Amount".into()),
            debit: None,
            credit: None,
            balance: None,
        };
        let rows = parse(text, &m).unwrap();
        assert_eq!(rows[0].payee.as_deref(), Some("WOOLWORTHS"));
        assert_eq!(rows[0].description, "EFTPOS 1234");
    }

    #[test]
    fn parses_debit_and_credit_columns() {
        let m = Mapping {
            date: "Date".into(),
            date_format: DateFormat::Dmy,
            description: "Details".into(),
            payee: None,
            amount: None,
            debit: Some("Withdrawal".into()),
            credit: Some("Deposit".into()),
            balance: Some("Balance".into()),
        };
        let rows = parse(TWO_COLUMNS, &m).unwrap();
        assert_eq!(rows[0].amount.cents(), -4510);
        assert_eq!(rows[1].amount.cents(), 200_000);
        assert_eq!(rows[0].date, NaiveDate::from_ymd_opt(2026, 11, 20).unwrap());
    }

    #[test]
    fn skips_blank_rows() {
        let text = "Date,Description,Amount\n\
            11/20/2026,COFFEE,-4.50\n\
            ,,\n\
            11/21/2026,LUNCH,-12.00\n";
        let m = Mapping {
            date: "Date".into(),
            date_format: DateFormat::Mdy,
            description: "Description".into(),
            payee: None,
            amount: Some("Amount".into()),
            debit: None,
            credit: None,
            balance: None,
        };
        let rows = parse(text, &m).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].line, 2);
        assert_eq!(rows[1].line, 4);
    }

    #[test]
    fn reports_bad_rows_with_their_line() {
        let text = "Date,Description,Amount\n\
            11/20/2026,COFFEE,not-a-number\n";
        let m = Mapping {
            date: "Date".into(),
            date_format: DateFormat::Mdy,
            description: "Description".into(),
            payee: None,
            amount: Some("Amount".into()),
            debit: None,
            credit: None,
            balance: None,
        };
        let err = parse(text, &m).unwrap_err().to_string();
        assert!(err.contains("Line 2"), "{err}");
        assert!(err.contains("isn't an amount"), "{err}");
    }

    #[test]
    fn reports_a_missing_column_as_a_mapping_problem() {
        let m = Mapping {
            date: "When".into(),
            date_format: DateFormat::Mdy,
            description: "Description".into(),
            payee: None,
            amount: Some("Amount".into()),
            debit: None,
            credit: None,
            balance: None,
        };
        let err = parse(SIMPLE, &m).unwrap_err();
        assert_eq!(err.code(), "invalid");
        assert!(err.to_string().contains("When"), "{err}");
    }

    #[test]
    fn needs_some_amount_column() {
        let m = Mapping {
            date: "Date".into(),
            date_format: DateFormat::Mdy,
            description: "Description".into(),
            payee: None,
            amount: None,
            debit: None,
            credit: None,
            balance: None,
        };
        let err = parse(SIMPLE, &m).unwrap_err();
        assert!(err.to_string().contains("amount column"), "{err}");
    }
}
