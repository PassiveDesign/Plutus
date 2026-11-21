//! Importing bank exports: parse → preview (new / already imported /
//! possible match) → commit.
//!
//! Each parsed row gets an `import_key` from the bank's own data (date,
//! amount, normalised description, occurrence number among identical rows
//! in the same file). Re-importing an overlapping export yields the same
//! keys, so already-imported rows are skipped. Rows that match a
//! hand-entered transaction (same date and amount, no key) are flagged so
//! the owner can decide.

pub mod bom;

use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::accounts;
use crate::db::{Database, DbError};
use crate::money::Money;
use crate::transactions::{self, TransactionInput};

/// One transaction read from a bank file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedRow {
    /// 1-based line in the file, for messages.
    pub line: usize,
    pub date: NaiveDate,
    /// Signed: money in positive.
    pub amount: Money,
    /// The bank's text, whitespace collapsed.
    pub description: String,
    /// Best guess at the merchant, if the description has one.
    pub payee: Option<String>,
    /// Balance after this row, as the bank reports it.
    pub balance: Option<Money>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowStatus {
    /// Not in Plutus yet; imported by default.
    New,
    /// Imported before (same key); always skipped.
    Duplicate,
    /// Same date and amount as a hand-entered transaction; skipped unless
    /// the owner ticks it.
    PossibleMatch,
    /// Dated before the account's opening date, so already counted in the
    /// opening balance; skipped unless ticked.
    BeforeOpening,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportRow {
    #[serde(flatten)]
    pub row: ParsedRow,
    pub import_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreviewRow {
    #[serde(flatten)]
    pub row: ImportRow,
    pub status: RowStatus,
    /// For `PossibleMatch`: the hand-entered transaction it resembles.
    pub matched_transaction_id: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileOrder {
    NewestFirst,
    OldestFirst,
    /// One row, or the balances/dates don't say.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Preview {
    pub account_id: i64,
    /// In file order.
    pub rows: Vec<PreviewRow>,
    pub order: FileOrder,
    /// Lines whose balance doesn't follow from the previous row's (missing
    /// rows in the export, or an edited file). Empty when consistent.
    pub balance_breaks: Vec<usize>,
    /// The bank's balance after the newest row, if the file has balances.
    pub bank_balance: Option<Money>,
    /// Plutus's balance for the account now.
    pub current_balance: Money,
    /// Plutus's balance if the `New` rows are imported.
    pub balance_after: Money,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CommitResult {
    pub imported: usize,
    /// Rows already present by the time of commit.
    pub skipped: usize,
}

pub(crate) fn file_error(message: impl Into<String>) -> DbError {
    DbError::Invalid {
        field: "file",
        message: message.into(),
    }
}

/// Stable de-duplication keys, in row order.
pub fn keys(rows: &[ParsedRow]) -> Vec<String> {
    let mut seen: HashMap<(NaiveDate, i64, String), usize> = HashMap::new();
    rows.iter()
        .map(|r| {
            let desc = r.description.to_lowercase();
            let n = seen
                .entry((r.date, r.amount.cents(), desc.clone()))
                .or_insert(0);
            *n += 1;
            format!("{}|{}|{}|{}", r.date, r.amount.cents(), desc, n)
        })
        .collect()
}

fn order_and_breaks(rows: &[ParsedRow]) -> (FileOrder, Vec<usize>) {
    if rows.len() < 2 {
        return (FileOrder::Unknown, Vec::new());
    }
    let desc = rows.windows(2).all(|w| w[0].date >= w[1].date);
    let asc = rows.windows(2).all(|w| w[0].date <= w[1].date);
    // Line numbers where the chain breaks, for each reading.
    let breaks = |newest_first: bool| -> Option<Vec<usize>> {
        let mut out = Vec::new();
        for w in rows.windows(2) {
            let (older, newer) = if newest_first {
                (&w[1], &w[0])
            } else {
                (&w[0], &w[1])
            };
            let (Some(ob), Some(nb)) = (older.balance, newer.balance) else {
                return None;
            };
            if ob.checked_add(newer.amount) != Some(nb) {
                out.push(newer.line);
            }
        }
        Some(out)
    };
    match (desc, asc) {
        (true, false) => (FileOrder::NewestFirst, breaks(true).unwrap_or_default()),
        (false, true) => (FileOrder::OldestFirst, breaks(false).unwrap_or_default()),
        // All on one day (or jumbled): let the balances decide.
        _ => match (breaks(true), breaks(false)) {
            (Some(n), Some(o)) if n.len() <= o.len() && (desc || asc) => {
                (FileOrder::NewestFirst, n)
            }
            (Some(_), Some(o)) if desc || asc => (FileOrder::OldestFirst, o),
            _ => (FileOrder::Unknown, Vec::new()),
        },
    }
}

/// What importing `rows` into `account_id` would do. Changes nothing.
pub fn preview(db: &Database, account_id: i64, rows: Vec<ParsedRow>) -> Result<Preview, DbError> {
    let account = accounts::get(db, account_id).map_err(|e| match e {
        DbError::RecordNotFound { .. } => DbError::Invalid {
            field: "account_id",
            message: "Choose an account to import into.".into(),
        },
        other => other,
    })?;
    if account.is_archived() {
        return Err(DbError::Invalid {
            field: "account_id",
            message: format!("{} is archived. Restore it first.", account.name),
        });
    }

    let mut existing_keys = HashSet::new();
    let mut manual: HashMap<(NaiveDate, i64), Vec<i64>> = HashMap::new();
    {
        let mut stmt = db.conn().prepare(
            "SELECT id, date, amount_cents, import_key FROM transactions \
             WHERE account_id = ?1 ORDER BY id",
        )?;
        let mut q = stmt.query([account_id])?;
        while let Some(r) = q.next()? {
            match r.get::<_, Option<String>>(3)? {
                Some(k) => {
                    existing_keys.insert(k);
                }
                None => manual
                    .entry((r.get(1)?, r.get(2)?))
                    .or_default()
                    .push(r.get(0)?),
            }
        }
    }

    let (order, balance_breaks) = order_and_breaks(&rows);
    let newest = match order {
        FileOrder::OldestFirst => rows.last(),
        _ => rows.first(),
    };
    let bank_balance = newest.and_then(|r| r.balance);
    let current_balance = accounts::balance(db, account_id)?;

    let keys = keys(&rows);
    let mut out = Vec::with_capacity(rows.len());
    let mut new_total = current_balance;
    for (row, import_key) in rows.into_iter().zip(keys) {
        let mut matched = None;
        let status = if existing_keys.contains(&import_key) {
            RowStatus::Duplicate
        } else if row.date < account.opening_date {
            RowStatus::BeforeOpening
        } else if let Some(id) = manual
            .get_mut(&(row.date, row.amount.cents()))
            .and_then(|ids| (!ids.is_empty()).then(|| ids.remove(0)))
        {
            matched = Some(id);
            RowStatus::PossibleMatch
        } else {
            new_total = new_total
                .checked_add(row.amount)
                .ok_or_else(|| file_error("The amounts in this file are too large."))?;
            RowStatus::New
        };
        out.push(PreviewRow {
            row: ImportRow { row, import_key },
            status,
            matched_transaction_id: matched,
        });
    }

    Ok(Preview {
        account_id,
        rows: out,
        order,
        balance_breaks,
        bank_balance,
        current_balance,
        balance_after: new_total,
    })
}

/// Insert the chosen rows in one SQL transaction. Rows whose key is already
/// in the account are skipped (so committing twice is harmless). Any invalid
/// row aborts the whole import.
pub fn commit(db: &Database, account_id: i64, rows: &[ImportRow]) -> Result<CommitResult, DbError> {
    let tx = db.conn().unchecked_transaction()?;
    let mut result = CommitResult {
        imported: 0,
        skipped: 0,
    };
    let mut in_batch = HashSet::new();
    for r in rows {
        let exists: bool = db.conn().query_row(
            "SELECT EXISTS (SELECT 1 FROM transactions WHERE account_id = ?1 AND import_key = ?2)",
            params![account_id, r.import_key],
            |q| q.get(0),
        )?;
        if exists || !in_batch.insert(r.import_key.as_str()) {
            result.skipped += 1;
            continue;
        }
        let input = TransactionInput {
            account_id,
            date: r.row.date,
            amount: r.row.amount,
            payee: r.row.payee.clone(),
            description: Some(r.row.description.clone()),
            memo: None,
            cleared: true,
        };
        let clean = transactions::validate(db, &input, None).map_err(|e| match e {
            DbError::Invalid { field, message } if field != "account_id" => {
                file_error(format!("Line {}: {message}", r.row.line))
            }
            other => other,
        })?;
        db.conn().execute(
            "INSERT INTO transactions (account_id, date, amount_cents, payee, description, memo, \
             cleared, import_key) VALUES (?1, ?2, ?3, ?4, ?5, NULL, 1, ?6)",
            params![
                account_id,
                input.date,
                input.amount.cents(),
                clean.payee,
                clean.description,
                r.import_key
            ],
        )?;
        result.imported += 1;
    }
    tx.commit()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::{AccountInput, AccountKind};
    use crate::transactions::TransactionQuery;
    use tempfile::TempDir;

    const FIXTURE: &str =
        include_str!("../../tests/fixtures/bank_of_melbourne/synthetic_export.csv");

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn setup(opening_cents: i64, opening: &str) -> (TempDir, Database, i64) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::create(&Database::path_in(dir.path()), "test password").unwrap();
        let a = accounts::create(
            &db,
            &AccountInput {
                name: "Everyday".into(),
                kind: AccountKind::Offset,
                institution: Some("Bank of Melbourne".into()),
                bsb: None,
                account_number: None,
                opening_balance: Money::from_cents(opening_cents),
                opening_date: d(opening),
                offset_for: None,
                notes: None,
            },
        )
        .unwrap()
        .id;
        (dir, db, a)
    }

    fn import_rows(p: &Preview, want: &[RowStatus]) -> Vec<ImportRow> {
        p.rows
            .iter()
            .filter(|r| want.contains(&r.status))
            .map(|r| r.row.clone())
            .collect()
    }

    fn statuses(p: &Preview) -> Vec<RowStatus> {
        p.rows.iter().map(|r| r.status).collect()
    }

    #[test]
    fn fixture_previews_commits_and_reconciles() {
        // Opening balance = the bank balance before the oldest row.
        let (_d, db, a) = setup(124_311, "2026-11-01");
        let p = preview(&db, a, bom::parse(FIXTURE).unwrap()).unwrap();
        assert_eq!(statuses(&p), [RowStatus::New; 5]);
        assert_eq!(p.order, FileOrder::NewestFirst);
        assert!(p.balance_breaks.is_empty(), "{:?}", p.balance_breaks);
        assert_eq!(p.bank_balance, Some(Money::from_cents(241_275)));
        assert_eq!(p.current_balance, Money::from_cents(124_311));
        assert_eq!(p.balance_after, p.bank_balance.unwrap(), "reconciles");

        let r = commit(&db, a, &import_rows(&p, &[RowStatus::New])).unwrap();
        assert_eq!(
            r,
            CommitResult {
                imported: 5,
                skipped: 0
            }
        );
        assert_eq!(
            accounts::balance(&db, a).unwrap(),
            Money::from_cents(241_275)
        );
        let txs = transactions::list(&db, &TransactionQuery::default()).unwrap();
        let bakery = &txs[0];
        assert_eq!(bakery.payee.as_deref(), Some("Corner Bakery"));
        assert!(
            bakery.cleared,
            "imported rows come from the bank, so cleared"
        );
        assert!(
            bakery
                .description
                .as_deref()
                .unwrap()
                .starts_with("Eftpos Debit")
        );
    }

    #[test]
    fn overlapping_export_only_adds_the_new_rows() {
        let (_d, db, a) = setup(0, "2026-01-01");
        let h = "Date,Description,Debit,Credit,Balance\r\n";
        let first = format!(
            "{h}11/03/2026,Coffee,4.50,,95.50\r\n11/02/2026,Coffee,4.50,,100.00\r\n11/01/2026,Pay,,104.50,104.50\r\n"
        );
        let p = preview(&db, a, bom::parse(&first).unwrap()).unwrap();
        commit(&db, a, &import_rows(&p, &[RowStatus::New])).unwrap();

        // Next export overlaps by two days and has two identical coffees on 11/04.
        let second = format!(
            "{h}11/04/2026,Coffee,4.50,,86.50\r\n11/04/2026,Coffee,4.50,,91.00\r\n11/03/2026,Coffee,4.50,,95.50\r\n11/02/2026,Coffee,4.50,,100.00\r\n"
        );
        let p = preview(&db, a, bom::parse(&second).unwrap()).unwrap();
        assert_eq!(
            statuses(&p),
            [
                RowStatus::New,
                RowStatus::New,
                RowStatus::Duplicate,
                RowStatus::Duplicate
            ]
        );
        assert_ne!(p.rows[0].row.import_key, p.rows[1].row.import_key);
        assert_eq!(p.balance_after, Money::from_cents(8_650));
        let r = commit(&db, a, &import_rows(&p, &[RowStatus::New])).unwrap();
        assert_eq!(r.imported, 2);
        assert_eq!(accounts::balance(&db, a).unwrap(), Money::from_cents(8_650));

        // Committing the same rows again is harmless.
        let again = commit(&db, a, &import_rows(&p, &[RowStatus::New])).unwrap();
        assert_eq!(
            again,
            CommitResult {
                imported: 0,
                skipped: 2
            }
        );
        let p = preview(&db, a, bom::parse(&second).unwrap()).unwrap();
        assert_eq!(statuses(&p), [RowStatus::Duplicate; 4]);
    }

    #[test]
    fn hand_entered_and_pre_opening_rows_are_flagged() {
        let (_d, db, a) = setup(10_000, "2026-11-02");
        let manual = transactions::add(
            &db,
            &TransactionInput {
                account_id: a,
                date: d("2026-11-03"),
                amount: Money::from_cents(-450),
                payee: Some("Coffee".into()),
                description: None,
                memo: None,
                cleared: false,
            },
        )
        .unwrap();
        let text = "Date,Description,Debit,Credit,Balance\n\
                    11/03/2026,Cafe A,4.50,,91.00\n11/03/2026,Cafe B,4.50,,95.50\n11/01/2026,Old,1.00,,100.00\n";
        let p = preview(&db, a, bom::parse(text).unwrap()).unwrap();
        assert_eq!(
            statuses(&p),
            [
                RowStatus::PossibleMatch,
                RowStatus::New,
                RowStatus::BeforeOpening
            ]
        );
        assert_eq!(p.rows[0].matched_transaction_id, Some(manual.id));
        assert_eq!(
            p.rows[1].matched_transaction_id, None,
            "one match per manual entry"
        );
        // Only New rows count toward the projected balance.
        assert_eq!(p.balance_after, Money::from_cents(10_000 - 450 - 450));
    }

    #[test]
    fn balance_breaks_point_at_missing_rows() {
        let (_d, db, a) = setup(0, "2026-01-01");
        // Row for 11/02 (-10.00) is missing between these two.
        let text = "Date,Description,Debit,Credit,Balance\n\
                    11/03/2026,C,5.00,,85.00\n11/01/2026,A,,100.00,100.00\n";
        let p = preview(&db, a, bom::parse(text).unwrap()).unwrap();
        assert_eq!(p.balance_breaks, [2]);

        let oldest_first = "Date,Description,Debit,Credit,Balance\n\
                    11/01/2026,A,,100.00,100.00\n11/03/2026,C,5.00,,95.00\n";
        let p = preview(&db, a, bom::parse(oldest_first).unwrap()).unwrap();
        assert_eq!(
            (p.order, p.balance_breaks.len()),
            (FileOrder::OldestFirst, 0)
        );
        assert_eq!(p.bank_balance, Some(Money::from_cents(9_500)));
    }

    #[test]
    fn errors() {
        let (_d, db, a) = setup(0, "2026-01-01");
        let rows = bom::parse(FIXTURE).unwrap();
        assert!(matches!(
            preview(&db, 999, rows.clone()),
            Err(DbError::Invalid {
                field: "account_id",
                ..
            })
        ));
        accounts::set_archived(&db, a, true).unwrap();
        assert!(matches!(
            preview(&db, a, rows.clone()),
            Err(DbError::Invalid {
                field: "account_id",
                ..
            })
        ));
        accounts::set_archived(&db, a, false).unwrap();

        // A bad row aborts the whole commit: nothing is written.
        let mut bad: Vec<ImportRow> = keys(&rows)
            .into_iter()
            .zip(rows)
            .map(|(import_key, row)| ImportRow { row, import_key })
            .collect();
        bad[3].row.date = d("1850-01-01");
        let e = commit(&db, a, &bad).unwrap_err();
        assert!(
            matches!(&e, DbError::Invalid { field: "file", message } if message.starts_with("Line 5: ")),
            "{e:?}"
        );
        assert_eq!(
            transactions::count(&db, &TransactionQuery::default()).unwrap(),
            0
        );
    }

    #[test]
    fn json_shapes_for_the_ui() {
        let (_d, db, a) = setup(0, "2026-01-01");
        let p = preview(&db, a, bom::parse(FIXTURE).unwrap()).unwrap();
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["order"], "newest_first");
        let r = &v["rows"][0];
        assert_eq!(r["status"], "new");
        assert_eq!(r["amount"], -650);
        assert_eq!(r["date"], "2026-11-18");
        assert_eq!(r["line"], 2);
        assert!(r["import_key"].is_string());
        // The UI sends rows back as they came.
        let back: ImportRow = serde_json::from_value(r.clone()).unwrap();
        assert_eq!(back, p.rows[0].row);
    }
}
