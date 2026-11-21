//! Monthly budgets per expense category.
//!
//! Setting a budget for a month applies from that month onwards until a
//! later month changes it. Spent = money out minus refunds in the month for
//! that category (transfers are never categorised, so never counted).

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use chrono::{Datelike, NaiveDate};
use rusqlite::params;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::categories::{self, GroupKind};
use crate::db::{Database, DbError};
use crate::money::Money;
use crate::transactions::invalid;

/// A calendar month, written `YYYY-MM`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Month {
    year: i32,
    month: u32,
}

impl Month {
    pub fn new(year: i32, month: u32) -> Option<Month> {
        ((1900..=2100).contains(&year) && (1..=12).contains(&month))
            .then_some(Month { year, month })
    }

    pub fn of(date: NaiveDate) -> Month {
        Month {
            year: date.year(),
            month: date.month(),
        }
    }

    pub fn first_day(self) -> NaiveDate {
        NaiveDate::from_ymd_opt(self.year, self.month, 1).expect("valid month")
    }

    pub fn next(self) -> Month {
        if self.month == 12 {
            Month {
                year: self.year + 1,
                month: 1,
            }
        } else {
            Month {
                year: self.year,
                month: self.month + 1,
            }
        }
    }
}

impl fmt::Display for Month {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}", self.year, self.month)
    }
}

impl FromStr for Month {
    type Err = DbError;
    fn from_str(s: &str) -> Result<Month, DbError> {
        let bad = || invalid("month", format!("\"{s}\" isn't a month (YYYY-MM)."));
        let (y, m) = s.split_once('-').ok_or_else(bad)?;
        if y.len() != 4 || m.len() != 2 {
            return Err(bad());
        }
        Month::new(y.parse().map_err(|_| bad())?, m.parse().map_err(|_| bad())?).ok_or_else(bad)
    }
}

