//! Accounts: bank accounts, offsets, cards, loans, super, investments, etc.
//!
//! Sign convention: balances are from the owner's point of view — assets
//! positive, liabilities negative — so net worth is a plain sum.

use std::cmp::Ordering;
use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};
use rusqlite::{OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use crate::db::{Database, DbError};
use crate::money::Money;

pub const MAX_NAME_CHARS: usize = 100;
pub const MAX_INSTITUTION_CHARS: usize = 100;
pub const MAX_NOTES_CHARS: usize = 2000;
pub const MAX_ACCOUNT_NUMBER_DIGITS: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountKind {
    Transaction,
    Savings,
    Offset,
    CreditCard,
    HomeLoan,
    Loan,
    Super,
    Shares,
    Investment,
    Property,
    Cash,
    Other,
}

impl AccountKind {
    /// All kinds, in display order. Must match `KIND_ORDER` in
    /// src/lib/accounts.ts (Banking, Credit & loans, Investments & super, Other).
    pub const ALL: [AccountKind; 12] = [
        AccountKind::Transaction,
        AccountKind::Offset,
        AccountKind::Savings,
        AccountKind::Cash,
        AccountKind::CreditCard,
        AccountKind::HomeLoan,
        AccountKind::Loan,
        AccountKind::Super,
        AccountKind::Shares,
        AccountKind::Investment,
        AccountKind::Property,
        AccountKind::Other,
    ];

    /// Identifier stored in the database and sent to the UI.
    pub fn as_str(self) -> &'static str {
        match self {
            AccountKind::Transaction => "transaction",
            AccountKind::Savings => "savings",
            AccountKind::Offset => "offset",
            AccountKind::CreditCard => "credit_card",
            AccountKind::HomeLoan => "home_loan",
            AccountKind::Loan => "loan",
            AccountKind::Super => "super",
            AccountKind::Shares => "shares",
            AccountKind::Investment => "investment",
            AccountKind::Property => "property",
            AccountKind::Cash => "cash",
            AccountKind::Other => "other",
        }
    }

    pub fn parse(s: &str) -> Option<AccountKind> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }

    /// Money owed: balance is normally negative.
    pub fn is_liability(self) -> bool {
        matches!(
            self,
            AccountKind::CreditCard | AccountKind::HomeLoan | AccountKind::Loan
        )
    }

    /// Tracked by periodic valuations (market value) rather than bank
    /// transactions: super, shares, managed investments, property.
    pub fn is_valued(self) -> bool {
        matches!(
            self,
            AccountKind::Super
                | AccountKind::Shares
                | AccountKind::Investment
                | AccountKind::Property
        )
    }

    /// Kinds an offset account may be linked to.
    pub fn can_be_offset_target(self) -> bool {
        matches!(self, AccountKind::HomeLoan | AccountKind::Loan)
    }

    fn display_rank(self) -> usize {
        Self::ALL
            .iter()
            .position(|k| *k == self)
            .unwrap_or(usize::MAX)
    }
}

/// Fields the user edits, for both create and update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountInput {
    pub name: String,
    pub kind: AccountKind,
    #[serde(default)]
    pub institution: Option<String>,
    /// Accepts `063-000`, `063 000` or `063000`; stored as 6 digits.
    #[serde(default)]
    pub bsb: Option<String>,
    /// Digits; spaces and dashes are stripped.
    #[serde(default)]
    pub account_number: Option<String>,
    /// Signed cents: liabilities negative.
    pub opening_balance: Money,
    pub opening_date: NaiveDate,
    /// For offset accounts: id of the loan being offset.
    #[serde(default)]
    pub offset_for: Option<i64>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub kind: AccountKind,
    pub institution: Option<String>,
    pub bsb: Option<String>,
    pub account_number: Option<String>,
    pub opening_balance: Money,
    pub opening_date: NaiveDate,
    pub offset_for: Option<i64>,
    pub notes: Option<String>,
    pub archived_at: Option<String>,
    pub created_at: String,
}

