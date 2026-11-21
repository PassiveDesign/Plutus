//! Backups of the encrypted database file.
//!
//! A backup is a byte-for-byte copy of the SQLCipher file, so it stays
//! encrypted under the same password and opens on any machine. Copies are kept
//! in a `backups/` folder beside the database and named by timestamp, so they
//! sort newest-first as plain strings.
//!
//! Restoring replaces the live database with a copy. The current file is kept
//! as `plutus.db.before-restore` so an unwanted restore can be undone by hand.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, SecondsFormat};
use serde::Serialize;

use crate::db::DbError;
use crate::vault::Vault;

/// Folder (inside the app data directory) holding timestamped copies.
pub const BACKUP_DIR: &str = "backups";

/// How many timestamped copies to keep; older ones are deleted after a backup.
pub const KEEP_BACKUPS: usize = 10;

/// A copy of the database file on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BackupFile {
    /// File name only, e.g. `plutus-20261121-093000.db`.
    pub name: String,
    /// Absolute path, for display and for the "open folder" hint.
    pub path: String,
    pub bytes: u64,
    /// Last-modified time, RFC 3339 in local time.
    pub modified: String,
}

/// The backup folder for an app data directory.
pub fn dir_in(data_dir: &Path) -> PathBuf {
    data_dir.join(BACKUP_DIR)
}

/// Copy the database into the backup folder and prune old copies.
///
/// `now` is passed in so tests (and the command layer) control the file name.
pub fn backup_now(
    vault: &mut Vault,
    data_dir: &Path,
    now: DateTime<Local>,
) -> Result<BackupFile, DbError> {
    let dir = dir_in(data_dir);
    std::fs::create_dir_all(&dir)?;
    let name = format!("plutus-{}.db", now.format("%Y%m%d-%H%M%S"));
    let dest = dir.join(&name);
    let bytes = vault.copy_to(&dest)?;
    prune(&dir, KEEP_BACKUPS)?;
    Ok(BackupFile {
        name,
        path: dest.display().to_string(),
        bytes,
        modified: now.to_rfc3339_opts(SecondsFormat::Secs, true),
    })
}

/// Every backup in the folder, newest first. Missing folder means no backups.
pub fn list(data_dir: &Path) -> Result<Vec<BackupFile>, DbError> {
    let dir = dir_in(data_dir);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("db") {
            continue;
        }
        let meta = entry.metadata()?;
        if !meta.is_file() {
            continue;
        }
        let modified = meta
            .modified()
            .map(|t| {
                let local: DateTime<Local> = t.into();
                local.to_rfc3339_opts(SecondsFormat::Secs, true)
            })
            .unwrap_or_default();
        out.push(BackupFile {
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path: path.display().to_string(),
            bytes: meta.len(),
            modified,
        });
    }
    out.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(out)
}

/// Delete all but the newest `keep` copies. Returns how many were removed.
pub fn prune(dir: &Path, keep: usize) -> Result<usize, DbError> {
    let mut names: Vec<PathBuf> = Vec::new();
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) == Some("db") {
                names.push(path);
            }
        }
    }
    names.sort();
    let mut removed = 0;
    while names.len() > keep {
        let oldest = names.remove(0);
        std::fs::remove_file(&oldest)?;
        removed += 1;
    }
    Ok(removed)
}

