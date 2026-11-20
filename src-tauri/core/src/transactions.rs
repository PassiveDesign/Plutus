//! Transactions: money moving in or out of an account.
//!
//! Amounts are signed from the account's point of view: money in positive,
//! money out negative. An account's balance is its opening balance plus the
//! transactions dated on or after its opening date (see `accounts::balance`).

use chrono::{Datelike, NaiveDate};
use rusqlite::{OptionalExtension, Row, ToSql, params};
use serde::{Deserialize, Serialize};

use crate::accounts;
use crate::db::{Database, DbError};
use crate::money::Money;

pub const MAX_PAYEE_CHARS: usize = 200;
pub const MAX_DESCRIPTION_CHARS: usize = 500;
pub const MAX_MEMO_CHARS: usize = 1000;
/// Default and maximum page sizes for [`list`].
pub const DEFAULT_PAGE: u32 = 200;
pub const MAX_PAGE: u32 = 1000;

/// Fields the user (or an importer) supplies, for both add and update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionInput {
    pub account_id: i64,
    pub date: NaiveDate,
    /// Signed cents: money in positive, out negative.
    pub amount: Money,
    #[serde(default)]
    pub payee: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub memo: Option<String>,
    #[serde(default)]
    pub cleared: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    pub id: i64,
    pub account_id: i64,
    pub date: NaiveDate,
    pub amount: Money,
    pub payee: Option<String>,
    pub description: Option<String>,
    pub memo: Option<String>,
    pub cleared: bool,
    pub created_at: String,
    pub updated_at: Option<String>,
}

/// Filter for [`list`] / [`count`]. Dates are inclusive.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionQuery {
    #[serde(default)]
    pub account_id: Option<i64>,
    #[serde(default)]
    pub from: Option<NaiveDate>,
    #[serde(default)]
    pub to: Option<NaiveDate>,
    /// Page size; defaults to [`DEFAULT_PAGE`], capped at [`MAX_PAGE`].
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

const SELECT: &str = "SELECT id, account_id, date, amount_cents, payee, description, memo, \
     cleared, created_at, updated_at FROM transactions";