impl Account {
    pub fn is_archived(&self) -> bool {
        self.archived_at.is_some()
    }
}

const SELECT: &str = "SELECT id, name, kind, institution, bsb, account_number, \
     opening_balance_cents, opening_date, offset_for_account_id, notes, archived_at, created_at \
     FROM accounts";

fn from_row(row: &Row<'_>) -> rusqlite::Result<Account> {
    let kind_str: String = row.get(2)?;
    let kind = AccountKind::parse(&kind_str).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Text,
            format!("unknown account kind {kind_str:?}").into(),
        )
    })?;
    Ok(Account {
        id: row.get(0)?,
        name: row.get(1)?,
        kind,
        institution: row.get(3)?,
        bsb: row.get(4)?,
        account_number: row.get(5)?,
        opening_balance: Money::from_cents(row.get(6)?),
        opening_date: row.get(7)?,
        offset_for: row.get(8)?,
        notes: row.get(9)?,
        archived_at: row.get(10)?,
        created_at: row.get(11)?,
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
        what: "account",
        id,
    }
}

/// Trim; empty becomes `None`; enforce a character limit.
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

fn digits_only(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(|s| {
            s.chars()
                .filter(|c| !matches!(c, ' ' | '-'))
                .collect::<String>()
        })
        .filter(|s| !s.is_empty())
}

/// Validated, normalised input ready to write.
struct Clean {
    name: String,
    kind: AccountKind,
    institution: Option<String>,
    bsb: Option<String>,
    account_number: Option<String>,
    opening_balance: Money,
    opening_date: NaiveDate,
    offset_for: Option<i64>,
    notes: Option<String>,
}

fn validate(db: &Database, input: &AccountInput, self_id: Option<i64>) -> Result<Clean, DbError> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(invalid("name", "Name is required."));
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(invalid(
            "name",
            format!("Name must be at most {MAX_NAME_CHARS} characters."),
        ));
    }

    let institution = optional_text(
        &input.institution,
        "institution",
        "Institution",
        MAX_INSTITUTION_CHARS,
    )?;
    let notes = optional_text(&input.notes, "notes", "Notes", MAX_NOTES_CHARS)?;

    let bsb = digits_only(&input.bsb);
    if let Some(b) = &bsb
        && (b.len() != 6 || !b.bytes().all(|c| c.is_ascii_digit()))
    {
        return Err(invalid("bsb", "BSB must be 6 digits, e.g. 063-000."));
    }

    let account_number = digits_only(&input.account_number);
    if let Some(n) = &account_number
        && (!n.bytes().all(|c| c.is_ascii_digit()) || n.len() > MAX_ACCOUNT_NUMBER_DIGITS)
    {
        return Err(invalid(
            "account_number",
            format!("Account number must be up to {MAX_ACCOUNT_NUMBER_DIGITS} digits."),
        ));
    }

    if input.opening_balance.cents().abs() > Money::MAX_CENTS {
        return Err(invalid("opening_balance", "Opening balance is too large."));
    }

    let year = input.opening_date.year();
    if !(1900..=2100).contains(&year) {
        return Err(invalid(
            "opening_date",
            "Opening date must be between 1900 and 2100.",
        ));
    }

    let offset_for = match (input.kind, input.offset_for) {
        (_, None) => None,
        (AccountKind::Offset, Some(target_id)) => {
            if Some(target_id) == self_id {
                return Err(invalid("offset_for", "An account can't offset itself."));
            }
            let target = get(db, target_id).map_err(|e| match e {
                DbError::RecordNotFound { .. } => invalid("offset_for", "Linked loan not found."),
                other => other,
            })?;
            if !target.kind.can_be_offset_target() {
                return Err(invalid(
                    "offset_for",
                    "An offset account can only be linked to a home loan or loan.",
                ));
            }
            if target.is_archived() {
                return Err(invalid("offset_for", "The linked loan is archived."));
            }
            Some(target_id)
        }
        (_, Some(_)) => {
            return Err(invalid(
                "offset_for",
                "Only offset accounts can be linked to a loan.",
            ));
        }
    };

    // A loan with offsets pointing at it must stay a loan.
    if let Some(id) = self_id
        && !input.kind.can_be_offset_target()
        && !offsets_for(db, id)?.is_empty()
    {
        return Err(invalid(
            "kind",
            "This loan has linked offset accounts. Unlink them before changing its type.",
        ));
    }

    ensure_name_free(db, &name, self_id)?;

    Ok(Clean {
        name,
        kind: input.kind,
        institution,
        bsb,
        account_number,
        opening_balance: input.opening_balance,
        opening_date: input.opening_date,
        offset_for,
        notes,
    })
}

