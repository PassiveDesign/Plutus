//! Categories: a two-level tree. Groups (income or expense) contain
//! categories. Defaults are seeded by migration 0005. Both levels are
//! archived rather than deleted so past transactions keep their labels.

use std::fmt;

use rusqlite::{OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use crate::db::{Database, DbError};

pub const MAX_NAME_CHARS: usize = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupKind {
    Income,
    Expense,
}

impl GroupKind {
    pub fn as_str(self) -> &'static str {
        match self {
            GroupKind::Income => "income",
            GroupKind::Expense => "expense",
        }
    }

    pub fn parse(s: &str) -> Option<GroupKind> {
        match s {
            "income" => Some(GroupKind::Income),
            "expense" => Some(GroupKind::Expense),
            _ => None,
        }
    }
}

impl fmt::Display for GroupKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Category {
    pub id: i64,
    pub group_id: i64,
    pub name: String,
    pub sort_order: i64,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CategoryGroup {
    pub id: i64,
    pub name: String,
    pub kind: GroupKind,
    pub sort_order: i64,
    pub archived_at: Option<String>,
    /// In `sort_order`, then name.
    pub categories: Vec<Category>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupInput {
    pub name: String,
    pub kind: GroupKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryInput {
    pub group_id: i64,
    pub name: String,
}

fn invalid(field: &'static str, message: impl Into<String>) -> DbError {
    DbError::Invalid {
        field,
        message: message.into(),
    }
}

fn clean_name(name: &str) -> Result<String, DbError> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err(invalid("name", "Enter a name."));
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(invalid(
            "name",
            format!("Keep the name to {MAX_NAME_CHARS} characters or fewer."),
        ));
    }
    Ok(name)
}

const GROUP_SELECT: &str = "SELECT id, name, kind, sort_order, archived_at FROM category_groups";
const CATEGORY_SELECT: &str = "SELECT id, group_id, name, sort_order, archived_at FROM categories";

fn group_from_row(row: &Row<'_>) -> rusqlite::Result<CategoryGroup> {
    let kind: String = row.get(2)?;
    Ok(CategoryGroup {
        id: row.get(0)?,
        name: row.get(1)?,
        kind: GroupKind::parse(&kind).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                2,
                rusqlite::types::Type::Text,
                format!("unknown group kind {kind:?}").into(),
            )
        })?,
        sort_order: row.get(3)?,
        archived_at: row.get(4)?,
        categories: Vec::new(),
    })
}

fn category_from_row(row: &Row<'_>) -> rusqlite::Result<Category> {
    Ok(Category {
        id: row.get(0)?,
        group_id: row.get(1)?,
        name: row.get(2)?,
        sort_order: row.get(3)?,
        archived_at: row.get(4)?,
    })
}

fn group_row(db: &Database, id: i64) -> Result<CategoryGroup, DbError> {
    db.conn()
        .query_row(
            &format!("{GROUP_SELECT} WHERE id = ?1"),
            [id],
            group_from_row,
        )
        .optional()?
        .ok_or(DbError::RecordNotFound {
            what: "category group",
            id,
        })
}

/// A group with its categories (archived ones included).
pub fn get_group(db: &Database, id: i64) -> Result<CategoryGroup, DbError> {
    let mut g = group_row(db, id)?;
    let mut stmt = db.conn().prepare(&format!(
        "{CATEGORY_SELECT} WHERE group_id = ?1 ORDER BY sort_order, name COLLATE NOCASE, id"
    ))?;
    g.categories = stmt
        .query_map([id], category_from_row)?
        .collect::<Result<_, _>>()?;
    Ok(g)
}

pub fn get(db: &Database, id: i64) -> Result<Category, DbError> {
    db.conn()
        .query_row(
            &format!("{CATEGORY_SELECT} WHERE id = ?1"),
            [id],
            category_from_row,
        )
        .optional()?
        .ok_or(DbError::RecordNotFound {
            what: "category",
            id,
        })
}

