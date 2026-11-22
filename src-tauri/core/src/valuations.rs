//! Market valuations for accounts tracked by value (super, shares, managed
//! investments, property).
//!
//! These accounts don't move with bank transactions; their balance is the
//! latest valuation. One valuation per account per day — entering the same
//! date again replaces the earlier figure, which is what "update the value"
//! should do when the owner corrects a typo.
//!
//! Values use the same sign convention as account balances: assets positive.

use std::collections::HashMap;

use chrono::NaiveDate;
use rusqlite::{OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use crate::accounts;
use crate::db::{Database, DbError};
use crate::money::Money;
use crate::transactions::invalid;

pub const MAX_NOTE_CHARS: usize = 500;

/// Fields the owner supplies, for both add and update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValuationInput {
    pub date: NaiveDate,
    pub value: Money,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Valuation {
    pub id: i64,
    pub account_id: i64,
    pub date: NaiveDate,
    pub value: Money,
    pub note: Option<String>,
    pub created_at: String,
}

/// Add a valuation, or replace the one already recorded for that date.
pub fn add(db: &Database, account_id: i64, input: &ValuationInput) -> Result<Valuation, DbError> {
    check_account(db, account_id)?;
    let note = clean_note(input.note.as_deref())?;
    db.conn().execute(
        "INSERT INTO valuations (account_id, date, value_cents, note) \
         VALUES (?1, ?2, ?3, ?4) \
         ON CONFLICT (account_id, date) DO UPDATE SET \
             value_cents = excluded.value_cents, \
             note = excluded.note",
        params![
            account_id,
            input.date.to_string(),
            input.value.cents(),
            note
        ],
    )?;
    get_by_date(db, account_id, input.date)
}

/// Change an existing valuation. Moving it onto a date that already has one is
/// a conflict rather than a silent overwrite, so the owner chooses.
pub fn update(db: &Database, id: i64, input: &ValuationInput) -> Result<Valuation, DbError> {
    let current = get(db, id)?;
    let note = clean_note(input.note.as_deref())?;
    let clash: Option<i64> = db
        .conn()
        .query_row(
            "SELECT id FROM valuations WHERE account_id = ?1 AND date = ?2 AND id <> ?3",
            params![current.account_id, input.date.to_string(), id],
            |r| r.get(0),
        )
        .optional()?;
    if clash.is_some() {
        return Err(DbError::Conflict {
            field: "date",
            message: "That account already has a valuation on this date.".into(),
        });
    }
    db.conn().execute(
        "UPDATE valuations SET date = ?1, value_cents = ?2, note = ?3 WHERE id = ?4",
        params![input.date.to_string(), input.value.cents(), note, id],
    )?;
    get(db, id)
}

pub fn delete(db: &Database, id: i64) -> Result<(), DbError> {
    let changed = db
        .conn()
        .execute("DELETE FROM valuations WHERE id = ?1", [id])?;
    if changed == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

/// Valuations for one account, newest first.
pub fn list(db: &Database, account_id: i64) -> Result<Vec<Valuation>, DbError> {
    accounts::get(db, account_id)?;
    let mut stmt = db.conn().prepare(
        "SELECT id, account_id, date, value_cents, note, created_at \
         FROM valuations WHERE account_id = ?1 ORDER BY date DESC, id DESC",
    )?;
    let rows = stmt.query_map([account_id], from_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

pub fn get(db: &Database, id: i64) -> Result<Valuation, DbError> {
    db.conn()
        .query_row(
            "SELECT id, account_id, date, value_cents, note, created_at \
             FROM valuations WHERE id = ?1",
            [id],
            from_row,
        )
        .optional()?
        .ok_or_else(|| not_found(id))
}

/// Latest valuation per account, for computing balances. Accounts without a
/// valuation are absent from the map.
pub fn latest_by_account(db: &Database) -> Result<HashMap<i64, Money>, DbError> {
    let mut stmt = db.conn().prepare(
        "SELECT v.account_id, v.value_cents \
         FROM valuations v \
         WHERE v.date = (SELECT MAX(w.date) FROM valuations w \
                         WHERE w.account_id = v.account_id)",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, i64>(0)?, Money::from_cents(r.get::<_, i64>(1)?)))
    })?;
    let mut out = HashMap::new();
    for row in rows {
        let (id, value) = row?;
        out.insert(id, value);
    }
    Ok(out)
}

fn get_by_date(db: &Database, account_id: i64, date: NaiveDate) -> Result<Valuation, DbError> {
    db.conn()
        .query_row(
            "SELECT id, account_id, date, value_cents, note, created_at \
             FROM valuations WHERE account_id = ?1 AND date = ?2",
            params![account_id, date.to_string()],
            from_row,
        )
        .optional()?
        .ok_or_else(|| not_found(account_id))
}

fn from_row(r: &Row<'_>) -> rusqlite::Result<Valuation> {
    Ok(Valuation {
        id: r.get(0)?,
        account_id: r.get(1)?,
        date: r.get(2)?,
        value: Money::from_cents(r.get(3)?),
        note: r.get(4)?,
        created_at: r.get(5)?,
    })
}

/// Valuations only make sense for kinds tracked by market value.
fn check_account(db: &Database, account_id: i64) -> Result<(), DbError> {
    let account = accounts::get(db, account_id)?;
    if !account.kind.is_valued() {
        return Err(invalid(
            "account_id",
            "Only super, shares, investment and property accounts take valuations.",
        ));
    }
    Ok(())
}

fn clean_note(note: Option<&str>) -> Result<Option<String>, DbError> {
    let Some(note) = note else { return Ok(None) };
    let trimmed = note.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > MAX_NOTE_CHARS {
        return Err(invalid(
            "note",
            format!("Keep the note under {MAX_NOTE_CHARS} characters."),
        ));
    }
    Ok(Some(trimmed.to_string()))
}

fn not_found(id: i64) -> DbError {
    DbError::RecordNotFound {
        what: "valuation",
        id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::{AccountInput, AccountKind};
    use tempfile::TempDir;

    fn db() -> (TempDir, Database) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::create(&Database::path_in(dir.path()), "test password").unwrap();
        (dir, db)
    }

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn account(db: &Database, kind: AccountKind, opening: i64) -> i64 {
        accounts::create(
            db,
            &AccountInput {
                name: format!("{kind:?} account"),
                kind,
                institution: None,
                bsb: None,
                account_number: None,
                opening_balance: Money::from_cents(opening),
                opening_date: date("2026-07-01"),
                offset_for: None,
                notes: None,
            },
        )
        .unwrap()
        .id
    }

    fn input(date_s: &str, cents: i64) -> ValuationInput {
        ValuationInput {
            date: date(date_s),
            value: Money::from_cents(cents),
            note: None,
        }
    }

    #[test]
    fn adds_and_lists_newest_first() {
        let (_d, db) = db();
        let id = account(&db, AccountKind::Super, 0);
        add(&db, id, &input("2026-09-30", 120_000)).unwrap();
        add(&db, id, &input("2026-11-20", 131_500)).unwrap();

        let list = list(&db, id).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].date, date("2026-11-20"));
        assert_eq!(list[0].value, Money::from_cents(131_500));
        assert_eq!(list[1].date, date("2026-09-30"));
    }

    #[test]
    fn same_date_replaces_instead_of_duplicating() {
        let (_d, db) = db();
        let id = account(&db, AccountKind::Shares, 0);
        let first = add(&db, id, &input("2026-11-20", 50_000)).unwrap();
        let second = add(&db, id, &input("2026-11-20", 55_000)).unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(second.value, Money::from_cents(55_000));
        assert_eq!(list(&db, id).unwrap().len(), 1);
    }

    #[test]
    fn rejects_accounts_that_are_not_valued() {
        let (_d, db) = db();
        let id = account(&db, AccountKind::Savings, 0);
        let err = add(&db, id, &input("2026-11-20", 1_000)).unwrap_err();
        assert!(matches!(
            err,
            DbError::Invalid {
                field: "account_id",
                ..
            }
        ));
    }

    #[test]
    fn update_refuses_to_collide_with_another_date() {
        let (_d, db) = db();
        let id = account(&db, AccountKind::Property, 0);
        add(&db, id, &input("2026-09-30", 90_000_000)).unwrap();
        let second = add(&db, id, &input("2026-11-20", 95_000_000)).unwrap();

        let err = update(&db, second.id, &input("2026-09-30", 96_000_000)).unwrap_err();
        assert!(matches!(err, DbError::Conflict { field: "date", .. }));
    }

    #[test]
    fn delete_removes_and_reports_missing() {
        let (_d, db) = db();
        let id = account(&db, AccountKind::Investment, 0);
        let v = add(&db, id, &input("2026-11-20", 10_000)).unwrap();
        delete(&db, v.id).unwrap();
        assert!(list(&db, id).unwrap().is_empty());
        assert!(matches!(
            delete(&db, v.id).unwrap_err(),
            DbError::RecordNotFound { .. }
        ));
    }

    #[test]
    fn latest_per_account_picks_the_newest_date() {
        let (_d, db) = db();
        let a = account(&db, AccountKind::Super, 0);
        let b = account(&db, AccountKind::Shares, 0);
        add(&db, a, &input("2026-09-30", 100_000)).unwrap();
        add(&db, a, &input("2026-11-20", 110_000)).unwrap();
        add(&db, b, &input("2026-10-31", 20_000)).unwrap();

        let latest = latest_by_account(&db).unwrap();
        assert_eq!(latest.get(&a), Some(&Money::from_cents(110_000)));
        assert_eq!(latest.get(&b), Some(&Money::from_cents(20_000)));
    }

    #[test]
    fn balances_use_the_latest_valuation_for_valued_accounts() {
        let (_d, db) = db();
        let id = account(&db, AccountKind::Super, 10_000);
        // No valuation yet: opening balance stands.
        assert_eq!(
            accounts::balance(&db, id).unwrap(),
            Money::from_cents(10_000)
        );

        add(&db, id, &input("2026-09-30", 120_000)).unwrap();
        add(&db, id, &input("2026-11-20", 131_500)).unwrap();

        assert_eq!(
            accounts::balance(&db, id).unwrap(),
            Money::from_cents(131_500)
        );
        assert_eq!(
            accounts::balances(&db).unwrap().get(&id),
            Some(&Money::from_cents(131_500))
        );
    }

    #[test]
    fn notes_are_trimmed_and_blank_ones_dropped() {
        let (_d, db) = db();
        let id = account(&db, AccountKind::Super, 0);
        let mut v = input("2026-11-20", 1_000);
        v.note = Some("  annual statement  ".into());
        assert_eq!(
            add(&db, id, &v).unwrap().note.as_deref(),
            Some("annual statement")
        );

        v.note = Some("   ".into());
        assert_eq!(add(&db, id, &v).unwrap().note, None);
    }
}