fn ensure_name_free(db: &Database, name: &str, self_id: Option<i64>) -> Result<(), DbError> {
    let taken: Option<i64> = db
        .conn()
        .query_row(
            "SELECT id FROM accounts WHERE archived_at IS NULL \
             AND name = ?1 COLLATE NOCASE AND id IS NOT ?2",
            params![name, self_id],
            |r| r.get(0),
        )
        .optional()?;
    if taken.is_some() {
        Err(DbError::Conflict {
            field: "name",
            message: format!("An active account named \"{name}\" already exists."),
        })
    } else {
        Ok(())
    }
}

pub fn get(db: &Database, id: i64) -> Result<Account, DbError> {
    db.conn()
        .query_row(&format!("{SELECT} WHERE id = ?1"), [id], from_row)
        .optional()?
        .ok_or_else(|| not_found(id))
}

/// Accounts in display order: active first, then by kind, then name.
pub fn list(db: &Database, include_archived: bool) -> Result<Vec<Account>, DbError> {
    let sql = if include_archived {
        SELECT.to_string()
    } else {
        format!("{SELECT} WHERE archived_at IS NULL")
    };
    let mut stmt = db.conn().prepare(&sql)?;
    let mut accounts = stmt
        .query_map([], from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    accounts.sort_by(display_order);
    Ok(accounts)
}

fn display_order(a: &Account, b: &Account) -> Ordering {
    a.is_archived()
        .cmp(&b.is_archived())
        .then(a.kind.display_rank().cmp(&b.kind.display_rank()))
        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        .then(a.id.cmp(&b.id))
}

/// Offset accounts (active or archived) linked to `loan_id`.
pub fn offsets_for(db: &Database, loan_id: i64) -> Result<Vec<Account>, DbError> {
    let mut stmt = db
        .conn()
        .prepare(&format!("{SELECT} WHERE offset_for_account_id = ?1"))?;
    let mut accounts = stmt
        .query_map([loan_id], from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    accounts.sort_by(display_order);
    Ok(accounts)
}

pub fn create(db: &Database, input: &AccountInput) -> Result<Account, DbError> {
    let c = validate(db, input, None)?;
    db.conn().execute(
        "INSERT INTO accounts (name, kind, institution, bsb, account_number, \
         opening_balance_cents, opening_date, offset_for_account_id, notes) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            c.name,
            c.kind.as_str(),
            c.institution,
            c.bsb,
            c.account_number,
            c.opening_balance.cents(),
            c.opening_date,
            c.offset_for,
            c.notes
        ],
    )?;
    get(db, db.conn().last_insert_rowid())
}

pub fn update(db: &Database, id: i64, input: &AccountInput) -> Result<Account, DbError> {
    get(db, id)?;
    let c = validate(db, input, Some(id))?;
    db.conn().execute(
        "UPDATE accounts SET name = ?1, kind = ?2, institution = ?3, bsb = ?4, \
         account_number = ?5, opening_balance_cents = ?6, opening_date = ?7, \
         offset_for_account_id = ?8, notes = ?9 WHERE id = ?10",
        params![
            c.name,
            c.kind.as_str(),
            c.institution,
            c.bsb,
            c.account_number,
            c.opening_balance.cents(),
            c.opening_date,
            c.offset_for,
            c.notes,
            id
        ],
    )?;
    get(db, id)
}

/// Current balance of every account: opening balance plus transactions dated
/// on or after the account's opening date. Earlier transactions (e.g.
/// imported history) are already reflected in the opening balance and are
/// not counted again. See docs/DECISIONS.md.
pub fn balances(db: &Database) -> Result<HashMap<i64, Money>, DbError> {
    balances_where(db, None)
}

/// Current balance of one account (same rule as [`balances`]).
pub fn balance(db: &Database, id: i64) -> Result<Money, DbError> {
    balances_where(db, Some(id))?
        .remove(&id)
        .ok_or_else(|| not_found(id))
}

fn balances_where(db: &Database, id: Option<i64>) -> Result<HashMap<i64, Money>, DbError> {
    let mut stmt = db.conn().prepare(
        "SELECT a.id, a.opening_balance_cents, COALESCE(SUM(t.amount_cents), 0) \
         FROM accounts a \
         LEFT JOIN transactions t ON t.account_id = a.id AND t.date >= a.opening_date \
         WHERE ?1 IS NULL OR a.id = ?1 \
         GROUP BY a.id",
    )?;
    let rows = stmt.query_map([id], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
        ))
    })?;
    let mut out = HashMap::new();
    for row in rows {
        let (id, opening, movement) = row?;
        let total = Money::from_cents(opening)
            .checked_add(Money::from_cents(movement))
            .ok_or_else(|| invalid("amount", "Balance is too large to calculate."))?;
        out.insert(id, total);
    }
    Ok(out)
}