impl Serialize for Month {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Month {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Month, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BudgetLine {
    pub category_id: i64,
    pub group_id: i64,
    /// `None` when no budget applies to this category in the month.
    pub budgeted: Option<Money>,
    /// Money out minus refunds; negative if refunds exceed spending.
    pub spent: Money,
    /// `budgeted − spent` (negative = over budget); `None` without a budget.
    pub remaining: Option<Money>,
    /// The month the budget amount was set in (≤ this month).
    pub set_in: Option<Month>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MonthBudget {
    pub month: Month,
    /// Active expense categories, plus archived ones with a budget or
    /// spending this month, in category-tree order.
    pub lines: Vec<BudgetLine>,
    pub total_budgeted: Money,
    /// Spending in budgeted categories.
    pub total_spent_budgeted: Money,
    /// Spending in expense categories without a budget.
    pub unbudgeted_spent: Money,
    /// Money out with no category (net of uncategorised refunds).
    pub uncategorised_spent: Money,
    /// Money in, categorised under income groups.
    pub income: Money,
}

/// Set a category's budget from `month` onwards (until a later change).
pub fn set(db: &Database, category_id: i64, month: Month, amount: Money) -> Result<(), DbError> {
    if amount.is_negative() {
        return Err(invalid("amount", "A budget can't be negative."));
    }
    let category = categories::get(db, category_id).map_err(|e| match e {
        DbError::RecordNotFound { .. } => invalid("category_id", "Category not found."),
        other => other,
    })?;
    let group = categories::get_group(db, category.group_id)?;
    if group.kind != GroupKind::Expense {
        return Err(invalid(
            "category_id",
            "Budgets are for expense categories.",
        ));
    }
    db.conn().execute(
        "INSERT INTO budgets (category_id, month, amount_cents) VALUES (?1, ?2, ?3) \
         ON CONFLICT (category_id, month) DO UPDATE SET amount_cents = excluded.amount_cents",
        params![category_id, month.to_string(), amount.cents()],
    )?;
    Ok(())
}

/// Remove the change made in exactly `month` (the earlier amount, if any,
/// applies again).
pub fn clear(db: &Database, category_id: i64, month: Month) -> Result<(), DbError> {
    db.conn().execute(
        "DELETE FROM budgets WHERE category_id = ?1 AND month = ?2",
        params![category_id, month.to_string()],
    )?;
    Ok(())
}

pub fn month(db: &Database, month: Month) -> Result<MonthBudget, DbError> {
    let from = month.first_day();
    let to = month.next().first_day();

    // Budget in force: latest row at or before this month.
    let mut budgets: HashMap<i64, (Money, Month)> = HashMap::new();
    {
        let mut stmt = db.conn().prepare(
            "SELECT b.category_id, b.amount_cents, b.month FROM budgets b \
             WHERE b.month = (SELECT max(month) FROM budgets x \
                              WHERE x.category_id = b.category_id AND x.month <= ?1)",
        )?;
        let mut q = stmt.query([month.to_string()])?;
        while let Some(r) = q.next()? {
            let m: String = r.get(2)?;
            budgets.insert(r.get(0)?, (Money::from_cents(r.get(1)?), m.parse()?));
        }
    }
    // Net amount per category this month (transfers excluded by design).
    let mut net: HashMap<Option<i64>, i64> = HashMap::new();
    {
        let mut stmt = db.conn().prepare(
            "SELECT category_id, sum(amount_cents) FROM transactions \
             WHERE date >= ?1 AND date < ?2 AND transfer_id IS NULL GROUP BY category_id",
        )?;
        let mut q = stmt.query(params![from, to])?;
        while let Some(r) = q.next()? {
            net.insert(r.get(0)?, r.get(1)?);
        }
    }

    let overflow = || invalid("month", "The totals for this month are too large.");
    let add = |a: Money, b: Money| a.checked_add(b).ok_or_else(overflow);
    let mut out = MonthBudget {
        month,
        lines: Vec::new(),
        total_budgeted: Money::ZERO,
        total_spent_budgeted: Money::ZERO,
        unbudgeted_spent: Money::ZERO,
        uncategorised_spent: Money::from_cents(-net.get(&None).copied().unwrap_or(0)),
        income: Money::ZERO,
    };
    for g in categories::tree(db, true)? {
        for c in &g.categories {
            let amount = net.get(&Some(c.id)).copied().unwrap_or(0);
            if g.kind == GroupKind::Income {
                out.income = add(out.income, Money::from_cents(amount))?;
                continue;
            }
            let spent = Money::from_cents(-amount);
            // A zero budget means "no longer budgeted".
            let budget = budgets.get(&c.id).filter(|(b, _)| !b.is_zero());
            let active = c.archived_at.is_none() && g.archived_at.is_none();
            if !active && budget.is_none() && amount == 0 {
                continue;
            }
            let remaining = match budget {
                Some((b, _)) => Some(b.checked_sub(spent).ok_or_else(overflow)?),
                None => None,
            };
            match budget {
                Some((b, _)) => {
                    out.total_budgeted = add(out.total_budgeted, *b)?;
                    out.total_spent_budgeted = add(out.total_spent_budgeted, spent)?;
                }
                None => out.unbudgeted_spent = add(out.unbudgeted_spent, spent)?,
            }
            out.lines.push(BudgetLine {
                category_id: c.id,
                group_id: g.id,
                budgeted: budget.map(|(b, _)| *b),
                spent,
                remaining,
                set_in: budget.map(|(_, m)| *m),
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::{self, AccountInput, AccountKind};
    use crate::transactions::{self, TransactionInput};
    use tempfile::TempDir;

    fn m(s: &str) -> Month {
        s.parse().unwrap()
    }

    fn setup() -> (TempDir, Database, i64) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::create(&Database::path_in(dir.path()), "test password").unwrap();
        let mk = |name: &str| {
            accounts::create(
                &db,
                &AccountInput {
                    name: name.into(),
                    kind: AccountKind::Offset,
                    institution: None,
                    bsb: None,
                    account_number: None,
                    opening_balance: Money::ZERO,
                    opening_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                    offset_for: None,
                    notes: None,
                },
            )
            .unwrap()
            .id
        };
        let a = mk("Everyday");
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

    fn spend(db: &Database, a: i64, date: &str, cents: i64, category: Option<i64>) {
        transactions::add(
            db,
            &TransactionInput {
                account_id: a,
                date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
                amount: Money::from_cents(cents),
                payee: Some("x".into()),
                description: None,
                memo: None,
                cleared: false,
                category_id: category,
            },
        )
        .unwrap();
    }

    fn line(b: &MonthBudget, id: i64) -> &BudgetLine {
        b.lines.iter().find(|l| l.category_id == id).unwrap()
    }

    #[test]
    fn months_parse_print_and_step() {
        assert_eq!(m("2026-11").to_string(), "2026-11");
        assert_eq!(m("2026-12").next(), m("2027-01"));
        assert_eq!(
            Month::of(NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()),
            m("2026-02")
        );
        for bad in ["2026-13", "2026-1", "26-11", "2026/11", "x", "1899-12"] {
            assert!(bad.parse::<Month>().is_err(), "{bad}");
        }
        let j: Month = serde_json::from_value(serde_json::json!("2026-11")).unwrap();
        assert_eq!(serde_json::to_value(j).unwrap(), "2026-11");
    }

    #[test]
    fn budgets_carry_forward_until_changed() {
        let (_d, db, _a) = setup();
        let g = cat(&db, "Groceries");
        set(&db, g, m("2026-09"), Money::from_cents(80_000)).unwrap();
        set(&db, g, m("2026-11"), Money::from_cents(90_000)).unwrap();
        let at = |s: &str| line(&month(&db, m(s)).unwrap(), g).budgeted;
        assert_eq!(at("2026-08"), None);
        assert_eq!(at("2026-09"), Some(Money::from_cents(80_000)));
        assert_eq!(at("2026-10"), Some(Money::from_cents(80_000)));
        assert_eq!(at("2027-03"), Some(Money::from_cents(90_000)));
        assert_eq!(
            line(&month(&db, m("2026-10")).unwrap(), g).set_in,
            Some(m("2026-09"))
        );

        // Setting the same month again replaces it; clearing reverts.
        set(&db, g, m("2026-11"), Money::from_cents(95_000)).unwrap();
        assert_eq!(at("2026-12"), Some(Money::from_cents(95_000)));
        clear(&db, g, m("2026-11")).unwrap();
        assert_eq!(at("2026-12"), Some(Money::from_cents(80_000)));
        // Zero stops budgeting from that month.
        set(&db, g, m("2027-01"), Money::ZERO).unwrap();
        assert_eq!(at("2027-02"), None);
        assert_eq!(at("2026-12"), Some(Money::from_cents(80_000)));
    }

    #[test]
    fn spent_remaining_and_totals() {
        let (_d, db, a) = setup();
        let (g, fuel, eat, salary) = (
            cat(&db, "Groceries"),
            cat(&db, "Fuel"),
            cat(&db, "Eating out"),
            cat(&db, "Salary & wages"),
        );
        set(&db, g, m("2026-11"), Money::from_cents(50_000)).unwrap();
        set(&db, fuel, m("2026-11"), Money::from_cents(10_000)).unwrap();
        spend(&db, a, "2026-11-03", -30_000, Some(g));
        spend(&db, a, "2026-11-20", -25_000, Some(g)); // over budget
        spend(&db, a, "2026-11-21", 1_000, Some(g)); // refund
        spend(&db, a, "2026-10-31", -99_999, Some(g)); // other month
        spend(&db, a, "2026-12-01", -99_999, Some(g)); // other month
        spend(&db, a, "2026-11-05", -4_000, Some(fuel));
        spend(&db, a, "2026-11-06", -2_500, Some(eat)); // unbudgeted
        spend(&db, a, "2026-11-07", -700, None); // uncategorised
        spend(&db, a, "2026-11-15", 300_000, Some(salary));

        let b = month(&db, m("2026-11")).unwrap();
        let gl = line(&b, g);
        assert_eq!(gl.spent, Money::from_cents(54_000));
        assert_eq!(gl.remaining, Some(Money::from_cents(-4_000)));
        assert_eq!(line(&b, fuel).remaining, Some(Money::from_cents(6_000)));
        let el = line(&b, eat);
        assert_eq!(
            (el.budgeted, el.remaining, el.spent),
            (None, None, Money::from_cents(2_500))
        );
        assert_eq!(b.total_budgeted, Money::from_cents(60_000));
        assert_eq!(b.total_spent_budgeted, Money::from_cents(58_000));
        assert_eq!(b.unbudgeted_spent, Money::from_cents(2_500));
        assert_eq!(b.uncategorised_spent, Money::from_cents(700));
        assert_eq!(b.income, Money::from_cents(300_000));
        assert!(
            b.lines.iter().all(|l| l.category_id != salary),
            "income isn't budgeted"
        );
    }

    #[test]
    fn archived_categories_show_only_when_relevant() {
        let (_d, db, a) = setup();
        let pets = cat(&db, "Pets");
        let kids = cat(&db, "Kids");
        spend(&db, a, "2026-11-03", -1_000, Some(pets));
        categories::set_archived(&db, pets, true).unwrap();
        categories::set_archived(&db, kids, true).unwrap();
        let b = month(&db, m("2026-11")).unwrap();
        assert!(b.lines.iter().any(|l| l.category_id == pets));
        assert!(b.lines.iter().all(|l| l.category_id != kids));
    }

    #[test]
    fn validation() {
        let (_d, db, _a) = setup();
        let field = |e: DbError| match e {
            DbError::Invalid { field, .. } => field,
            o => panic!("{o:?}"),
        };
        let g = cat(&db, "Groceries");
        assert_eq!(
            field(set(&db, g, m("2026-11"), Money::from_cents(-1)).unwrap_err()),
            "amount"
        );
        assert_eq!(
            field(set(&db, 99_999, m("2026-11"), Money::ZERO).unwrap_err()),
            "category_id"
        );
        let salary = cat(&db, "Salary & wages");
        assert_eq!(
            field(set(&db, salary, m("2026-11"), Money::from_cents(1)).unwrap_err()),
            "category_id"
        );
    }

    #[test]
    fn json_shape() {
        let (_d, db, _a) = setup();
        let g = cat(&db, "Groceries");
        set(&db, g, m("2026-11"), Money::from_cents(50_000)).unwrap();
        let v = serde_json::to_value(month(&db, m("2026-11")).unwrap()).unwrap();
        assert_eq!(v["month"], "2026-11");
        let l = v["lines"]
            .as_array()
            .unwrap()
            .iter()
            .find(|l| l["category_id"] == g)
            .unwrap();
        assert_eq!(
            (
                l["budgeted"].as_i64(),
                l["spent"].as_i64(),
                l["set_in"].as_str()
            ),
            (Some(50_000), Some(0), Some("2026-11"))
        );
    }
}
