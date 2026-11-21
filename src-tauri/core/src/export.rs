//! CSV export of transactions.
//!
//! The file is written beside the database in an `exports` folder, named by
//! timestamp. Amounts are signed decimal numbers with two places (`-7.99` for
//! money out), so the file opens cleanly in a spreadsheet without currency
//! symbols or thousands separators getting in the way.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use rusqlite::params_from_iter;
use rusqlite::types::Value;
use serde::Serialize;

use crate::db::{Database, DbError};
use crate::transactions::TransactionQuery;

/// Folder beside the database holding exported files.
pub const EXPORT_DIR: &str = "exports";

/// First line of every export.
pub const HEADER: &str = "Date,Account,Payee,Description,Category,Amount,Transfer";

/// A file written by an export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportedFile {
    pub name: String,
    pub path: String,
    pub bytes: u64,
    /// How many transactions went into the file.
    pub rows: u64,
}

/// Folder holding exports, created on demand.
pub fn dir_in(data_dir: &Path) -> PathBuf {
    data_dir.join(EXPORT_DIR)
}

/// Quote a CSV field when it contains a comma, quote or line break.
fn field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// Signed decimal amount, two places, no currency symbol or grouping.
fn amount(cents: i64) -> String {
    let abs = cents.unsigned_abs();
    let sign = if cents < 0 { "-" } else { "" };
    format!("{sign}{}.{:02}", abs / 100, abs % 100)
}

/// Every transaction matching `query`, oldest first, as CSV text.
///
/// Paging in the query is ignored: an export covers everything the filters
/// match. Oldest first so the file reads like a statement.
pub fn csv_for(db: &Database, query: &TransactionQuery) -> Result<String, DbError> {
    let mut clauses: Vec<&str> = Vec::new();
    let mut args: Vec<Value> = Vec::new();
    if let Some(account_id) = query.account_id {
        clauses.push("t.account_id = ?");
        args.push(Value::Integer(account_id));
    }
    if let Some(from) = query.from {
        clauses.push("t.date >= ?");
        args.push(Value::Text(from.to_string()));
    }
    if let Some(to) = query.to {
        clauses.push("t.date <= ?");
        args.push(Value::Text(to.to_string()));
    }
    let where_sql = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };
    let sql = format!(
        "SELECT t.date, a.name, COALESCE(t.payee, ''), COALESCE(t.description, ''), \
                COALESCE(c.name, ''), t.amount_cents, t.transfer_id IS NOT NULL \
         FROM transactions t \
         JOIN accounts a ON a.id = t.account_id \
         LEFT JOIN categories c ON c.id = t.category_id\
         {where_sql} \
         ORDER BY t.date, t.id"
    );
    let mut stmt = db.conn().prepare(&sql)?;
    let mut rows = stmt.query(params_from_iter(args.iter()))?;

    let mut out = String::from(HEADER);
    out.push('\n');
    while let Some(row) = rows.next()? {
        let date: String = row.get(0)?;
        let account: String = row.get(1)?;
        let payee: String = row.get(2)?;
        let description: String = row.get(3)?;
        let category: String = row.get(4)?;
        let cents: i64 = row.get(5)?;
        let transfer: bool = row.get(6)?;
        let line = [
            field(&date),
            field(&account),
            field(&payee),
            field(&description),
            field(&category),
            amount(cents),
            if transfer {
                "yes".to_string()
            } else {
                String::new()
            },
        ]
        .join(",");
        out.push_str(&line);
        out.push('\n');
    }
    Ok(out)
}

/// Write an export into the `exports` folder and return what was written.
///
/// `now` is passed in so tests control the file name.
pub fn export_at(
    db: &Database,
    data_dir: &Path,
    now: DateTime<Local>,
    query: &TransactionQuery,
) -> Result<ExportedFile, DbError> {
    let csv = csv_for(db, query)?;
    let dir = dir_in(data_dir);
    std::fs::create_dir_all(&dir)?;
    let name = format!("transactions-{}.csv", now.format("%Y%m%d-%H%M%S"));
    let path = dir.join(&name);
    std::fs::write(&path, csv.as_bytes())?;
    Ok(ExportedFile {
        name,
        path: path.display().to_string(),
        bytes: csv.len() as u64,
        rows: csv.lines().count().saturating_sub(1) as u64,
    })
}

