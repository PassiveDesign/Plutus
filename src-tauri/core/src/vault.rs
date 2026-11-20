//! The vault: the app-level lock around the encrypted database.
//!
//! State machine:
//! - `New`      — no database file yet (first run). Only `create` is allowed.
//! - `Locked`   — a database file exists but is not open. Only `unlock` is allowed.
//! - `Unlocked` — the database is open and usable. `lock` closes it.
//!
//! Password policy for *new* passwords lives here (the database layer only
//! rejects empty passwords), so existing databases always stay openable.

use std::path::{Path, PathBuf};

use crate::db::{Database, DbError};

/// Minimum length, in characters, for a newly chosen password.
pub const MIN_PASSWORD_CHARS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultStatus {
    New,
    Locked,
    Unlocked,
}

impl VaultStatus {
    /// Stable identifier used across the UI boundary.
    pub fn as_str(self) -> &'static str {
        match self {
            VaultStatus::New => "new",
            VaultStatus::Locked => "locked",
            VaultStatus::Unlocked => "unlocked",
        }
    }
}

#[derive(Debug)]
pub struct Vault {
    path: PathBuf,
    db: Option<Database>,
}

impl Vault {
    /// A locked vault for the database file at `path` (nothing is opened yet).
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            db: None,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn status(&self) -> VaultStatus {
        if self.db.is_some() {
            VaultStatus::Unlocked
        } else if Database::exists(&self.path) {
            VaultStatus::Locked
        } else {
            VaultStatus::New
        }
    }

    /// First run: create the encrypted database and leave the vault unlocked.
    pub fn create(&mut self, password: &str) -> Result<(), DbError> {
        check_new_password(password)?;
        let db = Database::create(&self.path, password)?;
        self.db = Some(db);
        Ok(())
    }

    /// Open the existing database. Unlocking an already-unlocked vault is a
    /// no-op.
    pub fn unlock(&mut self, password: &str) -> Result<(), DbError> {
        if self.db.is_some() {
            return Ok(());
        }
        let db = Database::open(&self.path, password)?;
        self.db = Some(db);
        Ok(())
    }

    /// Close the database. Safe to call when already locked.
    pub fn lock(&mut self) {
        self.db = None;
    }

    /// The open database, or [`DbError::Locked`].
    pub fn db(&self) -> Result<&Database, DbError> {
        self.db.as_ref().ok_or(DbError::Locked)
    }
}

/// Validate a password the user is choosing (create / change password).
pub fn check_new_password(password: &str) -> Result<(), DbError> {
    if password.chars().count() < MIN_PASSWORD_CHARS {
        Err(DbError::PasswordTooShort {
            min: MIN_PASSWORD_CHARS,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn vault() -> (TempDir, Vault) {
        let dir = tempfile::tempdir().unwrap();
        let v = Vault::new(Database::path_in(dir.path()));
        (dir, v)
    }

    #[test]
    fn full_lifecycle() {
        let (_d, mut v) = vault();
        assert_eq!(v.status(), VaultStatus::New);
        assert!(matches!(v.db(), Err(DbError::Locked)));

        v.create("long enough pw").unwrap();
        assert_eq!(v.status(), VaultStatus::Unlocked);
        assert!(v.db().is_ok());

        v.lock();
        assert_eq!(v.status(), VaultStatus::Locked);
        assert!(matches!(v.db(), Err(DbError::Locked)));

        v.unlock("long enough pw").unwrap();
        assert_eq!(v.status(), VaultStatus::Unlocked);
    }

    #[test]
    fn wrong_password_keeps_vault_locked() {
        let (_d, mut v) = vault();
        v.create("long enough pw").unwrap();
        v.lock();
        assert!(matches!(v.unlock("nope nope"), Err(DbError::WrongPassword)));
        assert_eq!(v.status(), VaultStatus::Locked);
    }

    #[test]
    fn short_password_rejected_on_create_without_creating_file() {
        let (_d, mut v) = vault();
        let err = v.create("1234567").unwrap_err();
        assert!(matches!(err, DbError::PasswordTooShort { min: 8 }));
        assert_eq!(v.status(), VaultStatus::New);
        assert!(!v.path().exists());
    }

    #[test]
    fn length_counts_characters_not_bytes() {
        // 8 multi-byte characters is long enough; 7 is not.
        assert!(check_new_password("ééééééé").is_err());
        assert!(check_new_password("éééééééé").is_ok());
    }

    #[test]
    fn create_twice_fails_and_keeps_original() {
        let (_d, mut v) = vault();
        v.create("first password").unwrap();
        v.lock();
        assert!(matches!(
            v.create("second password"),
            Err(DbError::AlreadyExists(_))
        ));
        v.unlock("first password").unwrap();
    }

    #[test]
    fn unlock_when_unlocked_is_noop_and_lock_is_idempotent() {
        let (_d, mut v) = vault();
        v.create("long enough pw").unwrap();
        v.unlock("anything at all").unwrap();
        assert_eq!(v.status(), VaultStatus::Unlocked);
        v.lock();
        v.lock();
        assert_eq!(v.status(), VaultStatus::Locked);
    }

    #[test]
    fn unlock_on_first_run_is_not_found() {
        let (_d, mut v) = vault();
        assert!(matches!(v.unlock("whatever pw"), Err(DbError::NotFound(_))));
        assert_eq!(v.status(), VaultStatus::New);
    }

    #[test]
    fn status_strings_are_stable() {
        assert_eq!(VaultStatus::New.as_str(), "new");
        assert_eq!(VaultStatus::Locked.as_str(), "locked");
        assert_eq!(VaultStatus::Unlocked.as_str(), "unlocked");
    }
}
