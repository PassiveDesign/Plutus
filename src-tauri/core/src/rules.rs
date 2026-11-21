//! Auto-categorisation rules: "if the payee/description contains X (and
//! the money goes in/out), use category Y". Tried in `sort_order`; the
//! first match wins. Rules only fill in missing categories and never touch
//! transfers.

use rusqlite::{OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use crate::db::{Database, DbError};
use crate::money::Money;
use crate::transactions;

pub const MAX_PATTERN_CHARS: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchField {
    Payee,
    Description,
    Either,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Money in (amount > 0).
    In,
    /// Money out (amount < 0).
    Out,
}

impl MatchField {
    fn as_str(self) -> &'static str {
        match self {
            MatchField::Payee => "payee",
            MatchField::Description => "description",
            MatchField::Either => "either",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "payee" => MatchField::Payee,
            "description" => MatchField::Description,
            "either" => MatchField::Either,
            _ => return None,
        })
    }
}

impl Direction {
    fn as_str(self) -> &'static str {
        match self {
            Direction::In => "in",
            Direction::Out => "out",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        match s {
            "in" => Some(Direction::In),
            "out" => Some(Direction::Out),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleInput {
    pub pattern: String,
    pub match_field: MatchField,
    #[serde(default)]
    pub direction: Option<Direction>,
    pub category_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Rule {
    pub id: i64,
    pub pattern: String,
    pub match_field: MatchField,
    pub direction: Option<Direction>,
    pub category_id: i64,
    pub sort_order: i64,
}

impl Rule {
    /// Does this rule match a transaction with these fields?
    pub fn matches(&self, payee: Option<&str>, description: Option<&str>, amount: Money) -> bool {
        match self.direction {
            Some(Direction::In) if amount.cents() <= 0 => return false,
            Some(Direction::Out) if amount.cents() >= 0 => return false,
            _ => {}
        }
        let needle = self.pattern.to_lowercase();
        let has = |s: Option<&str>| s.is_some_and(|s| collapse(s).to_lowercase().contains(&needle));
        match self.match_field {
            MatchField::Payee => has(payee),
            MatchField::Description => has(description),
            MatchField::Either => has(payee) || has(description),
        }
    }
}

fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// First matching rule's category.
pub fn category_for(
    rules: &[Rule],
    payee: Option<&str>,
    description: Option<&str>,
    amount: Money,
) -> Option<i64> {
    rules
        .iter()
        .find(|r| r.matches(payee, description, amount))
        .map(|r| r.category_id)
}

const SELECT: &str = "SELECT r.id, r.pattern, r.match_field, r.direction, r.category_id, r.sort_order FROM category_rules r";

fn from_row(row: &Row<'_>) -> rusqlite::Result<Rule> {
    let bad = |i: usize, v: String| {
        rusqlite::Error::FromSqlConversionFailure(i, rusqlite::types::Type::Text, v.into())
    };
    let field: String = row.get(2)?;
    let dir: Option<String> = row.get(3)?;
    Ok(Rule {
        id: row.get(0)?,
        pattern: row.get(1)?,
        match_field: MatchField::parse(&field).ok_or_else(|| bad(2, field.clone()))?,
        direction: match dir {
            None => None,
            Some(d) => Some(Direction::parse(&d).ok_or_else(|| bad(3, d.clone()))?),
        },
        category_id: row.get(4)?,
        sort_order: row.get(5)?,
    })
}

fn not_found(id: i64) -> DbError {
    DbError::RecordNotFound { what: "rule", id }
}

pub fn get(db: &Database, id: i64) -> Result<Rule, DbError> {
    db.conn()
        .query_row(&format!("{SELECT} WHERE r.id = ?1"), [id], from_row)
        .optional()?
        .ok_or_else(|| not_found(id))
}

/// All rules in the order they are tried.
pub fn list(db: &Database) -> Result<Vec<Rule>, DbError> {
    let mut stmt = db
        .conn()
        .prepare(&format!("{SELECT} ORDER BY r.sort_order, r.id"))?;
    let rules = stmt.query_map([], from_row)?.collect::<Result<_, _>>()?;
    Ok(rules)
}

/// Rules whose category (and its group) is still active, in order.
pub fn active(db: &Database) -> Result<Vec<Rule>, DbError> {
    let mut stmt = db.conn().prepare(&format!(
        "{SELECT} JOIN categories c ON c.id = r.category_id \
         JOIN category_groups g ON g.id = c.group_id \
         WHERE c.archived_at IS NULL AND g.archived_at IS NULL \
         ORDER BY r.sort_order, r.id"
    ))?;
    let rules = stmt.query_map([], from_row)?.collect::<Result<_, _>>()?;
    Ok(rules)
}

fn clean(
    db: &Database,
    input: &RuleInput,
    current_category: Option<i64>,
) -> Result<String, DbError> {
    let pattern = collapse(&input.pattern);
    if pattern.is_empty() {
        return Err(transactions::invalid(
            "pattern",
            "Enter some text to look for.",
        ));
    }
    if pattern.chars().count() > MAX_PATTERN_CHARS {
        return Err(transactions::invalid(
            "pattern",
            format!("Keep it to {MAX_PATTERN_CHARS} characters or fewer."),
        ));
    }
    transactions::check_category(db, Some(input.category_id), current_category)?;
    Ok(pattern)
}

pub fn create(db: &Database, input: &RuleInput) -> Result<Rule, DbError> {
    let pattern = clean(db, input, None)?;
    db.conn().execute(
        "INSERT INTO category_rules (pattern, match_field, direction, category_id, sort_order) \
         VALUES (?1, ?2, ?3, ?4, (SELECT coalesce(max(sort_order), 0) + 1 FROM category_rules))",
        params![
            pattern,
            input.match_field.as_str(),
            input.direction.map(Direction::as_str),
            input.category_id
        ],
    )?;
    get(db, db.conn().last_insert_rowid())
}

pub fn update(db: &Database, id: i64, input: &RuleInput) -> Result<Rule, DbError> {
    let existing = get(db, id)?;
    let pattern = clean(db, input, Some(existing.category_id))?;
    db.conn().execute(
        "UPDATE category_rules SET pattern = ?1, match_field = ?2, direction = ?3, category_id = ?4 \
         WHERE id = ?5",
        params![
            pattern,
            input.match_field.as_str(),
            input.direction.map(Direction::as_str),
            input.category_id,
            id
        ],
    )?;
    get(db, id)
}

/// Rules are settings, not history, so they can be deleted.
pub fn delete(db: &Database, id: i64) -> Result<(), DbError> {
    let n = db
        .conn()
        .execute("DELETE FROM category_rules WHERE id = ?1", [id])?;
    if n == 0 { Err(not_found(id)) } else { Ok(()) }
}

/// Swap a rule with its neighbour (earlier when `up`). Returns the new order.
pub fn move_rule(db: &Database, id: i64, up: bool) -> Result<Vec<Rule>, DbError> {
    let mut rules = list(db)?;
    let i = rules
        .iter()
        .position(|r| r.id == id)
        .ok_or_else(|| not_found(id))?;
    let j = if up {
        i.checked_sub(1)
    } else {
        (i + 1 < rules.len()).then_some(i + 1)
    };
    if let Some(j) = j {
        rules.swap(i, j);
        let tx = db.conn().unchecked_transaction()?;
        for (n, r) in rules.iter().enumerate() {
            db.conn().execute(
                "UPDATE category_rules SET sort_order = ?1 WHERE id = ?2",
                params![n as i64 + 1, r.id],
            )?;
        }
        tx.commit()?;
    }
    list(db)
}

/// Categorise every uncategorised, non-transfer transaction (optionally only
/// `ids`) that a rule matches. Returns how many were categorised.
pub fn apply(db: &Database, ids: Option<&[i64]>) -> Result<usize, DbError> {
    let rules = active(db)?;
    if rules.is_empty() {
        return Ok(0);
    }
    let mut todo: Vec<(i64, Option<String>, Option<String>, i64)> = {
        let mut stmt = db.conn().prepare(
            "SELECT id, payee, description, amount_cents FROM transactions \
             WHERE category_id IS NULL AND transfer_id IS NULL",
        )?;
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<Result<_, _>>()?
    };
    if let Some(ids) = ids {
        todo.retain(|t| ids.contains(&t.0));
    }
    let tx = db.conn().unchecked_transaction()?;
    let mut n = 0;
    for (id, payee, desc, cents) in todo {
        if let Some(cat) = category_for(
            &rules,
            payee.as_deref(),
            desc.as_deref(),
            Money::from_cents(cents),
        ) {
            db.conn().execute(
                "UPDATE transactions SET category_id = ?1, \
                 updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?2",
                params![cat, id],
            )?;
            n += 1;
        }
    }
    tx.commit()?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::{self, AccountInput, AccountKind};
    use crate::categories;
    use crate::transactions::{TransactionInput, TransactionQuery};
    use chrono::NaiveDate;
    use tempfile::TempDir;

    fn setup() -> (TempDir, Database, i64) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::create(&Database::path_in(dir.path()), "test password").unwrap();
        let a = accounts::create(
            &db,
            &AccountInput {
                name: "Everyday".into(),
                kind: AccountKind::Offset,
                institution: None,
                bsb: None,
                account_number: None,
                opening_balance: Money::ZERO,
                opening_date: NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(),
                offset_for: None,
                notes: None,
            },
        )
        .unwrap()
        .id;
        (dir, db, a)
    }

    fn cat(db: &Database, name: &str) -> i64 {
        categories::tree(db, true)
            .unwrap()
            .into_iter()
            .flat_map(|g| g.categories)
            .find(|c| c.name == name)
            .unwrap()
            .id
    }

    fn rule(
        pattern: &str,
        field: MatchField,
        dir: Option<Direction>,
        category_id: i64,
    ) -> RuleInput {
        RuleInput {
            pattern: pattern.into(),
            match_field: field,
            direction: dir,
            category_id,
        }
    }

    fn add(db: &Database, a: i64, payee: Option<&str>, desc: Option<&str>, cents: i64) -> i64 {
        transactions::add(
            db,
            &TransactionInput {
                account_id: a,
                date: NaiveDate::from_ymd_opt(2026, 11, 5).unwrap(),
                amount: Money::from_cents(cents),
                payee: payee.map(Into::into),
                description: desc.map(Into::into),
                memo: None,
                cleared: false,
                category_id: None,
            },
        )
        .unwrap()
        .id
    }

    #[test]
    fn matching_is_case_insensitive_contains_with_field_and_direction() {
        let r = |field, dir| Rule {
            id: 1,
            pattern: "woolworths".into(),
            match_field: field,
            direction: dir,
            category_id: 1,
            sort_order: 1,
        };
        let out = Money::from_cents(-100);
        let inn = Money::from_cents(100);
        let desc = Some("EFTPOS   WOOLWORTHS  3021 MELB");
        assert!(r(MatchField::Either, None).matches(None, desc, out));
        assert!(r(MatchField::Description, None).matches(Some("x"), desc, out));
        assert!(!r(MatchField::Payee, None).matches(Some("x"), desc, out));
        assert!(r(MatchField::Payee, None).matches(Some("Woolworths Metro"), None, out));
        assert!(r(MatchField::Either, Some(Direction::Out)).matches(None, desc, out));
        assert!(!r(MatchField::Either, Some(Direction::Out)).matches(None, desc, inn));
        assert!(r(MatchField::Either, Some(Direction::In)).matches(None, desc, inn));
        // Spacing in the pattern is normalised the same way as the text.
        let spaced = Rule {
            pattern: "eftpos woolworths".into(),
            ..r(MatchField::Either, None)
        };
        assert!(spaced.matches(None, desc, out));
    }

    #[test]
    fn crud_order_and_validation() {
        let (_d, db, _a) = setup();
        let groceries = cat(&db, "Groceries");
        let fuel = cat(&db, "Fuel");
        let r1 = create(
            &db,
            &rule("  woolworths ", MatchField::Either, None, groceries),
        )
        .unwrap();
        assert_eq!(r1.pattern, "woolworths");
        let r2 = create(
            &db,
            &rule("shell", MatchField::Payee, Some(Direction::Out), fuel),
        )
        .unwrap();
        let r3 = create(&db, &rule("coles", MatchField::Either, None, groceries)).unwrap();
        assert_eq!(
            list(&db).unwrap().iter().map(|r| r.id).collect::<Vec<_>>(),
            [r1.id, r2.id, r3.id]
        );

        let ids = |v: Vec<Rule>| v.iter().map(|r| r.id).collect::<Vec<_>>();
        assert_eq!(
            ids(move_rule(&db, r3.id, true).unwrap()),
            [r1.id, r3.id, r2.id]
        );
        assert_eq!(
            ids(move_rule(&db, r1.id, true).unwrap()),
            [r1.id, r3.id, r2.id],
            "top stays"
        );
        assert_eq!(
            ids(move_rule(&db, r2.id, false).unwrap()),
            [r1.id, r3.id, r2.id],
            "bottom stays"
        );

        let u = update(&db, r2.id, &rule("BP", MatchField::Either, None, fuel)).unwrap();
        assert_eq!((u.pattern.as_str(), u.direction), ("BP", None));
        delete(&db, r3.id).unwrap();
        assert_eq!(list(&db).unwrap().len(), 2);
        assert!(matches!(
            delete(&db, r3.id),
            Err(DbError::RecordNotFound { what: "rule", .. })
        ));

        let field = |e: DbError| match e {
            DbError::Invalid { field, .. } => field,
            o => panic!("{o:?}"),
        };
        assert_eq!(
            field(create(&db, &rule("  ", MatchField::Either, None, fuel)).unwrap_err()),
            "pattern"
        );
        assert_eq!(
            field(
                create(&db, &rule(&"x".repeat(101), MatchField::Either, None, fuel)).unwrap_err()
            ),
            "pattern"
        );
        assert_eq!(
            field(create(&db, &rule("x", MatchField::Either, None, 99_999)).unwrap_err()),
            "category_id"
        );
        categories::set_archived(&db, fuel, true).unwrap();
        assert_eq!(
            field(create(&db, &rule("x", MatchField::Either, None, fuel)).unwrap_err()),
            "category_id"
        );
        // An existing rule may keep its (now archived) category when edited.
        update(
            &db,
            r2.id,
            &rule("BP Connect", MatchField::Either, None, fuel),
        )
        .unwrap();
    }

    #[test]
    fn apply_fills_only_uncategorised_non_transfers_first_rule_wins() {
        let (_d, db, a) = setup();
        let groceries = cat(&db, "Groceries");
        let eating = cat(&db, "Eating out");
        let salary = cat(&db, "Salary & wages");
        create(
            &db,
            &rule("woolworths", MatchField::Either, None, groceries),
        )
        .unwrap();
        create(&db, &rule("wool", MatchField::Either, None, eating)).unwrap(); // shadowed
        create(
            &db,
            &rule("acme", MatchField::Description, Some(Direction::In), salary),
        )
        .unwrap();

        let w = add(&db, a, Some("Woolworths"), None, -5_000);
        let pay = add(&db, a, None, Some("Direct Credit ACME PTY LTD"), 300_000);
        let refund = add(&db, a, None, Some("ACME refund"), -100); // money out: no match
        let set = add(&db, a, Some("Woolworths"), None, -100);
        transactions::set_category(&db, &[set], Some(eating)).unwrap();
        let b = accounts::create(
            &db,
            &AccountInput {
                name: "Bills".into(),
                kind: AccountKind::Offset,
                institution: None,
                bsb: None,
                account_number: None,
                opening_balance: Money::ZERO,
                opening_date: NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(),
                offset_for: None,
                notes: None,
            },
        )
        .unwrap()
        .id;
        let x = crate::transfers::create(
            &db,
            &crate::transfers::TransferInput {
                from_account: a,
                to_account: b,
                date: NaiveDate::from_ymd_opt(2026, 11, 6).unwrap(),
                amount: Money::from_cents(100),
                memo: Some("woolworths money".into()),
            },
        )
        .unwrap();

        assert_eq!(apply(&db, None).unwrap(), 2);
        let c = |id| transactions::get(&db, id).unwrap().category_id;
        assert_eq!(c(w), Some(groceries));
        assert_eq!(c(pay), Some(salary));
        assert_eq!(c(refund), None);
        assert_eq!(c(set), Some(eating), "never overwrites");
        assert_eq!(c(x.from.id), None);
        assert_eq!(apply(&db, None).unwrap(), 0, "idempotent");

        // Rules for archived categories are skipped.
        let w2 = add(&db, a, Some("Woolworths"), None, -1);
        categories::set_archived(&db, groceries, true).unwrap();
        assert_eq!(apply(&db, Some(&[w2])).unwrap(), 1);
        assert_eq!(c(w2), Some(eating), "falls through to the next rule");
        let _ = TransactionQuery::default();
    }

    #[test]
    fn json_shapes() {
        let i: RuleInput = serde_json::from_value(serde_json::json!({
            "pattern": "x", "match_field": "either", "category_id": 3
        }))
        .unwrap();
        assert_eq!(i.direction, None);
        let r = Rule {
            id: 1,
            pattern: "x".into(),
            match_field: MatchField::Payee,
            direction: Some(Direction::Out),
            category_id: 3,
            sort_order: 1,
        };
        let v = serde_json::to_value(r).unwrap();
        assert_eq!(
            (v["match_field"].as_str(), v["direction"].as_str()),
            (Some("payee"), Some("out"))
        );
    }
}
