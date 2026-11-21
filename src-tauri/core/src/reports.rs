//! Figures for the dashboard and reports. Read-only.

use chrono::NaiveDate;
use rusqlite::params;
use serde::Serialize;

use crate::accounts;
use crate::budgets::{self, Month};
use crate::db::{Database, DbError};
use crate::money::Money;
use crate::transactions::{self, Transaction, TransactionQuery, invalid};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct NetWorth {
    /// Sum of positive balances.
    pub assets: Money,
    /// Sum of negative balances (a negative number).
    pub debts: Money,
    pub net: Money,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CategorySpend {
    /// `None` = uncategorised.
    pub category_id: Option<i64>,
    /// Money out minus refunds (positive).
    pub spent: Money,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MonthTotals {
    /// Money in, excluding transfers.
    pub income: Money,
    /// Money out, excluding transfers (positive).
    pub spending: Money,
    /// `income − spending`.
    pub net: Money,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Dashboard {
    pub month: Month,
    pub net_worth: NetWorth,
    pub totals: MonthTotals,
    /// Biggest spending categories this month, largest first (at most 5).
    pub top_spending: Vec<CategorySpend>,
    /// From the budget: budgeted / spent / remaining for budgeted categories.
    pub budgeted: Money,
    pub budget_spent: Money,
    pub budget_remaining: Money,
    /// Latest transactions across all accounts.
    pub recent: Vec<Transaction>,
}

fn overflow() -> DbError {
    invalid("amount", "The totals are too large to calculate.")
}

/// Net worth from every account's current balance (archived included, so
/// closing an account with money in it doesn't make it vanish).
pub fn net_worth(db: &Database) -> Result<NetWorth, DbError> {
    let (mut assets, mut debts) = (Money::ZERO, Money::ZERO);
    for b in accounts::balances(db)?.into_values() {
        if b.is_negative() {
            debts = debts.checked_add(b).ok_or_else(overflow)?;
        } else {
            assets = assets.checked_add(b).ok_or_else(overflow)?;
        }
    }
    Ok(NetWorth {
        assets,
        debts,
        net: assets.checked_add(debts).ok_or_else(overflow)?,
    })
}

/// Income and spending between two dates (`from` inclusive, `to` exclusive).
pub fn totals(db: &Database, from: NaiveDate, to: NaiveDate) -> Result<MonthTotals, DbError> {
    let (inc, out): (i64, i64) = db.conn().query_row(
        "SELECT coalesce(sum(CASE WHEN amount_cents > 0 THEN amount_cents END), 0), \
                coalesce(-sum(CASE WHEN amount_cents < 0 THEN amount_cents END), 0) \
         FROM transactions WHERE transfer_id IS NULL AND date >= ?1 AND date < ?2",
        params![from, to],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let (income, spending) = (Money::from_cents(inc), Money::from_cents(out));
    Ok(MonthTotals {
        income,
        spending,
        net: income.checked_sub(spending).ok_or_else(overflow)?,
    })
}

/// Net spending per expense category (money out minus refunds), plus
/// uncategorised money out, largest first; nothing-spent entries are left
/// out. Uncategorised counts only money out, because uncategorised money in
/// (e.g. pay not yet categorised) isn't a refund.
pub fn spending_by_category(
    db: &Database,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<CategorySpend>, DbError> {
    let mut stmt = db.conn().prepare(
        "SELECT t.category_id, -sum(t.amount_cents) AS spent FROM transactions t \
         JOIN categories c ON c.id = t.category_id \
         JOIN category_groups g ON g.id = c.group_id \
         WHERE t.transfer_id IS NULL AND t.date >= ?1 AND t.date < ?2 AND g.kind = 'expense' \
         GROUP BY t.category_id HAVING spent > 0",
    )?;
    let mut out: Vec<CategorySpend> = stmt
        .query_map(params![from, to], |r| {
            Ok(CategorySpend {
                category_id: Some(r.get(0)?),
                spent: Money::from_cents(r.get(1)?),
            })
        })?
        .collect::<Result<_, _>>()?;
    let uncategorised: i64 = db.conn().query_row(
        "SELECT coalesce(-sum(amount_cents), 0) FROM transactions \
         WHERE transfer_id IS NULL AND category_id IS NULL AND amount_cents < 0 \
           AND date >= ?1 AND date < ?2",
        params![from, to],
        |q| q.get(0),
    )?;
    if uncategorised > 0 {
        out.push(CategorySpend {
            category_id: None,
            spent: Money::from_cents(uncategorised),
        });
    }
    out.sort_by(|a, b| {
        b.spent
            .cmp(&a.spent)
            .then(a.category_id.cmp(&b.category_id))
    });
    Ok(out)
}

pub fn dashboard(db: &Database, month: Month) -> Result<Dashboard, DbError> {
    let (from, to) = (month.first_day(), month.next().first_day());
    let mut top = spending_by_category(db, from, to)?;
    top.truncate(5);
    let budget = budgets::month(db, month)?;
    Ok(Dashboard {
        month,
        net_worth: net_worth(db)?,
        totals: totals(db, from, to)?,
        top_spending: top,
        budgeted: budget.total_budgeted,
        budget_spent: budget.total_spent_budgeted,
        budget_remaining: budget.total_remaining,
        recent: transactions::list(
            db,
            &TransactionQuery {
                limit: Some(5),
                ..Default::default()
            },
        )?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::{AccountInput, AccountKind};
    use crate::categories;
    use crate::transactions::TransactionInput;
    use tempfile::TempDir;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn account(db: &Database, name: &str, kind: AccountKind, opening: i64) -> i64 {
        accounts::create(
            db,
            &AccountInput {
                name: name.into(),
                kind,
                institution: None,
                bsb: None,
                account_number: None,
                opening_balance: Money::from_cents(opening),
                opening_date: d("2026-01-01"),
                offset_for: None,
                notes: None,
            },
        )
        .unwrap()
        .id
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

    fn tx(db: &Database, a: i64, date: &str, cents: i64, category: Option<i64>) {
        transactions::add(
            db,
            &TransactionInput {
                account_id: a,
                date: d(date),
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

    fn setup() -> (TempDir, Database) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::create(&Database::path_in(dir.path()), "test password").unwrap();
        (dir, db)
    }

    #[test]
    fn net_worth_splits_assets_and_debts_and_keeps_archived() {
        let (_d, db) = setup();
        let offset = account(&db, "Offset", AccountKind::Offset, 1_000_000);
        account(&db, "Home loan", AccountKind::HomeLoan, -50_000_000);
        let old = account(&db, "Old saver", AccountKind::Savings, 20_000);
        accounts::set_archived(&db, old, true).unwrap();
        tx(&db, offset, "2026-11-05", -10_000, None);
        let nw = net_worth(&db).unwrap();
        assert_eq!(nw.assets, Money::from_cents(1_010_000));
        assert_eq!(nw.debts, Money::from_cents(-50_000_000));
        assert_eq!(nw.net, Money::from_cents(-48_990_000));
    }

    #[test]
    fn month_totals_and_top_spending_exclude_transfers() {
        let (_d, db) = setup();
        let a = account(&db, "Everyday", AccountKind::Offset, 0);
        let b = account(&db, "Bills", AccountKind::Offset, 0);
        let (g, fuel, salary) = (
            cat(&db, "Groceries"),
            cat(&db, "Fuel"),
            cat(&db, "Salary & wages"),
        );
        tx(&db, a, "2026-11-01", 300_000, Some(salary));
        tx(&db, a, "2026-11-02", 5_000, None); // uncategorised money in
        tx(&db, a, "2026-11-03", -20_000, Some(g));
        tx(&db, a, "2026-11-04", -15_000, Some(g));
        tx(&db, a, "2026-11-05", 2_000, Some(g)); // refund
        tx(&db, a, "2026-11-06", -6_000, Some(fuel));
        tx(&db, a, "2026-11-07", -1_000, None);
        tx(&db, a, "2026-10-31", -99_999, Some(g)); // other month
        crate::transfers::create(
            &db,
            &crate::transfers::TransferInput {
                from_account: a,
                to_account: b,
                date: d("2026-11-08"),
                amount: Money::from_cents(50_000),
                memo: None,
            },
        )
        .unwrap();

        let (from, to) = (d("2026-11-01"), d("2026-12-01"));
        let t = totals(&db, from, to).unwrap();
        assert_eq!(t.income, Money::from_cents(307_000));
        assert_eq!(t.spending, Money::from_cents(42_000));
        assert_eq!(t.net, Money::from_cents(265_000));

        let top = spending_by_category(&db, from, to).unwrap();
        assert_eq!(
            top.iter()
                .map(|c| (c.category_id, c.spent.cents()))
                .collect::<Vec<_>>(),
            [(Some(g), 33_000), (Some(fuel), 6_000), (None, 1_000)]
        );
    }

    #[test]
    fn dashboard_combines_everything() {
        let (_d, db) = setup();
        let a = account(&db, "Everyday", AccountKind::Offset, 100_000);
        let g = cat(&db, "Groceries");
        budgets::set(
            &db,
            g,
            "2026-11".parse().unwrap(),
            Money::from_cents(50_000),
        )
        .unwrap();
        for i in 1..=7 {
            tx(&db, a, &format!("2026-11-0{i}"), -1_000, Some(g));
        }
        let dash = dashboard(&db, "2026-11".parse().unwrap()).unwrap();
        assert_eq!(dash.net_worth.net, Money::from_cents(93_000));
        assert_eq!(dash.recent.len(), 5);
        assert_eq!(dash.recent[0].date, d("2026-11-07"));
        assert_eq!(
            (
                dash.budgeted.cents(),
                dash.budget_spent.cents(),
                dash.budget_remaining.cents()
            ),
            (50_000, 7_000, 43_000)
        );
        let v = serde_json::to_value(&dash).unwrap();
        assert_eq!(v["month"], "2026-11");
        assert_eq!(v["net_worth"]["assets"], 93_000);
        assert_eq!(v["top_spending"][0]["spent"], 7_000);
    }
}