/// Replace the live database with the named copy from the backup folder.
///
/// The name must be a plain file name: paths and `..` are rejected so the UI
/// can only reach the folder Plutus itself manages.
pub fn restore(vault: &mut Vault, data_dir: &Path, name: &str) -> Result<(), DbError> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || name.starts_with('.')
    {
        return Err(DbError::Invalid {
            field: "file",
            message: "Choose a backup from the list.".to_string(),
        });
    }
    let src = dir_in(data_dir).join(name);
    if !src.is_file() {
        return Err(DbError::NotFound(src));
    }
    vault.replace_with(&src)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::{self, AccountInput};
    use crate::db::Database;
    use chrono::TimeZone;
    use tempfile::TempDir;

    const PW: &str = "long enough pw";

    fn setup() -> (TempDir, Vault) {
        let dir = tempfile::tempdir().unwrap();
        let mut vault = Vault::new(Database::path_in(dir.path()));
        vault.create(PW).unwrap();
        (dir, vault)
    }

    fn when(day: u32, hour: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(2026, 11, day, hour, 0, 0)
            .single()
            .unwrap()
    }

    fn add_account(vault: &mut Vault, name: &str) {
        let db = vault.db().unwrap();
        accounts::create(
            db,
            &AccountInput {
                name: name.to_string(),
                kind: accounts::AccountKind::Transaction,
                institution: None,
                opening_balance: crate::money::Money::from_cents(1_000),
                opening_date: chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(),
                bsb: None,
                account_number: None,
                notes: None,
                offset_for: None,
            },
        )
        .unwrap();
    }

    #[test]
    fn backup_writes_an_encrypted_copy() {
        let (dir, mut vault) = setup();
        add_account(&mut vault, "Everyday");
        let file = backup_now(&mut vault, dir.path(), when(21, 9)).unwrap();
        assert_eq!(file.name, "plutus-20261121-090000.db");
        assert!(file.bytes > 0);
        let bytes = std::fs::read(&file.path).unwrap();
        assert!(
            !bytes.starts_with(b"SQLite format 3"),
            "the copy must stay encrypted"
        );
        // The vault is still usable after the backup.
        assert_eq!(vault.db().unwrap().schema_version().unwrap(), 9);
    }

    #[test]
    fn backup_is_listed_newest_first() {
        let (dir, mut vault) = setup();
        backup_now(&mut vault, dir.path(), when(20, 9)).unwrap();
        backup_now(&mut vault, dir.path(), when(21, 9)).unwrap();
        let files = list(dir.path()).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].name, "plutus-20261121-090000.db");
        assert_eq!(files[1].name, "plutus-20261120-090000.db");
    }

    #[test]
    fn list_is_empty_without_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        assert!(list(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn prune_keeps_the_newest() {
        let (dir, mut vault) = setup();
        for day in 1..=12 {
            backup_now(&mut vault, dir.path(), when(day, 9)).unwrap();
        }
        let files = list(dir.path()).unwrap();
        assert_eq!(files.len(), KEEP_BACKUPS);
        assert_eq!(files[0].name, "plutus-20261112-090000.db");
        assert_eq!(files[KEEP_BACKUPS - 1].name, "plutus-20261103-090000.db");
    }

    #[test]
    fn restore_brings_back_older_data() {
        let (dir, mut vault) = setup();
        add_account(&mut vault, "Before backup");
        let file = backup_now(&mut vault, dir.path(), when(21, 9)).unwrap();
        add_account(&mut vault, "After backup");
        let names = |v: &Vault| {
            accounts::list(v.db().unwrap(), false)
                .unwrap()
                .into_iter()
                .map(|a| a.name)
                .collect::<Vec<_>>()
        };
        let sorted = |v: &Vault| {
            let mut names = names(v);
            names.sort();
            names
        };
        assert_eq!(sorted(&vault), ["After backup", "Before backup"]);

        restore(&mut vault, dir.path(), &file.name).unwrap();
        assert_eq!(sorted(&vault), ["Before backup"]);
        // The vault stays open, and the pre-restore file is kept for recovery.
        assert_eq!(vault.status(), crate::vault::VaultStatus::Unlocked);
        assert!(
            Database::path_in(dir.path())
                .with_extension("before-restore")
                .is_file()
        );
    }

    #[test]
    fn restore_rejects_a_foreign_file() {
        let (dir, mut vault) = setup();
        let junk = dir_in(dir.path());
        std::fs::create_dir_all(&junk).unwrap();
        std::fs::write(junk.join("plutus-20260101-000000.db"), b"not a database").unwrap();
        let err = restore(&mut vault, dir.path(), "plutus-20260101-000000.db").unwrap_err();
        assert!(matches!(err, DbError::WrongPassword | DbError::Sqlite(_)));
        // The live database is untouched.
        assert_eq!(vault.status(), crate::vault::VaultStatus::Unlocked);
        assert!(vault.db().is_ok());
    }

    #[test]
    fn restore_refuses_paths_outside_the_folder() {
        let (dir, mut vault) = setup();
        for name in ["../plutus.db", "sub/x.db", "..", ""] {
            let err = restore(&mut vault, dir.path(), name).unwrap_err();
            assert!(matches!(err, DbError::Invalid { .. }), "{name} was allowed");
        }
    }

    #[test]
    fn restore_requires_an_unlocked_vault() {
        let (dir, mut vault) = setup();
        let file = backup_now(&mut vault, dir.path(), when(21, 9)).unwrap();
        vault.lock();
        assert!(matches!(
            restore(&mut vault, dir.path(), &file.name),
            Err(DbError::Locked)
        ));
    }
}
