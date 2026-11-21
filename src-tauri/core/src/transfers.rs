//! Transfers: money moved between two of the owner's accounts.
//!
//! A transfer is stored as two transactions sharing a `transfer_id` (the
//! outgoing leg's id): `-amount` in the source account and `+amount` in the
//! destination, same date. They are created, changed and deleted together.

use chrono::NaiveDate;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::accounts;
use crate::db::{Database, DbError};
use crate::money::Money;
use crate::transactions::{self, Transaction, TransactionInput, invalid};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferInput {
    pub from_account: i64,
    pub to_account: i64,
    pub date: NaiveDate,
    /// Positive cents moved from `from_account` to `to_account`.
    pub amount: Money,
    #[serde(default)]
    pub memo: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Transfer {
    /// The shared `transfer_id`.
    pub id: i64,
    /// Outgoing leg (negative amount).
    pub from: Transaction,
    /// Incoming leg (positive amount).
    pub to: Transaction,
}

fn not_found(id: i64) -> DbError {
    DbError::RecordNotFound {
        what: "transfer",
        id,
    }
}

/// Both legs as `TransactionInput`s, validated with the normal transaction
/// rules (accounts exist and are active, date range, text limits). Errors on
/// a leg's `account_id` are reported against `from_account` / `to_account`.
fn legs(
    db: &Database,
    input: &TransferInput,
    existing: Option<&Transfer>,
) -> Result<[(TransactionInput, transactions::Clean); 2], DbError> {
    if input.from_account == input.to_account {
        return Err(invalid(
            "to_account",
            "Choose a different account to transfer to.",
        ));
    }
    if input.amount.cents() <= 0 {
        return Err(invalid("amount", "Enter an amount greater than zero."));
    }
    let name = |id: i64, field: &'static str| -> Result<String, DbError> {
        accounts::get(db, id).map(|a| a.name).map_err(|e| match e {
            DbError::RecordNotFound { .. } => invalid(field, "Account not found."),
            other => other,
        })
    };
    let from_name = name(input.from_account, "from_account")?;
    let to_name = name(input.to_account, "to_account")?;

    let make = |account_id: i64, cents: i64, payee: String, cleared: bool| TransactionInput {
        account_id,
        date: input.date,
        amount: Money::from_cents(cents),
        payee: Some(payee),
        description: None,
        memo: input.memo.clone(),
        cleared,
    };
    let out = make(
        input.from_account,
        -input.amount.cents(),
        format!("Transfer to {to_name}"),
        existing.is_some_and(|t| t.from.cleared && t.from.account_id == input.from_account),
    );
    let inc = make(
        input.to_account,
        input.amount.cents(),
        format!("Transfer from {from_name}"),
        existing.is_some_and(|t| t.to.cleared && t.to.account_id == input.to_account),
    );

    let rename = |e: DbError, field: &'static str| match e {
        DbError::Invalid {
            field: "account_id",
            message,
        } => invalid(field, message),
        other => other,
    };
    let out_clean = transactions::validate(db, &out, existing.map(|t| t.from.account_id))
        .map_err(|e| rename(e, "from_account"))?;
    let in_clean = transactions::validate(db, &inc, existing.map(|t| t.to.account_id))
        .map_err(|e| rename(e, "to_account"))?;
    Ok([(out, out_clean), (inc, in_clean)])
}

fn insert(db: &Database, leg: &(TransactionInput, transactions::Clean)) -> Result<i64, DbError> {
    let (i, c) = leg;
    db.conn().execute(
        "INSERT INTO transactions (account_id, date, amount_cents, payee, description, memo, cleared) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            i.account_id,
            i.date,
            i.amount.cents(),
            c.payee,
            c.description,
            c.memo,
            i.cleared
        ],
    )?;
    Ok(db.conn().last_insert_rowid())
}

fn rewrite(
    db: &Database,
    id: i64,
    leg: &(TransactionInput, transactions::Clean),
) -> Result<(), DbError> {
    let (i, c) = leg;
    db.conn().execute(
        "UPDATE transactions SET account_id = ?1, date = ?2, amount_cents = ?3, payee = ?4, \
         memo = ?5, cleared = ?6, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
         WHERE id = ?7",
        params![
            i.account_id,
            i.date,
            i.amount.cents(),
            c.payee,
            c.memo,
            i.cleared,
            id
        ],
    )?;
    Ok(())
}