fn from_row(row: &Row<'_>) -> rusqlite::Result<Transaction> {
    Ok(Transaction {
        id: row.get(0)?,
        account_id: row.get(1)?,
        date: row.get(2)?,
        amount: Money::from_cents(row.get(3)?),
        payee: row.get(4)?,
        description: row.get(5)?,
        memo: row.get(6)?,
        cleared: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn invalid(field: &'static str, message: impl Into<String>) -> DbError {
    DbError::Invalid {
        field,
        message: message.into(),
    }
}

fn not_found(id: i64) -> DbError {
    DbError::RecordNotFound {
        what: "transaction",
        id,
    }
}

fn optional_text(
    value: &Option<String>,
    field: &'static str,
    label: &str,
    max: usize,
) -> Result<Option<String>, DbError> {
    match value.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(s) if s.chars().count() > max => Err(invalid(
            field,
            format!("{label} must be at most {max} characters."),
        )),
        Some(s) => Ok(Some(s.to_string())),
    }
}

struct Clean {
    payee: Option<String>,
    description: Option<String>,
    memo: Option<String>,
}

/// Validate input. `existing_account` is the transaction's current account
/// when updating: staying in an archived account is allowed (fixing old
/// history), moving into one is not.
fn validate(
    db: &Database,
    input: &TransactionInput,
    existing_account: Option<i64>,
) -> Result<Clean, DbError> {
    let account = accounts::get(db, input.account_id).map_err(|e| match e {
        DbError::RecordNotFound { .. } => invalid("account_id", "Account not found."),
        other => other,
    })?;
    if account.is_archived() && existing_account != Some(account.id) {
        return Err(invalid(
            "account_id",
            format!(
                "{} is archived. Restore it to add transactions.",
                account.name
            ),
        ));
    }

    if !(1900..=2100).contains(&input.date.year()) {
        return Err(invalid("date", "Date must be between 1900 and 2100."));
    }
    if input.amount.cents().abs() > Money::MAX_CENTS {
        return Err(invalid("amount", "Amount is too large."));
    }

    let payee = optional_text(&input.payee, "payee", "Payee", MAX_PAYEE_CHARS)?;
    let description = optional_text(
        &input.description,
        "description",
        "Description",
        MAX_DESCRIPTION_CHARS,
    )?;
    let memo = optional_text(&input.memo, "memo", "Memo", MAX_MEMO_CHARS)?;
    if payee.is_none() && description.is_none() {
        return Err(invalid("payee", "Enter a payee or description."));
    }
    Ok(Clean {
        payee,
        description,
        memo,
    })
}

pub fn get(db: &Database, id: i64) -> Result<Transaction, DbError> {
    db.conn()
        .query_row(&format!("{SELECT} WHERE id = ?1"), [id], from_row)
        .optional()?
        .ok_or_else(|| not_found(id))
}

pub fn add(db: &Database, input: &TransactionInput) -> Result<Transaction, DbError> {
    let c = validate(db, input, None)?;
    db.conn().execute(
        "INSERT INTO transactions (account_id, date, amount_cents, payee, description, memo, cleared) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            input.account_id,
            input.date,
            input.amount.cents(),
            c.payee,
            c.description,
            c.memo,
            input.cleared
        ],
    )?;
    get(db, db.conn().last_insert_rowid())
}

pub fn update(db: &Database, id: i64, input: &TransactionInput) -> Result<Transaction, DbError> {
    let existing = get(db, id)?;
    let c = validate(db, input, Some(existing.account_id))?;
    db.conn().execute(
        "UPDATE transactions SET account_id = ?1, date = ?2, amount_cents = ?3, payee = ?4, \
         description = ?5, memo = ?6, cleared = ?7, \
         updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?8",
        params![
            input.account_id,
            input.date,
            input.amount.cents(),
            c.payee,
            c.description,
            c.memo,
            input.cleared,
            id
        ],
    )?;
    get(db, id)
}

pub fn delete(db: &Database, id: i64) -> Result<(), DbError> {
    let n = db
        .conn()
        .execute("DELETE FROM transactions WHERE id = ?1", [id])?;
    if n == 0 { Err(not_found(id)) } else { Ok(()) }
}

/// WHERE clause + parameters shared by [`list`] and [`count`].
fn filter(q: &TransactionQuery) -> (String, Vec<Box<dyn ToSql>>) {
    let mut clauses: Vec<&str> = Vec::new();
    let mut args: Vec<Box<dyn ToSql>> = Vec::new();
    if let Some(a) = q.account_id {
        clauses.push("account_id = ?");
        args.push(Box::new(a));
    }
    if let Some(f) = q.from {
        clauses.push("date >= ?");
        args.push(Box::new(f));
    }
    if let Some(t) = q.to {
        clauses.push("date <= ?");
        args.push(Box::new(t));
    }
    let sql = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };
    (sql, args)
}

/// Newest first (date, then entry order), one page at a time.
pub fn list(db: &Database, q: &TransactionQuery) -> Result<Vec<Transaction>, DbError> {
    let (where_sql, mut args) = filter(q);
    let limit = q.limit.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE);
    args.push(Box::new(limit));
    args.push(Box::new(q.offset.unwrap_or(0)));
    let sql = format!("{SELECT}{where_sql} ORDER BY date DESC, id DESC LIMIT ? OFFSET ?");
    let mut stmt = db.conn().prepare(&sql)?;
    let refs: Vec<&dyn ToSql> = args.iter().map(|b| b.as_ref()).collect();
    let rows = stmt
        .query_map(refs.as_slice(), from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// A listed transaction plus the account's balance after it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LedgerRow {
    #[serde(flatten)]
    pub transaction: Transaction,
    /// Account balance after this transaction (opening balance + every
    /// transaction from the opening date up to and including this one, in
    /// date then entry order). `None` when the query isn't for a single
    /// account, or for rows dated before the account's opening date (already
    /// reflected in the opening balance, so they don't move it).
    pub running_balance: Option<Money>,
}

