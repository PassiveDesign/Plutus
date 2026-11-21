//! Saved column mappings, so a bank only has to be mapped once.

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::generic::{self, Mapping};
use crate::db::{Database, DbError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preset {
    pub id: i64,
    pub name: String,
    pub mapping: Mapping,
}

/// Every saved preset, alphabetically.
pub fn list(db: &Database) -> Result<Vec<Preset>, DbError> {
    let mut stmt = db
        .conn()
        .prepare("SELECT id, name, mapping FROM import_presets ORDER BY name COLLATE NOCASE")?;
    let rows = stmt.query_map([], |row| {
        let id: i64 = row.get(0)?;
        let name: String = row.get(1)?;
        let json: String = row.get(2)?;
        Ok((id, name, json))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (id, name, json) = row?;
        let mapping: Mapping = serde_json::from_str(&json).map_err(|e| DbError::Invalid {
            field: "preset",
            message: format!("Saved preset \"{name}\" can't be read: {e}."),
        })?;
        out.push(Preset { id, name, mapping });
    }
    Ok(out)
}

/// Save a mapping under a name. Saving an existing name replaces its mapping.
pub fn save(db: &Database, name: &str, mapping: &Mapping) -> Result<Preset, DbError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(DbError::Invalid {
            field: "name",
            message: "Give the preset a name.".to_string(),
        });
    }
    generic::validate(mapping)?;
    let json = serde_json::to_string(mapping).map_err(|e| DbError::Invalid {
        field: "mapping",
        message: format!("Couldn't save the mapping: {e}."),
    })?;
    let id: i64 = db
        .conn()
        .query_row(
            "INSERT INTO import_presets (name, mapping) VALUES (?1, ?2)
             ON CONFLICT(name) DO UPDATE SET mapping = excluded.mapping,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
             RETURNING id",
            params![name, json],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| DbError::Invalid {
            field: "name",
            message: "Couldn't save the preset.".to_string(),
        })?;
    Ok(Preset {
        id,
        name: name.to_string(),
        mapping: mapping.clone(),
    })
}

/// Delete a preset. Missing ids are ignored, so a double click is harmless.
pub fn delete(db: &Database, id: i64) -> Result<(), DbError> {
    db.conn()
        .execute("DELETE FROM import_presets WHERE id = ?1", params![id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::generic::DateFormat;

    fn db() -> Database {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plutus.db");
        // Keep the directory alive for the life of the connection.
        std::mem::forget(dir);
        Database::create(&path, "correct horse battery").unwrap()
    }

    fn mapping() -> Mapping {
        Mapping {
            date: "Date".into(),
            date_format: DateFormat::Mdy,
            description: "Description".into(),
            payee: None,
            amount: Some("Amount".into()),
            debit: None,
            credit: None,
            balance: None,
        }
    }

    #[test]
    fn saves_lists_and_replaces_by_name() {
        let db = db();
        let saved = save(&db, "ANZ", &mapping()).unwrap();
        assert_eq!(saved.name, "ANZ");
        assert_eq!(list(&db).unwrap().len(), 1);

        let mut changed = mapping();
        changed.amount = Some("Value".into());
        save(&db, "anz", &changed).unwrap();
        let all = list(&db).unwrap();
        assert_eq!(all.len(), 1, "names are case-insensitive");
        assert_eq!(all[0].mapping.amount.as_deref(), Some("Value"));
    }

    #[test]
    fn deletes_a_preset() {
        let db = db();
        let saved = save(&db, "Westpac", &mapping()).unwrap();
        delete(&db, saved.id).unwrap();
        assert!(list(&db).unwrap().is_empty());
        // Deleting twice is fine.
        delete(&db, saved.id).unwrap();
    }

    #[test]
    fn rejects_an_empty_name() {
        let db = db();
        let err = save(&db, "   ", &mapping()).unwrap_err();
        assert_eq!(err.code(), "invalid");
        assert!(err.to_string().contains("name"), "{err}");
    }

    #[test]
    fn rejects_a_mapping_without_an_amount_column() {
        let db = db();
        let mut m = mapping();
        m.amount = None;
        let err = save(&db, "Broken", &m).unwrap_err();
        assert!(err.to_string().contains("amount"), "{err}");
    }
}