/// The whole tree: income groups first, then expense groups, each in
/// `sort_order`. Without `include_archived`, archived groups and archived
/// categories are left out.
pub fn tree(db: &Database, include_archived: bool) -> Result<Vec<CategoryGroup>, DbError> {
    let active = if include_archived {
        ""
    } else {
        " WHERE archived_at IS NULL"
    };
    let mut stmt = db.conn().prepare(&format!(
        "{GROUP_SELECT}{active} \
         ORDER BY kind = 'expense', sort_order, name COLLATE NOCASE, id"
    ))?;
    let mut groups: Vec<CategoryGroup> = stmt
        .query_map([], group_from_row)?
        .collect::<Result<_, _>>()?;

    let mut stmt = db.conn().prepare(&format!(
        "{CATEGORY_SELECT}{active} ORDER BY sort_order, name COLLATE NOCASE, id"
    ))?;
    let cats = stmt
        .query_map([], category_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    for c in cats {
        if let Some(g) = groups.iter_mut().find(|g| g.id == c.group_id) {
            g.categories.push(c);
        }
    }
    Ok(groups)
}

fn ensure_group_name_free(db: &Database, name: &str, self_id: Option<i64>) -> Result<(), DbError> {
    let taken: Option<i64> = db
        .conn()
        .query_row(
            "SELECT id FROM category_groups WHERE archived_at IS NULL \
             AND name = ?1 COLLATE NOCASE AND id IS NOT ?2",
            params![name, self_id],
            |r| r.get(0),
        )
        .optional()?;
    match taken {
        Some(_) => Err(DbError::Conflict {
            field: "name",
            message: format!("A group named \"{name}\" already exists."),
        }),
        None => Ok(()),
    }
}

fn ensure_category_name_free(
    db: &Database,
    group_id: i64,
    name: &str,
    self_id: Option<i64>,
) -> Result<(), DbError> {
    let taken: Option<i64> = db
        .conn()
        .query_row(
            "SELECT id FROM categories WHERE archived_at IS NULL AND group_id = ?1 \
             AND name = ?2 COLLATE NOCASE AND id IS NOT ?3",
            params![group_id, name, self_id],
            |r| r.get(0),
        )
        .optional()?;
    match taken {
        Some(_) => Err(DbError::Conflict {
            field: "name",
            message: format!("This group already has a category named \"{name}\"."),
        }),
        None => Ok(()),
    }
}

fn next_order(db: &Database, sql: &str, arg: Option<i64>) -> Result<i64, DbError> {
    Ok(db
        .conn()
        .query_row(sql, params![arg], |r| r.get::<_, i64>(0))?)
}

pub fn create_group(db: &Database, input: &GroupInput) -> Result<CategoryGroup, DbError> {
    let name = clean_name(&input.name)?;
    ensure_group_name_free(db, &name, None)?;
    let order = next_order(
        db,
        "SELECT coalesce(max(sort_order), 0) + 1 FROM category_groups WHERE ?1 IS NULL",
        None,
    )?;
    db.conn().execute(
        "INSERT INTO category_groups (name, kind, sort_order) VALUES (?1, ?2, ?3)",
        params![name, input.kind.as_str(), order],
    )?;
    get_group(db, db.conn().last_insert_rowid())
}

/// Rename a group or change its kind.
pub fn update_group(db: &Database, id: i64, input: &GroupInput) -> Result<CategoryGroup, DbError> {
    let existing = group_row(db, id)?;
    let name = clean_name(&input.name)?;
    if existing.archived_at.is_none() {
        ensure_group_name_free(db, &name, Some(id))?;
    }
    db.conn().execute(
        "UPDATE category_groups SET name = ?1, kind = ?2 WHERE id = ?3",
        params![name, input.kind.as_str(), id],
    )?;
    get_group(db, id)
}

/// Hide or restore a group. Its categories keep their own archived flags;
/// an archived group hides them all from the tree.
pub fn set_group_archived(
    db: &Database,
    id: i64,
    archived: bool,
) -> Result<CategoryGroup, DbError> {
    let g = group_row(db, id)?;
    if archived == g.archived_at.is_some() {
        return get_group(db, id);
    }
    if archived {
        db.conn().execute(
            "UPDATE category_groups SET archived_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
             WHERE id = ?1",
            [id],
        )?;
    } else {
        ensure_group_name_free(db, &g.name, Some(id))?;
        db.conn().execute(
            "UPDATE category_groups SET archived_at = NULL WHERE id = ?1",
            [id],
        )?;
    }
    get_group(db, id)
}

fn active_group(db: &Database, group_id: i64) -> Result<CategoryGroup, DbError> {
    let g = group_row(db, group_id).map_err(|e| match e {
        DbError::RecordNotFound { .. } => invalid("group_id", "Choose a group."),
        other => other,
    })?;
    if g.archived_at.is_some() {
        return Err(invalid(
            "group_id",
            format!(
                "{} is archived. Restore it first or choose another group.",
                g.name
            ),
        ));
    }
    Ok(g)
}

pub fn create(db: &Database, input: &CategoryInput) -> Result<Category, DbError> {
    let name = clean_name(&input.name)?;
    active_group(db, input.group_id)?;
    ensure_category_name_free(db, input.group_id, &name, None)?;
    let order = next_order(
        db,
        "SELECT coalesce(max(sort_order), 0) + 1 FROM categories WHERE group_id = ?1",
        Some(input.group_id),
    )?;
    db.conn().execute(
        "INSERT INTO categories (group_id, name, sort_order) VALUES (?1, ?2, ?3)",
        params![input.group_id, name, order],
    )?;
    get(db, db.conn().last_insert_rowid())
}

/// Rename a category or move it to another (active) group; a moved
/// category goes to the end of its new group.
pub fn update(db: &Database, id: i64, input: &CategoryInput) -> Result<Category, DbError> {
    let existing = get(db, id)?;
    let name = clean_name(&input.name)?;
    let moving = input.group_id != existing.group_id;
    if moving {
        active_group(db, input.group_id)?;
    }
    if existing.archived_at.is_none() {
        ensure_category_name_free(db, input.group_id, &name, Some(id))?;
    }
    let order = if moving {
        next_order(
            db,
            "SELECT coalesce(max(sort_order), 0) + 1 FROM categories WHERE group_id = ?1",
            Some(input.group_id),
        )?
    } else {
        existing.sort_order
    };
    db.conn().execute(
        "UPDATE categories SET group_id = ?1, name = ?2, sort_order = ?3 WHERE id = ?4",
        params![input.group_id, name, order, id],
    )?;
    get(db, id)
}

/// Hide or restore a category. Transactions keep it either way.
pub fn set_archived(db: &Database, id: i64, archived: bool) -> Result<Category, DbError> {
    let c = get(db, id)?;
    if archived == c.archived_at.is_some() {
        return Ok(c);
    }
    if archived {
        db.conn().execute(
            "UPDATE categories SET archived_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
             WHERE id = ?1",
            [id],
        )?;
    } else {
        ensure_category_name_free(db, c.group_id, &c.name, Some(id))?;
        db.conn().execute(
            "UPDATE categories SET archived_at = NULL WHERE id = ?1",
            [id],
        )?;
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

    fn field_of(e: DbError) -> (&'static str, bool) {
        match e {
            DbError::Invalid { field, .. } => (field, false),
            DbError::Conflict { field, .. } => (field, true),
            other => panic!("expected a field error, got {other:?}"),
        }
    }

    fn group_named<'a>(t: &'a [CategoryGroup], name: &str) -> &'a CategoryGroup {
        t.iter().find(|g| g.name == name).unwrap()
    }

    #[test]
    fn defaults_are_seeded_income_first() {
        let (_d, db) = db();
        let t = tree(&db, false).unwrap();
        assert_eq!(t[0].name, "Income");
        assert_eq!(t[0].kind, GroupKind::Income);
        assert!(t[1..].iter().all(|g| g.kind == GroupKind::Expense));
        assert_eq!(t.len(), 11);
        assert_eq!(t.iter().map(|g| g.categories.len()).sum::<usize>(), 47);
        let food = group_named(&t, "Food");
        assert_eq!(
            food.categories
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            ["Groceries", "Eating out", "Takeaway & coffee"]
        );
        assert!(
            t.iter().all(|g| !g.categories.is_empty()),
            "no empty default group"
        );
    }

    #[test]
    fn create_appends_and_trims() {
        let (_d, db) = db();
        let g = create_group(
            &db,
            &GroupInput {
                name: "  Side   hustle ".into(),
                kind: GroupKind::Income,
            },
        )
        .unwrap();
        assert_eq!(g.name, "Side hustle");
        assert_eq!(g.sort_order, 12);
        let c = create(
            &db,
            &CategoryInput {
                group_id: g.id,
                name: "Etsy".into(),
            },
        )
        .unwrap();
        assert_eq!((c.group_id, c.sort_order), (g.id, 1));
        let c2 = create(
            &db,
            &CategoryInput {
                group_id: g.id,
                name: "Markets".into(),
            },
        )
        .unwrap();
        assert_eq!(c2.sort_order, 2);
        // Income groups sort before expense groups regardless of order.
        let t = tree(&db, false).unwrap();
        assert_eq!(
            t.iter()
                .take(2)
                .map(|g| g.name.as_str())
                .collect::<Vec<_>>(),
            ["Income", "Side hustle"]
        );
        assert_eq!(get_group(&db, g.id).unwrap().categories, [c, c2]);
    }

    #[test]
    fn validation_and_name_clashes() {
        let (_d, db) = db();
        let t = tree(&db, false).unwrap();
        let food = group_named(&t, "Food").id;
        let home = group_named(&t, "Home").id;
        let cat = |group_id, name: &str| CategoryInput {
            group_id,
            name: name.into(),
        };
        assert_eq!(
            field_of(create(&db, &cat(food, "  ")).unwrap_err()),
            ("name", false)
        );
        assert_eq!(
            field_of(create(&db, &cat(food, &"x".repeat(61))).unwrap_err()),
            ("name", false)
        );
        assert_eq!(
            field_of(create(&db, &cat(999, "X")).unwrap_err()),
            ("group_id", false)
        );
        assert_eq!(
            field_of(create(&db, &cat(food, "groceries")).unwrap_err()),
            ("name", true)
        );
        // Same name in a different group is fine.
        create(&db, &cat(home, "Groceries")).unwrap();
        let g = |name: &str| GroupInput {
            name: name.into(),
            kind: GroupKind::Expense,
        };
        assert_eq!(
            field_of(create_group(&db, &g("FOOD")).unwrap_err()),
            ("name", true)
        );
        assert_eq!(
            field_of(create_group(&db, &g("")).unwrap_err()),
            ("name", false)
        );
        // Renaming to its own name (different case) is fine.
        assert_eq!(update_group(&db, food, &g("food")).unwrap().name, "food");
    }

    #[test]
    fn archive_hides_and_frees_names() {
        let (_d, db) = db();
        let t = tree(&db, false).unwrap();
        let food = group_named(&t, "Food");
        let groceries = food.categories[0].id;

        set_archived(&db, groceries, true).unwrap();
        let t = tree(&db, false).unwrap();
        assert_eq!(group_named(&t, "Food").categories.len(), 2);
        let all = tree(&db, true).unwrap();
        assert_eq!(group_named(&all, "Food").categories.len(), 3);

        // Name is free while archived; restoring then clashes.
        let again = create(
            &db,
            &CategoryInput {
                group_id: food.id,
                name: "Groceries".into(),
            },
        )
        .unwrap();
        assert_eq!(
            field_of(set_archived(&db, groceries, false).unwrap_err()),
            ("name", true)
        );
        set_archived(&db, again.id, true).unwrap();
        assert!(
            set_archived(&db, groceries, false)
                .unwrap()
                .archived_at
                .is_none()
        );

        // Archived groups vanish with their categories and take no new ones.
        set_group_archived(&db, food.id, true).unwrap();
        assert!(tree(&db, false).unwrap().iter().all(|g| g.name != "Food"));
        let e = create(
            &db,
            &CategoryInput {
                group_id: food.id,
                name: "Snacks".into(),
            },
        )
        .unwrap_err();
        assert_eq!(field_of(e), ("group_id", false));
        let restored = set_group_archived(&db, food.id, false).unwrap();
        assert!(restored.archived_at.is_none());
        assert_eq!(
            tree(&db, false)
                .unwrap()
                .iter()
                .filter(|g| g.name == "Food")
                .count(),
            1
        );
    }

    #[test]
    fn move_category_between_groups() {
        let (_d, db) = db();
        let t = tree(&db, false).unwrap();
        let fitness = group_named(&t, "Health").categories[4].clone();
        assert_eq!(fitness.name, "Fitness");
        let lifestyle = group_named(&t, "Lifestyle");
        let moved = update(
            &db,
            fitness.id,
            &CategoryInput {
                group_id: lifestyle.id,
                name: "Gym".into(),
            },
        )
        .unwrap();
        assert_eq!((moved.group_id, moved.name.as_str()), (lifestyle.id, "Gym"));
        assert_eq!(moved.sort_order, 5, "goes to the end of the new group");
        assert!(matches!(
            update(
                &db,
                9999,
                &CategoryInput {
                    group_id: lifestyle.id,
                    name: "x".into()
                }
            ),
            Err(DbError::RecordNotFound {
                what: "category",
                ..
            })
        ));
    }

    #[test]
    fn transactions_can_reference_categories() {
        // Schema check: category_id exists and is a real foreign key.
        let (_d, db) = db();
        let fk: i64 = db
            .conn()
            .query_row(
                "SELECT count(*) FROM pragma_foreign_key_list('transactions') \
                 WHERE \"table\" = 'categories' AND \"from\" = 'category_id'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fk, 1);
    }

    #[test]
    fn json_shape_for_the_ui() {
        let (_d, db) = db();
        let t = tree(&db, false).unwrap();
        let v = serde_json::to_value(&t[0]).unwrap();
        assert_eq!(v["kind"], "income");
        assert_eq!(v["categories"][0]["name"], "Salary & wages");
        assert!(v["categories"][0]["archived_at"].is_null());
        let i: GroupInput =
            serde_json::from_value(serde_json::json!({"name": "X", "kind": "expense"})).unwrap();
        assert_eq!(i.kind, GroupKind::Expense);
    }
}