/// Like [`list`], with running balances when filtered to one account. The
/// running total is computed over the account's whole history first, so date
/// filters and paging don't change the figures.
pub fn list_with_running_balance(
    db: &Database,
    q: &TransactionQuery,
) -> Result<Vec<LedgerRow>, DbError> {
    let Some(account_id) = q.account_id else {
        return Ok(list(db, q)?
            .into_iter()
            .map(|transaction| LedgerRow {
                transaction,
                running_balance: None,
            })
            .collect());
    };

    let mut clauses: Vec<&str> = Vec::new();
    let mut args: Vec<Box<dyn ToSql>> = vec![Box::new(account_id)];
    if let Some(f) = q.from {
        clauses.push("date >= ?");
        args.push(Box::new(f));
    }
    if let Some(t) = q.to {
        clauses.push("date <= ?");
        args.push(Box::new(t));
    }
    let outer_where = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };
    let limit = q.limit.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE);
    args.push(Box::new(limit));
    args.push(Box::new(q.offset.unwrap_or(0)));

    let sql = format!(
        "SELECT * FROM ( \
           SELECT t.id, t.account_id, t.date, t.amount_cents, t.payee, t.description, t.memo, \
                  t.cleared, t.created_at, t.updated_at, \
                  CASE WHEN t.date >= a.opening_date THEN \
                    a.opening_balance_cents + SUM(CASE WHEN t.date >= a.opening_date \
                                                       THEN t.amount_cents ELSE 0 END) \
                      OVER (ORDER BY t.date, t.id ROWS UNBOUNDED PRECEDING) \
                  END AS running \
           FROM transactions t JOIN accounts a ON a.id = t.account_id \
           WHERE t.account_id = ? \
         ){outer_where} ORDER BY date DESC, id DESC LIMIT ? OFFSET ?"
    );
    let mut stmt = db.conn().prepare(&sql)?;
    let refs: Vec<&dyn ToSql> = args.iter().map(|b| b.as_ref()).collect();
    let rows = stmt
        .query_map(refs.as_slice(), |r| {
            Ok(LedgerRow {
                transaction: from_row(r)?,
                running_balance: r.get::<_, Option<i64>>(10)?.map(Money::from_cents),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Number of transactions matching the filter (ignores limit/offset).
pub fn count(db: &Database, q: &TransactionQuery) -> Result<u64, DbError> {
    let (where_sql, args) = filter(q);
    let refs: Vec<&dyn ToSql> = args.iter().map(|b| b.as_ref()).collect();
    let n: i64 = db.conn().query_row(
        &format!("SELECT count(*) FROM transactions{where_sql}"),
        refs.as_slice(),
        |r| r.get(0),
    )?;
    Ok(n as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::{AccountInput, AccountKind};
    use tempfile::TempDir;

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn setup() -> (TempDir, Database, i64) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::create(&Database::path_in(dir.path()), "test password").unwrap();
        let acct = account(&db, "Everyday Offset", 100_000, "2026-11-01");
        (dir, db, acct)
    }

    fn account(db: &Database, name: &str, opening: i64, opening_date: &str) -> i64 {
        accounts::create(
            db,
            &AccountInput {
                name: name.into(),
                kind: AccountKind::Offset,
                institution: None,
                bsb: None,
                account_number: None,
                opening_balance: Money::from_cents(opening),
                opening_date: date(opening_date),
                offset_for: None,
                notes: None,
            },
        )
        .unwrap()
        .id
    }

    fn tx(account_id: i64, d: &str, cents: i64, payee: &str) -> TransactionInput {
        TransactionInput {
            account_id,
            date: date(d),
            amount: Money::from_cents(cents),
            payee: Some(payee.into()),
            description: None,
            memo: None,
            cleared: false,
        }
    }

    fn field(e: DbError) -> &'static str {
        match e {
            DbError::Invalid { field, .. } => field,
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn add_and_get_round_trip_with_trimming() {
        let (_d, db, acct) = setup();
        let t = add(
            &db,
            &TransactionInput {
                payee: Some("  Woolworths ".into()),
                description: Some("WOOLWORTHS 3021 MELBOURNE AU".into()),
                memo: Some("   ".into()),
                cleared: true,
                ..tx(acct, "2026-11-15", -8_745, "")
            },
        )
        .unwrap();
        assert_eq!(t.payee.as_deref(), Some("Woolworths"));
        assert_eq!(
            t.description.as_deref(),
            Some("WOOLWORTHS 3021 MELBOURNE AU")
        );
        assert_eq!(t.memo, None);
        assert!(t.cleared);
        assert_eq!(t.amount, Money::from_cents(-8_745));
        assert_eq!(t.updated_at, None);
        assert_eq!(get(&db, t.id).unwrap(), t);
    }

    #[test]
    fn description_alone_is_enough_but_something_is_required() {
        let (_d, db, acct) = setup();
        add(
            &db,
            &TransactionInput {
                payee: None,
                description: Some("EFTPOS PURCHASE".into()),
                ..tx(acct, "2026-11-15", -100, "")
            },
        )
        .unwrap();
        let e = add(
            &db,
            &TransactionInput {
                payee: Some("  ".into()),
                ..tx(acct, "2026-11-15", -100, "")
            },
        )
        .unwrap_err();
        assert_eq!(field(e), "payee");
    }

    #[test]
    fn validation() {
        let (_d, db, acct) = setup();
        assert_eq!(
            field(add(&db, &tx(999, "2026-11-15", 1, "x")).unwrap_err()),
            "account_id"
        );
        assert_eq!(
            field(add(&db, &tx(acct, "1899-12-31", 1, "x")).unwrap_err()),
            "date"
        );
        assert_eq!(
            field(add(&db, &tx(acct, "2026-11-15", Money::MAX_CENTS + 1, "x")).unwrap_err()),
            "amount"
        );
        assert_eq!(
            field(
                add(
                    &db,
                    &tx(acct, "2026-11-15", 1, &"p".repeat(MAX_PAYEE_CHARS + 1))
                )
                .unwrap_err()
            ),
            "payee"
        );
        // Zero amounts are allowed (banks list $0.00 lines).
        add(&db, &tx(acct, "2026-11-15", 0, "Fee reversal")).unwrap();
    }

    #[test]
    fn archived_accounts_take_no_new_transactions_but_history_stays_editable() {
        let (_d, db, acct) = setup();
        let other = account(&db, "Bills", 0, "2026-11-01");
        let t = add(&db, &tx(acct, "2026-11-15", -500, "Coffee")).unwrap();
        accounts::set_archived(&db, acct, true).unwrap();

        let e = add(&db, &tx(acct, "2026-11-16", -500, "Coffee")).unwrap_err();
        assert_eq!(field(e), "account_id");
        // Fixing a typo in an archived account's history is fine.
        let fixed = update(&db, t.id, &tx(acct, "2026-11-15", -550, "Coffee")).unwrap();
        assert_eq!(fixed.amount, Money::from_cents(-550));

        // Moving into an archived account is not.
        let t2 = add(&db, &tx(other, "2026-11-15", -1, "x")).unwrap();
        assert_eq!(
            field(update(&db, t2.id, &tx(acct, "2026-11-15", -1, "x")).unwrap_err()),
            "account_id"
        );
    }

    #[test]
    fn update_and_delete() {
        let (_d, db, acct) = setup();
        let t = add(&db, &tx(acct, "2026-11-15", -500, "Coffee")).unwrap();
        let u = update(
            &db,
            t.id,
            &TransactionInput {
                memo: Some("with Sam".into()),
                cleared: true,
                ..tx(acct, "2026-11-14", -650, "Cafe")
            },
        )
        .unwrap();
        assert_eq!(u.payee.as_deref(), Some("Cafe"));
        assert_eq!(u.date, date("2026-11-14"));
        assert!(u.cleared && u.updated_at.is_some());
        assert_eq!(u.created_at, t.created_at);

        delete(&db, t.id).unwrap();
        for e in [
            get(&db, t.id).unwrap_err(),
            delete(&db, t.id).unwrap_err(),
            update(&db, t.id, &tx(acct, "2026-11-14", 1, "x")).unwrap_err(),
        ] {
            assert!(matches!(
                e,
                DbError::RecordNotFound {
                    what: "transaction",
                    ..
                }
            ));
        }
    }

    #[test]
    fn list_orders_filters_and_pages() {
        let (_d, db, a) = setup();
        let b = account(&db, "Bills", 0, "2026-11-01");
        let t1 = add(&db, &tx(a, "2026-11-10", -1, "a1")).unwrap();
        let t2 = add(&db, &tx(a, "2026-11-12", -2, "a2")).unwrap();
        let t3 = add(&db, &tx(a, "2026-11-12", -3, "a3")).unwrap(); // same day, later entry
        let t4 = add(&db, &tx(b, "2026-11-11", -4, "b1")).unwrap();
        let ids = |q: TransactionQuery| {
            list(&db, &q)
                .unwrap()
                .iter()
                .map(|t| t.id)
                .collect::<Vec<_>>()
        };

        assert_eq!(
            ids(TransactionQuery::default()),
            [t3.id, t2.id, t4.id, t1.id]
        );
        let only_a = TransactionQuery {
            account_id: Some(a),
            ..Default::default()
        };
        assert_eq!(ids(only_a.clone()), [t3.id, t2.id, t1.id]);
        assert_eq!(count(&db, &only_a).unwrap(), 3);

        let range = TransactionQuery {
            from: Some(date("2026-11-11")),
            to: Some(date("2026-11-12")),
            ..Default::default()
        };
        assert_eq!(
            ids(range.clone()),
            [t3.id, t2.id, t4.id],
            "inclusive both ends"
        );
        assert_eq!(count(&db, &range).unwrap(), 3);

        let page2 = TransactionQuery {
            limit: Some(2),
            offset: Some(2),
            ..Default::default()
        };
        assert_eq!(ids(page2.clone()), [t4.id, t1.id]);
        assert_eq!(count(&db, &page2).unwrap(), 4, "count ignores paging");

        let zero = TransactionQuery {
            limit: Some(0),
            ..Default::default()
        };
        assert_eq!(ids(zero).len(), 1, "limit is clamped to at least 1");
    }

    #[test]
    fn balance_counts_transactions_from_the_opening_date() {
        let (_d, db, acct) = setup(); // opening $1,000.00 as at 2026-11-01
        assert_eq!(
            accounts::balance(&db, acct).unwrap(),
            Money::from_cents(100_000)
        );
        add(&db, &tx(acct, "2026-10-31", -99_999, "Before opening")).unwrap();
        add(&db, &tx(acct, "2026-11-01", -2_500, "On opening day")).unwrap();
        add(&db, &tx(acct, "2026-11-20", 350_000, "Salary")).unwrap();
        assert_eq!(
            accounts::balance(&db, acct).unwrap(),
            Money::from_cents(100_000 - 2_500 + 350_000),
            "pre-opening transaction is not counted"
        );

        // Moving the opening date back brings the older transaction in.
        let mut a = accounts::get(&db, acct).unwrap();
        a.opening_date = date("2026-10-01");
        accounts::update(
            &db,
            acct,
            &AccountInput {
                name: a.name,
                kind: a.kind,
                institution: None,
                bsb: None,
                account_number: None,
                opening_balance: a.opening_balance,
                opening_date: a.opening_date,
                offset_for: None,
                notes: None,
            },
        )
        .unwrap();
        assert_eq!(
            accounts::balance(&db, acct).unwrap(),
            Money::from_cents(100_000 - 99_999 - 2_500 + 350_000)
        );
        assert!(matches!(
            accounts::balance(&db, 999),
            Err(DbError::RecordNotFound { .. })
        ));
    }

    #[test]
    fn balances_cover_every_account() {
        let (_d, db, a) = setup();
        let b = account(&db, "Bills", -500, "2026-11-01");
        add(&db, &tx(a, "2026-11-02", -100, "x")).unwrap();
        let all = accounts::balances(&db).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[&a], Money::from_cents(99_900));
        assert_eq!(
            all[&b],
            Money::from_cents(-500),
            "no transactions: opening balance"
        );
    }

    #[test]
    fn transactions_persist_in_the_encrypted_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = Database::path_in(dir.path());
        let db = Database::create(&path, "test password").unwrap();
        let acct = account(&db, "Everyday", 0, "2026-11-01");
        add(&db, &tx(acct, "2026-11-02", -420, "Coffee")).unwrap();
        drop(db);
        let db = Database::open(&path, "test password").unwrap();
        assert_eq!(
            list(&db, &TransactionQuery::default()).unwrap()[0]
                .payee
                .as_deref(),
            Some("Coffee")
        );
        assert_eq!(
            accounts::balance(&db, acct).unwrap(),
            Money::from_cents(-420)
        );
    }

    fn running(db: &Database, q: TransactionQuery) -> Vec<(String, Option<i64>)> {
        list_with_running_balance(db, &q)
            .unwrap()
            .into_iter()
            .map(|r| {
                (
                    r.transaction.payee.unwrap(),
                    r.running_balance.map(Money::cents),
                )
            })
            .collect()
    }

    #[test]
    fn running_balance_follows_date_then_entry_order() {
        let (_d, db, a) = setup(); // opening $1,000.00 as at 2026-11-01
        add(&db, &tx(a, "2026-11-10", -2_000, "second")).unwrap();
        add(&db, &tx(a, "2026-11-05", -1_000, "first")).unwrap();
        add(
            &db,
            &tx(a, "2026-11-10", 500, "third (same day, entered later)"),
        )
        .unwrap();
        add(&db, &tx(a, "2026-10-20", -99_999, "pre-opening")).unwrap();
        let q = TransactionQuery {
            account_id: Some(a),
            ..Default::default()
        };
        assert_eq!(
            running(&db, q),
            [
                ("third (same day, entered later)".to_string(), Some(97_500)),
                ("second".to_string(), Some(97_000)),
                ("first".to_string(), Some(99_000)),
                ("pre-opening".to_string(), None),
            ]
        );
        // Newest running balance equals the account balance.
        assert_eq!(
            accounts::balance(&db, a).unwrap(),
            Money::from_cents(97_500)
        );
    }

    #[test]
    fn date_filters_and_paging_do_not_change_running_figures() {
        let (_d, db, a) = setup();
        for (d, c, p) in [
            ("2026-11-02", -100, "a"),
            ("2026-11-03", -200, "b"),
            ("2026-11-04", -300, "c"),
            ("2026-11-05", -400, "d"),
        ] {
            add(&db, &tx(a, d, c, p)).unwrap();
        }
        let filtered = running(
            &db,
            TransactionQuery {
                account_id: Some(a),
                from: Some(date("2026-11-03")),
                to: Some(date("2026-11-04")),
                ..Default::default()
            },
        );
        assert_eq!(
            filtered,
            [
                ("c".to_string(), Some(99_400)),
                ("b".to_string(), Some(99_700))
            ]
        );
        let page2 = running(
            &db,
            TransactionQuery {
                account_id: Some(a),
                limit: Some(2),
                offset: Some(2),
                ..Default::default()
            },
        );
        assert_eq!(
            page2,
            [
                ("b".to_string(), Some(99_700)),
                ("a".to_string(), Some(99_900))
            ]
        );
    }

    #[test]
    fn running_balance_is_per_account_and_absent_for_all_accounts() {
        let (_d, db, a) = setup();
        let b = account(&db, "Bills", 5_000, "2026-11-01");
        add(&db, &tx(a, "2026-11-02", -100, "in a")).unwrap();
        add(&db, &tx(b, "2026-11-02", -700, "in b")).unwrap();
        assert_eq!(
            running(
                &db,
                TransactionQuery {
                    account_id: Some(b),
                    ..Default::default()
                }
            ),
            [("in b".to_string(), Some(4_300))]
        );
        assert!(
            running(&db, TransactionQuery::default())
                .iter()
                .all(|(_, r)| r.is_none())
        );
    }

    #[test]
    fn ledger_row_json_is_flat() {
        let (_d, db, a) = setup();
        add(&db, &tx(a, "2026-11-02", -100, "x")).unwrap();
        let row = &list_with_running_balance(
            &db,
            &TransactionQuery {
                account_id: Some(a),
                ..Default::default()
            },
        )
        .unwrap()[0];
        let v = serde_json::to_value(row).unwrap();
        assert_eq!(v["payee"], "x");
        assert_eq!(v["amount"], -100);
        assert_eq!(v["running_balance"], 99_900);
        assert!(v.get("transaction").is_none(), "flattened");
    }

    #[test]
    fn json_shape_for_the_ui() {
        let input: TransactionInput = serde_json::from_value(serde_json::json!({
            "account_id": 1, "date": "2026-11-15", "amount": -8745, "payee": "Woolworths"
        }))
        .unwrap();
        assert_eq!(input.amount, Money::from_cents(-8745));
        assert!(!input.cleared);
        let q: TransactionQuery = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(q, TransactionQuery::default());
    }
}