/// Archive (hide) or restore an account. History is kept either way.
pub fn set_archived(db: &Database, id: i64, archived: bool) -> Result<Account, DbError> {
    let account = get(db, id)?;
    if archived == account.is_archived() {
        return Ok(account);
    }
    if archived {
        db.conn().execute(
            "UPDATE accounts SET archived_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?1",
            [id],
        )?;
    } else {
        ensure_name_free(db, &account.name, Some(id))?;
        db.conn()
            .execute("UPDATE accounts SET archived_at = NULL WHERE id = ?1", [id])?;
    }
    get(db, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn db() -> (TempDir, Database) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::create(&Database::path_in(dir.path()), "test password").unwrap();
        (dir, db)
    }

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn input(name: &str, kind: AccountKind) -> AccountInput {
        AccountInput {
            name: name.into(),
            kind,
            institution: None,
            bsb: None,
            account_number: None,
            opening_balance: Money::ZERO,
            opening_date: date("2026-11-01"),
            offset_for: None,
            notes: None,
        }
    }

    fn field_of(e: &DbError) -> &'static str {
        match e {
            DbError::Invalid { field, .. } | DbError::Conflict { field, .. } => field,
            other => panic!("expected a field error, got {other:?}"),
        }
    }

    #[test]
    fn create_normalises_and_round_trips() {
        let (_d, db) = db();
        let a = create(
            &db,
            &AccountInput {
                name: "  Everyday Offset ".into(),
                institution: Some(" Bank of Melbourne ".into()),
                bsb: Some("193-879".into()),
                account_number: Some("4567 8901".into()),
                opening_balance: Money::from_cents(1_234_567),
                notes: Some("   ".into()),
                ..input("", AccountKind::Offset)
            },
        )
        .unwrap();
        assert_eq!(a.name, "Everyday Offset");
        assert_eq!(a.institution.as_deref(), Some("Bank of Melbourne"));
        assert_eq!(a.bsb.as_deref(), Some("193879"));
        assert_eq!(a.account_number.as_deref(), Some("45678901"));
        assert_eq!(a.notes, None);
        assert_eq!(a.opening_balance, Money::from_cents(1_234_567));
        assert_eq!(a.opening_date, date("2026-11-01"));
        assert!(!a.is_archived());
        assert!(a.created_at.ends_with('Z'));
        assert_eq!(get(&db, a.id).unwrap(), a);
    }

    #[test]
    fn every_kind_round_trips_through_the_database() {
        let (_d, db) = db();
        for kind in AccountKind::ALL {
            let a = create(&db, &input(kind.as_str(), kind)).unwrap();
            assert_eq!(get(&db, a.id).unwrap().kind, kind);
            assert_eq!(AccountKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(AccountKind::parse("chequing"), None);
    }

    #[test]
    fn display_order_matches_the_ui_groups() {
        // Keep in sync with KIND_ORDER in src/lib/accounts.ts.
        let ids: Vec<&str> = AccountKind::ALL.iter().map(|k| k.as_str()).collect();
        assert_eq!(
            ids,
            [
                "transaction",
                "offset",
                "savings",
                "cash",
                "credit_card",
                "home_loan",
                "loan",
                "super",
                "shares",
                "investment",
                "property",
                "other"
            ]
        );
    }

    #[test]
    fn kind_classification() {
        use AccountKind::*;
        for k in [CreditCard, HomeLoan, Loan] {
            assert!(k.is_liability(), "{k:?}");
        }
        for k in [Super, Shares, Investment, Property] {
            assert!(k.is_valued(), "{k:?}");
            assert!(!k.is_liability(), "{k:?}");
        }
        for k in [Transaction, Savings, Offset, Cash] {
            assert!(!k.is_liability() && !k.is_valued(), "{k:?}");
        }
        assert!(HomeLoan.can_be_offset_target() && Loan.can_be_offset_target());
        assert!(!CreditCard.can_be_offset_target());
    }

    #[test]
    fn validation_errors_name_the_field() {
        let (_d, db) = db();
        let k = AccountKind::Savings;
        let cases: Vec<(AccountInput, &str)> = vec![
            (input("   ", k), "name"),
            (input(&"x".repeat(MAX_NAME_CHARS + 1), k), "name"),
            (
                AccountInput {
                    bsb: Some("06300".into()),
                    ..input("a", k)
                },
                "bsb",
            ),
            (
                AccountInput {
                    bsb: Some("063-00A".into()),
                    ..input("a", k)
                },
                "bsb",
            ),
            (
                AccountInput {
                    account_number: Some("12AB".into()),
                    ..input("a", k)
                },
                "account_number",
            ),
            (
                AccountInput {
                    account_number: Some("1".repeat(21)),
                    ..input("a", k)
                },
                "account_number",
            ),
            (
                AccountInput {
                    opening_date: date("1899-12-31"),
                    ..input("a", k)
                },
                "opening_date",
            ),
            (
                AccountInput {
                    institution: Some("i".repeat(MAX_INSTITUTION_CHARS + 1)),
                    ..input("a", k)
                },
                "institution",
            ),
            (
                AccountInput {
                    opening_balance: Money::from_cents(Money::MAX_CENTS + 1),
                    ..input("a", k)
                },
                "opening_balance",
            ),
            (
                AccountInput {
                    offset_for: Some(1),
                    ..input("a", k)
                },
                "offset_for",
            ),
        ];
        for (inp, field) in cases {
            let e = create(&db, &inp).expect_err(field);
            assert_eq!(field_of(&e), field, "{inp:?}");
            assert_eq!(e.code(), "invalid");
        }
        assert!(list(&db, true).unwrap().is_empty(), "nothing written");
    }

    #[test]
    fn offsets_link_to_loans() {
        let (_d, db) = db();
        let loan = create(
            &db,
            &AccountInput {
                opening_balance: Money::from_cents(-45_000_000),
                ..input("Home Loan", AccountKind::HomeLoan)
            },
        )
        .unwrap();
        let link = |name: &str| AccountInput {
            offset_for: Some(loan.id),
            ..input(name, AccountKind::Offset)
        };
        let o1 = create(&db, &link("Offset 1")).unwrap();
        let o2 = create(&db, &link("Offset 2")).unwrap();
        assert_eq!(o1.offset_for, Some(loan.id));
        let linked: Vec<i64> = offsets_for(&db, loan.id)
            .unwrap()
            .iter()
            .map(|a| a.id)
            .collect();
        assert_eq!(linked, vec![o1.id, o2.id]);

        // Unlinked offsets are fine too (loan held elsewhere / not tracked).
        create(&db, &input("Standalone offset", AccountKind::Offset)).unwrap();
    }

    #[test]
    fn offset_link_rules() {
        let (_d, db) = db();
        let savings = create(&db, &input("Saver", AccountKind::Savings)).unwrap();
        let loan = create(&db, &input("Loan", AccountKind::Loan)).unwrap();
        let link_to = |id| AccountInput {
            offset_for: Some(id),
            ..input("Offset", AccountKind::Offset)
        };

        let e = create(&db, &link_to(savings.id)).unwrap_err();
        assert!(matches!(
            e,
            DbError::Invalid {
                field: "offset_for",
                ..
            }
        ));
        let e = create(&db, &link_to(9999)).unwrap_err();
        assert!(matches!(
            e,
            DbError::Invalid {
                field: "offset_for",
                ..
            }
        ));

        set_archived(&db, loan.id, true).unwrap();
        let e = create(&db, &link_to(loan.id)).unwrap_err();
        assert!(matches!(
            e,
            DbError::Invalid {
                field: "offset_for",
                ..
            }
        ));
        set_archived(&db, loan.id, false).unwrap();

        let o = create(&db, &link_to(loan.id)).unwrap();
        let e = update(&db, o.id, &link_to(o.id)).unwrap_err();
        assert!(matches!(
            e,
            DbError::Invalid {
                field: "offset_for",
                ..
            }
        ));

        // Changing an offset to another kind must drop the link explicitly.
        let e = update(
            &db,
            o.id,
            &AccountInput {
                offset_for: Some(loan.id),
                ..input("Offset", AccountKind::Savings)
            },
        )
        .unwrap_err();
        assert!(matches!(
            e,
            DbError::Invalid {
                field: "offset_for",
                ..
            }
        ));
    }

    #[test]
    fn linked_loan_cannot_change_to_non_loan_kind() {
        let (_d, db) = db();
        let loan = create(&db, &input("Home Loan", AccountKind::HomeLoan)).unwrap();
        create(
            &db,
            &AccountInput {
                offset_for: Some(loan.id),
                ..input("Offset", AccountKind::Offset)
            },
        )
        .unwrap();
        let e = update(&db, loan.id, &input("Home Loan", AccountKind::Savings)).unwrap_err();
        assert!(matches!(e, DbError::Invalid { field: "kind", .. }));
        // Switching between loan kinds is fine.
        update(&db, loan.id, &input("Home Loan", AccountKind::Loan)).unwrap();
    }

    #[test]
    fn active_names_are_unique_case_insensitively() {
        let (_d, db) = db();
        let a = create(&db, &input("Everyday", AccountKind::Transaction)).unwrap();
        let e = create(&db, &input("everyday", AccountKind::Savings)).unwrap_err();
        assert!(matches!(e, DbError::Conflict { field: "name", .. }));
        assert_eq!(e.code(), "conflict");

        // Renaming to its own name (different case) is fine.
        update(&db, a.id, &input("EVERYDAY", AccountKind::Transaction)).unwrap();

        // Archiving frees the name; restoring into a clash is refused.
        set_archived(&db, a.id, true).unwrap();
        let b = create(&db, &input("Everyday", AccountKind::Savings)).unwrap();
        let e = set_archived(&db, a.id, false).unwrap_err();
        assert!(matches!(e, DbError::Conflict { field: "name", .. }));
        set_archived(&db, b.id, true).unwrap();
        assert!(!set_archived(&db, a.id, false).unwrap().is_archived());
    }

    #[test]
    fn update_changes_fields_and_missing_ids_are_not_found() {
        let (_d, db) = db();
        let a = create(&db, &input("Old", AccountKind::Savings)).unwrap();
        let b = update(
            &db,
            a.id,
            &AccountInput {
                opening_balance: Money::from_cents(500),
                notes: Some("Bonus interest".into()),
                ..input("New", AccountKind::Cash)
            },
        )
        .unwrap();
        assert_eq!((b.name.as_str(), b.kind), ("New", AccountKind::Cash));
        assert_eq!(b.opening_balance, Money::from_cents(500));
        assert_eq!(b.created_at, a.created_at);

        for e in [
            get(&db, 42).unwrap_err(),
            update(&db, 42, &input("x", AccountKind::Cash)).unwrap_err(),
            set_archived(&db, 42, true).unwrap_err(),
        ] {
            assert!(matches!(
                e,
                DbError::RecordNotFound {
                    what: "account",
                    id: 42
                }
            ));
            assert_eq!(e.code(), "record_not_found");
        }
    }

    #[test]
    fn list_order_and_archive_filter() {
        let (_d, db) = db();
        let names = [
            ("zeta super", AccountKind::Super),
            ("Credit", AccountKind::CreditCard),
            ("b offset", AccountKind::Offset),
            ("A offset", AccountKind::Offset),
            ("Everyday", AccountKind::Transaction),
        ];
        for (n, k) in names {
            create(&db, &input(n, k)).unwrap();
        }
        let archived = create(&db, &input("Old cash", AccountKind::Cash)).unwrap();
        set_archived(&db, archived.id, true).unwrap();

        let active: Vec<String> = list(&db, false)
            .unwrap()
            .into_iter()
            .map(|a| a.name)
            .collect();
        assert_eq!(
            active,
            ["Everyday", "A offset", "b offset", "Credit", "zeta super"]
        );
        let all = list(&db, true).unwrap();
        assert_eq!(all.len(), 6);
        assert_eq!(all.last().unwrap().name, "Old cash", "archived sort last");

        // Archiving twice is a no-op that keeps the original timestamp.
        let first = get(&db, archived.id).unwrap().archived_at;
        assert_eq!(
            set_archived(&db, archived.id, true).unwrap().archived_at,
            first
        );
    }

    #[test]
    fn json_shape_for_the_ui() {
        let a = Account {
            id: 7,
            name: "Card".into(),
            kind: AccountKind::CreditCard,
            institution: None,
            bsb: None,
            account_number: None,
            opening_balance: Money::from_cents(-4250),
            opening_date: date("2026-11-20"),
            offset_for: None,
            notes: None,
            archived_at: None,
            created_at: "2026-11-20T00:00:00Z".into(),
        };
        let v = serde_json::to_value(&a).unwrap();
        assert_eq!(v["kind"], "credit_card");
        assert_eq!(v["opening_balance"], -4250);
        assert_eq!(v["opening_date"], "2026-11-20");

        let inp: AccountInput = serde_json::from_value(serde_json::json!({
            "name": "Offset", "kind": "offset",
            "opening_balance": 100, "opening_date": "2026-11-01"
        }))
        .unwrap();
        assert_eq!(inp.kind, AccountKind::Offset);
        assert_eq!(inp.offset_for, None);
    }

    #[test]
    fn accounts_persist_in_the_encrypted_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = Database::path_in(dir.path());
        let db = Database::create(&path, "test password").unwrap();
        create(&db, &input("Everyday", AccountKind::Offset)).unwrap();
        drop(db);
        let db = Database::open(&path, "test password").unwrap();
        assert_eq!(list(&db, false).unwrap()[0].name, "Everyday");
    }
}