/// Like [`export_at`], named with the current time.
pub fn export_now(
    db: &Database,
    data_dir: &Path,
    query: &TransactionQuery,
) -> Result<ExportedFile, DbError> {
    export_at(db, data_dir, Local::now(), query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::{self, AccountInput, AccountKind};
    use crate::categories::{self, CategoryInput, GroupInput, GroupKind};
    use crate::money::Money;
    use crate::transactions::{self, TransactionInput};
    use chrono::{NaiveDate, TimeZone};
    use tempfile::TempDir;

    fn setup() -> (TempDir, Database, i64, i64) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::create(&Database::path_in(dir.path()), "long enough pw").unwrap();
        let account = accounts::create(
            &db,
            &AccountInput {
                name: "Everyday".to_string(),
                kind: AccountKind::Transaction,
                institution: Some("Bank of Melbourne".to_string()),
                bsb: None,
                account_number: None,
                opening_balance: Money::from_cents(100_000),
                opening_date: NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(),
                notes: None,
                offset_for: None,
            },
        )
        .unwrap();
        let group = categories::create_group(
            &db,
            &GroupInput {
                name: "Living".to_string(),
                kind: GroupKind::Expense,
            },
        )
        .unwrap();
        let category = categories::create(
            &db,
            &CategoryInput {
                group_id: group.id,
                name: "Groceries".to_string(),
            },
        )
        .unwrap();
        (dir, db, account.id, category.id)
    }

    fn add(db: &Database, account_id: i64, day: u32, cents: i64, payee: &str, desc: &str) -> i64 {
        transactions::add(
            db,
            &TransactionInput {
                account_id,
                date: NaiveDate::from_ymd_opt(2026, 11, day).unwrap(),
                amount: Money::from_cents(cents),
                payee: Some(payee.to_string()),
                description: Some(desc.to_string()),
                memo: None,
                cleared: false,
                category_id: None,
            },
        )
        .unwrap()
        .id
    }

    #[test]
    fn writes_a_header_and_signed_amounts_oldest_first() {
        let (_dir, db, account, _) = setup();
        add(&db, account, 20, -799, "Woolworths", "Eftpos");
        add(&db, account, 5, 250_000, "Salary", "Pay");
        let csv = csv_for(&db, &TransactionQuery::default()).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], HEADER);
        assert_eq!(lines.len(), 3);
        assert!(
            lines[1].starts_with("2026-11-05,Everyday,Salary,Pay,"),
            "{}",
            lines[1]
        );
        assert!(lines[1].ends_with(",2500.00,"), "{}", lines[1]);
        assert!(lines[2].starts_with("2026-11-20,Everyday,Woolworths,Eftpos,"));
        assert!(lines[2].ends_with(",-7.99,"), "{}", lines[2]);
    }

    #[test]
    fn quotes_fields_that_need_it() {
        let (_dir, db, account, _) = setup();
        add(
            &db,
            account,
            3,
            -100,
            "Smith, Jones & \"Co\"",
            "Line one\nLine two",
        );
        let csv = csv_for(&db, &TransactionQuery::default()).unwrap();
        assert!(csv.contains("\"Smith, Jones & \"\"Co\"\"\""), "{csv}");
        assert!(csv.contains("\"Line one\nLine two\""), "{csv}");
    }

    #[test]
    fn includes_the_category() {
        let (_dir, db, account, category) = setup();
        let id = add(&db, account, 4, -2000, "IGA", "Groceries");
        transactions::set_category(&db, &[id], Some(category)).unwrap();
        let csv = csv_for(&db, &TransactionQuery::default()).unwrap();
        assert!(csv.contains(",Groceries,-20.00,"), "{csv}");
        // The last column is empty, so this row is not a transfer.
        assert!(csv.lines().nth(1).unwrap().ends_with(",-20.00,"));
    }

    #[test]
    fn respects_the_filters() {
        let (_dir, db, account, _) = setup();
        add(&db, account, 3, -100, "Early", "");
        add(&db, account, 25, -200, "Late", "");
        let q = TransactionQuery {
            from: NaiveDate::from_ymd_opt(2026, 11, 10),
            ..Default::default()
        };
        let csv = csv_for(&db, &q).unwrap();
        assert!(!csv.contains("Early"), "{csv}");
        assert!(csv.contains("Late"), "{csv}");
    }

    #[test]
    fn writes_the_file_and_counts_rows() {
        let (dir, db, account, _) = setup();
        add(&db, account, 3, -100, "One", "");
        add(&db, account, 4, -200, "Two", "");
        let when = Local.with_ymd_and_hms(2026, 11, 21, 9, 30, 0).unwrap();
        let file = export_at(&db, dir.path(), when, &TransactionQuery::default()).unwrap();
        assert_eq!(file.name, "transactions-20261121-093000.csv");
        assert_eq!(file.rows, 2);
        let text = std::fs::read_to_string(&file.path).unwrap();
        assert_eq!(text.lines().count(), 3);
        assert_eq!(file.bytes, text.len() as u64);
        assert_eq!(
            file.path,
            dir_in(dir.path()).join(&file.name).display().to_string()
        );
    }
}