pub fn get(db: &Database, id: i64) -> Result<Transfer, DbError> {
    let mut stmt = db
        .conn()
        .prepare("SELECT id FROM transactions WHERE transfer_id = ?1 ORDER BY amount_cents")?;
    let ids = stmt
        .query_map([id], |r| r.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    match ids.as_slice() {
        [out, inc] => Ok(Transfer {
            id,
            from: transactions::get(db, *out)?,
            to: transactions::get(db, *inc)?,
        }),
        _ => Err(not_found(id)),
    }
}

/// Create both legs atomically.
pub fn create(db: &Database, input: &TransferInput) -> Result<Transfer, DbError> {
    let [out, inc] = legs(db, input, None)?;
    let tx = db.conn().unchecked_transaction()?;
    let out_id = insert(db, &out)?;
    let in_id = insert(db, &inc)?;
    db.conn().execute(
        "UPDATE transactions SET transfer_id = ?1 WHERE id IN (?1, ?2)",
        params![out_id, in_id],
    )?;
    tx.commit()?;
    get(db, out_id)
}

/// Change accounts, date, amount or memo of both legs together. Payees are
/// regenerated from the account names; each leg keeps its cleared flag unless
/// its account changed.
pub fn update(db: &Database, id: i64, input: &TransferInput) -> Result<Transfer, DbError> {
    let existing = get(db, id)?;
    let [out, inc] = legs(db, input, Some(&existing))?;
    let tx = db.conn().unchecked_transaction()?;
    rewrite(db, existing.from.id, &out)?;
    rewrite(db, existing.to.id, &inc)?;
    tx.commit()?;
    get(db, id)
}

/// Delete both legs.
pub fn delete(db: &Database, id: i64) -> Result<(), DbError> {
    let n = db
        .conn()
        .execute("DELETE FROM transactions WHERE transfer_id = ?1", [id])?;
    if n == 0 { Err(not_found(id)) } else { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::{AccountInput, AccountKind};
    use crate::transactions::TransactionQuery;
    use tempfile::TempDir;

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn account(db: &Database, name: &str, opening: i64) -> i64 {
        accounts::create(
            db,
            &AccountInput {
                name: name.into(),
                kind: AccountKind::Offset,
                institution: None,
                bsb: None,
                account_number: None,
                opening_balance: Money::from_cents(opening),
                opening_date: date("2026-11-01"),
                offset_for: None,
                notes: None,
            },
        )
        .unwrap()
        .id
    }

    fn setup() -> (TempDir, Database, i64, i64) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::create(&Database::path_in(dir.path()), "test password").unwrap();
        let a = account(&db, "Everyday", 100_000);
        let b = account(&db, "Bills", 0);
        (dir, db, a, b)
    }

    fn input(from: i64, to: i64, cents: i64) -> TransferInput {
        TransferInput {
            from_account: from,
            to_account: to,
            date: date("2026-11-15"),
            amount: Money::from_cents(cents),
            memo: Some("Power bill money".into()),
        }
    }

    fn field(e: DbError) -> &'static str {
        match e {
            DbError::Invalid { field, .. } => field,
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn create_makes_two_mirrored_linked_legs() {
        let (_d, db, a, b) = setup();
        let t = create(&db, &input(a, b, 25_000)).unwrap();
        assert_eq!(t.id, t.from.id);
        assert_eq!((t.from.account_id, t.from.amount.cents()), (a, -25_000));
        assert_eq!((t.to.account_id, t.to.amount.cents()), (b, 25_000));
        assert_eq!(t.from.payee.as_deref(), Some("Transfer to Bills"));
        assert_eq!(t.to.payee.as_deref(), Some("Transfer from Everyday"));
        assert_eq!(t.to.memo.as_deref(), Some("Power bill money"));
        for leg in [&t.from, &t.to] {
            assert_eq!(leg.transfer_id, Some(t.id));
            assert_eq!(leg.date, date("2026-11-15"));
        }
        assert_eq!(t.from.transfer_account_id, Some(b));
        assert_eq!(t.to.transfer_account_id, Some(a));

        let bal = accounts::balances(&db).unwrap();
        assert_eq!(bal[&a], Money::from_cents(75_000));
        assert_eq!(bal[&b], Money::from_cents(25_000));
    }

    #[test]
    fn validation() {
        let (_d, db, a, b) = setup();
        assert_eq!(
            field(create(&db, &input(a, a, 100)).unwrap_err()),
            "to_account"
        );
        assert_eq!(field(create(&db, &input(a, b, 0)).unwrap_err()), "amount");
        assert_eq!(field(create(&db, &input(a, b, -5)).unwrap_err()), "amount");
        assert_eq!(
            field(create(&db, &input(999, b, 100)).unwrap_err()),
            "from_account"
        );
        assert_eq!(
            field(create(&db, &input(a, 999, 100)).unwrap_err()),
            "to_account"
        );
        accounts::set_archived(&db, b, true).unwrap();
        assert_eq!(
            field(create(&db, &input(a, b, 100)).unwrap_err()),
            "to_account"
        );
        let q = TransactionQuery::default();
        assert_eq!(
            transactions::count(&db, &q).unwrap(),
            0,
            "nothing half-written"
        );
    }

    #[test]
    fn update_moves_both_legs_together() {
        let (_d, db, a, b) = setup();
        let c = account(&db, "Saver", 0);
        let t = create(&db, &input(a, b, 25_000)).unwrap();
        // Clear the incoming leg via an ordinary (allowed) leg edit.
        let mut leg = TransactionInput {
            account_id: t.to.account_id,
            date: t.to.date,
            amount: t.to.amount,
            payee: t.to.payee.clone(),
            description: None,
            memo: t.to.memo.clone(),
            cleared: true,
        };
        transactions::update(&db, t.to.id, &leg).unwrap();

        let u = update(
            &db,
            t.id,
            &TransferInput {
                to_account: c,
                date: date("2026-11-16"),
                amount: Money::from_cents(30_000),
                memo: None,
                ..input(a, b, 0)
            },
        )
        .unwrap();
        assert_eq!(
            (u.from.id, u.to.id),
            (t.from.id, t.to.id),
            "same rows, not recreated"
        );
        assert_eq!(u.from.amount.cents(), -30_000);
        assert_eq!((u.to.account_id, u.to.amount.cents()), (c, 30_000));
        assert_eq!(u.from.payee.as_deref(), Some("Transfer to Saver"));
        assert!(u.from.memo.is_none() && u.to.date == date("2026-11-16"));
        assert!(
            !u.to.cleared,
            "cleared resets when the leg's account changes"
        );

        let bal = accounts::balances(&db).unwrap();
        assert_eq!(
            (bal[&a].cents(), bal[&b].cents(), bal[&c].cents()),
            (70_000, 0, 30_000)
        );

        // Cleared survives an update that keeps the account.
        leg.account_id = c;
        leg.date = u.to.date;
        leg.amount = u.to.amount;
        leg.payee = u.to.payee.clone();
        transactions::update(&db, u.to.id, &leg).unwrap();
        let again = update(
            &db,
            t.id,
            &TransferInput {
                to_account: c,
                memo: Some("x".into()),
                date: date("2026-11-16"),
                ..input(a, b, 30_000)
            },
        )
        .unwrap();
        assert!(again.to.cleared);
    }

    #[test]
    fn ordinary_edits_cannot_unbalance_a_transfer() {
        let (_d, db, a, b) = setup();
        let t = create(&db, &input(a, b, 25_000)).unwrap();
        let base = TransactionInput {
            account_id: a,
            date: t.from.date,
            amount: t.from.amount,
            payee: Some("Renamed".into()),
            description: None,
            memo: Some("own note".into()),
            cleared: true,
        };
        // Payee, memo and cleared are per leg.
        let ok = transactions::update(&db, t.from.id, &base).unwrap();
        assert_eq!(ok.payee.as_deref(), Some("Renamed"));
        assert_eq!(
            get(&db, t.id).unwrap().to.memo.as_deref(),
            Some("Power bill money")
        );

        for bad in [
            TransactionInput {
                amount: Money::from_cents(-1),
                ..base.clone()
            },
            TransactionInput {
                date: date("2026-11-20"),
                ..base.clone()
            },
            TransactionInput {
                account_id: b,
                ..base.clone()
            },
        ] {
            assert_eq!(
                field(transactions::update(&db, t.from.id, &bad).unwrap_err()),
                "transfer"
            );
        }
    }

    #[test]
    fn deleting_either_leg_or_the_transfer_removes_both() {
        let (_d, db, a, b) = setup();
        let q = TransactionQuery::default();
        let t1 = create(&db, &input(a, b, 100)).unwrap();
        transactions::delete(&db, t1.to.id).unwrap();
        assert_eq!(transactions::count(&db, &q).unwrap(), 0);

        let t2 = create(&db, &input(a, b, 100)).unwrap();
        delete(&db, t2.id).unwrap();
        assert_eq!(transactions::count(&db, &q).unwrap(), 0);
        assert!(matches!(
            delete(&db, t2.id),
            Err(DbError::RecordNotFound {
                what: "transfer",
                ..
            })
        ));
        assert!(matches!(
            get(&db, t2.id),
            Err(DbError::RecordNotFound { .. })
        ));
        assert_eq!(
            accounts::balance(&db, a).unwrap(),
            Money::from_cents(100_000)
        );
    }

    #[test]
    fn running_balances_and_listing_include_transfer_fields() {
        let (_d, db, a, b) = setup();
        create(&db, &input(a, b, 25_000)).unwrap();
        let rows = transactions::list_with_running_balance(
            &db,
            &TransactionQuery {
                account_id: Some(b),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(rows[0].running_balance, Some(Money::from_cents(25_000)));
        assert_eq!(rows[0].transaction.transfer_account_id, Some(a));
        let v = serde_json::to_value(&rows[0]).unwrap();
        assert_eq!(v["transfer_account_id"], a);
        assert!(v["transfer_id"].is_i64());
    }

    #[test]
    fn json_input_shape() {
        let i: TransferInput = serde_json::from_value(serde_json::json!({
            "from_account": 1, "to_account": 2, "date": "2026-11-15", "amount": 2500
        }))
        .unwrap();
        assert_eq!(i.amount, Money::from_cents(2500));
        assert_eq!(i.memo, None);
    }
}
