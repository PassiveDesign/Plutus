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
    /// The password of the open database, kept only while unlocked so the app
    /// can close and reopen the file for a backup or a restore without asking
    /// the owner to type it again. Cleared on `lock`.
    password: Option<String>,
}

impl Vault {
    /// A locked vault for the database file at `path` (nothing is opened yet).
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            db: None,
            password: None,
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
        self.password = Some(password.to_string());
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
        self.password = Some(password.to_string());
        Ok(())
    }

    /// Close the database. Safe to call when already locked.
    pub fn lock(&mut self) {
        self.db = None;
        self.password = None;
    }

    /// Change the password. Requires the vault to be unlocked and the current
    /// password to be re-entered (so an unattended unlocked app can't be
    /// re-keyed by someone else). The new password must meet the policy.
    pub fn change_password(&mut self, current: &str, new: &str) -> Result<(), DbError> {
        let db = self.db.as_ref().ok_or(DbError::Locked)?;
        if !Database::verify_password(db.path(), current)? {
            return Err(DbError::WrongPassword);
        }
        check_new_password(new)?;
        db.change_password(new)?;
        self.password = Some(new.to_string());
        Ok(())
    }

    /// Copy the database file to `dest`, reopening the vault afterwards.
    ///
    /// The connection is closed first so the WAL is checkpointed and the file
    /// on disk is a complete, self-contained snapshot. Returns the number of
    /// bytes written.
    pub fn copy_to(&mut self, dest: &Path) -> Result<u64, DbError> {
        let password = self.password.clone().ok_or(DbError::Locked)?;
        let db = self.db.take().ok_or(DbError::Locked)?;
        drop(db);
        let copied = std::fs::copy(&self.path, dest);
        match (copied, Database::open(&self.path, &password)) {
            (Ok(bytes), Ok(db)) => {
                self.db = Some(db);
                Ok(bytes)
            }
            (Err(e), Ok(db)) => {
                self.db = Some(db);
                Err(DbError::Io(e))
            }
            (_, Err(e)) => {
                self.password = None;
                Err(e)
            }
        }
    }

    /// Replace the database file with `src` (a backup), reopening afterwards.
    ///
    /// The candidate is first copied next to the database and opened there, so
    /// a wrong password or a foreign file is rejected without touching the
    /// current data. The old file is kept beside the database as
    /// `<db>.before-restore` and restored if the swap fails.
    pub fn replace_with(&mut self, src: &Path) -> Result<(), DbError> {
        let password = self.password.clone().ok_or(DbError::Locked)?;
        if !src.is_file() {
            return Err(DbError::NotFound(src.to_path_buf()));
        }
        let staging = self.path.with_extension("restore-new");
        let _ = std::fs::remove_file(&staging);
        std::fs::copy(src, &staging)?;
        if let Err(e) = Database::open(&staging, &password) {
            let _ = std::fs::remove_file(&staging);
            return Err(e);
        }
        let old = self.path.with_extension("before-restore");
        let _ = std::fs::remove_file(&old);
        self.db = None;
        if let Err(e) = std::fs::rename(&self.path, &old) {
            let _ = std::fs::remove_file(&staging);
            self.db = Database::open(&self.path, &password).ok();
            return Err(DbError::Io(e));
        }
        remove_sidecars(&self.path);
        if let Err(e) = std::fs::rename(&staging, &self.path) {
            let _ = std::fs::rename(&old, &self.path);
            self.db = Database::open(&self.path, &password).ok();
            return Err(DbError::Io(e));
        }
        match Database::open(&self.path, &password) {
            Ok(db) => {
                self.db = Some(db);
                Ok(())
            }
            Err(e) => {
                let _ = std::fs::remove_file(&self.path);
                remove_sidecars(&self.path);
                let _ = std::fs::rename(&old, &self.path);
                self.db = Database::open(&self.path, &password).ok();
                Err(e)
            }
        }
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

/// Remove the WAL and shared-memory files that sit beside a database file.
fn remove_sidecars(path: &Path) {
    for suffix in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
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
    fn change_password_full_cycle() {
        let (_d, mut v) = vault();
        v.create("first password").unwrap();
        v.change_password("first password", "second password")
            .unwrap();
        assert_eq!(
            v.status(),
            VaultStatus::Unlocked,
            "stays unlocked after change"
        );
        v.lock();
        assert!(matches!(
            v.unlock("first password"),
            Err(DbError::WrongPassword)
        ));
        v.unlock("second password").unwrap();
    }

    #[test]
    fn change_password_rejects_wrong_current_and_keeps_old() {
        let (_d, mut v) = vault();
        v.create("first password").unwrap();
        assert!(matches!(
            v.change_password("not it at all", "second password"),
            Err(DbError::WrongPassword)
        ));
        v.lock();
        v.unlock("first password").unwrap();
    }

    #[test]
    fn change_password_enforces_policy_on_new() {
        let (_d, mut v) = vault();
        v.create("first password").unwrap();
        assert!(matches!(
            v.change_password("first password", "short"),
            Err(DbError::PasswordTooShort { .. })
        ));
        v.lock();
        v.unlock("first password").unwrap();
    }

    #[test]
    fn change_password_requires_unlocked() {
        let (_d, mut v) = vault();
        v.create("first password").unwrap();
        v.lock();
        assert!(matches!(
            v.change_password("first password", "second password"),
            Err(DbError::Locked)
        ));
    }

    #[test]
    fn status_strings_are_stable() {
        assert_eq!(VaultStatus::New.as_str(), "new");
        assert_eq!(VaultStatus::Locked.as_str(), "locked");
        assert_eq!(VaultStatus::Unlocked.as_str(), "unlocked");
    }
}
