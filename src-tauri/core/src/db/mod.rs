//! Encrypted database access.
//!
//! The whole database file is encrypted with SQLCipher. The user's password is
//! passed to SQLCipher as a passphrase; SQLCipher derives the actual key with
//! PBKDF2-HMAC-SHA512 (256,000 iterations, per-file random salt — SQLCipher 4
//! defaults). There is no recovery: without the password the file is
//! unreadable.

mod migrations;

use std::path::{Path, PathBuf};

use rusqlite::{Connection, ErrorCode, OpenFlags};

pub use migrations::LATEST_VERSION;

/// File name of the database inside the app-data directory.
pub const DB_FILE_NAME: &str = "plutus.db";

/// Errors from opening, unlocking or migrating the database.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("incorrect password")]
    WrongPassword,
    #[error("password must not be empty")]
    EmptyPassword,
    #[error("password must be at least {min} characters")]
    PasswordTooShort { min: usize },
    #[error("the vault is locked")]
    Locked,
    #[error("a database already exists at {0}")]
    AlreadyExists(PathBuf),
    #[error("no database found at {0}")]
    NotFound(PathBuf),
    #[error(
        "database schema v{found} is newer than this app supports (v{supported}); update Plutus"
    )]
    NewerSchema { found: u32, supported: u32 },
    #[error("migration {version} failed: {source}")]
    Migration {
        version: u32,
        #[source]
        source: rusqlite::Error,
    },
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

impl DbError {
    /// Stable machine-readable code for the UI (messages may change; codes don't).
    pub fn code(&self) -> &'static str {
        match self {
            DbError::WrongPassword => "wrong_password",
            DbError::EmptyPassword => "empty_password",
            DbError::PasswordTooShort { .. } => "password_too_short",
            DbError::Locked => "locked",
            DbError::AlreadyExists(_) => "already_exists",
            DbError::NotFound(_) => "not_found",
            DbError::NewerSchema { .. } => "newer_schema",
            DbError::Migration { .. } => "migration_failed",
            DbError::Io(_) => "io",
            DbError::Sqlite(_) => "database",
        }
    }
}

/// An unlocked, fully migrated Plutus database.
#[derive(Debug)]
pub struct Database {
    conn: Connection,
    path: PathBuf,
}

impl Database {
    /// Path of the database file inside an app-data directory.
    pub fn path_in(data_dir: &Path) -> PathBuf {
        data_dir.join(DB_FILE_NAME)
    }

    /// Whether a database file exists at `path` (i.e. first run or not).
    pub fn exists(path: &Path) -> bool {
        path.is_file()
    }

    /// Create a new encrypted database protected by `password`.
    /// Fails if a file already exists at `path`, so data is never overwritten.
    pub fn create(path: &Path, password: &str) -> Result<Self, DbError> {
        check_password(password)?;
        if path.exists() {
            return Err(DbError::AlreadyExists(path.to_path_buf()));
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let conn = Connection::open_with_flags(path, flags)?;
        Self::finish_open(conn, path, password)
    }

    /// Unlock an existing database with `password`.
    pub fn open(path: &Path, password: &str) -> Result<Self, DbError> {
        check_password(password)?;
        if !path.is_file() {
            return Err(DbError::NotFound(path.to_path_buf()));
        }
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let conn = Connection::open_with_flags(path, flags)?;
        Self::finish_open(conn, path, password)
    }

    fn finish_open(mut conn: Connection, path: &Path, password: &str) -> Result<Self, DbError> {
        // pragma_update binds the value as a quoted SQL string literal, so any
        // password characters (quotes included) are passed through safely.
        conn.pragma_update(None, "key", password)?;
        verify_key(&conn)?;
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        migrations::migrate(&mut conn)?;
        Ok(Self {
            conn,
            path: path.to_path_buf(),
        })
    }

    /// Re-encrypt the database under a new password.
    pub fn change_password(&self, new_password: &str) -> Result<(), DbError> {
        check_password(new_password)?;
        self.conn.pragma_update(None, "rekey", new_password)?;
        Ok(())
    }

    /// Current schema version.
    pub fn schema_version(&self) -> Result<u32, DbError> {
        migrations::current_version(&self.conn)
    }

    /// Location of the database file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Underlying connection, for repository modules inside this crate.
    #[allow(dead_code)] // first users arrive with the accounts module (M1)
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }
}

fn check_password(password: &str) -> Result<(), DbError> {
    if password.is_empty() {
        Err(DbError::EmptyPassword)
    } else {
        Ok(())
    }
}

/// SQLCipher only discovers a wrong key on first read; force one and map the
/// "not a database" failure to [`DbError::WrongPassword`].
fn verify_key(conn: &Connection) -> Result<(), DbError> {
    match conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    }) {
        Ok(_) => Ok(()),
        Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == ErrorCode::NotADatabase => {
            Err(DbError::WrongPassword)
        }
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tmp() -> (TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = Database::path_in(dir.path());
        (dir, path)
    }

    #[test]
    fn create_then_reopen_with_same_password() {
        let (_d, path) = tmp();
        assert!(!Database::exists(&path));
        let db = Database::create(&path, "correct horse").unwrap();
        assert_eq!(db.schema_version().unwrap(), LATEST_VERSION);
        drop(db);
        assert!(Database::exists(&path));
        let db = Database::open(&path, "correct horse").unwrap();
        assert_eq!(db.schema_version().unwrap(), LATEST_VERSION);
    }

    #[test]
    fn wrong_password_is_rejected() {
        let (_d, path) = tmp();
        Database::create(&path, "right").unwrap();
        assert!(matches!(
            Database::open(&path, "wrong"),
            Err(DbError::WrongPassword)
        ));
    }

    #[test]
    fn file_on_disk_is_encrypted() {
        let (_d, path) = tmp();
        let db = Database::create(&path, "secret").unwrap();
        db.conn()
            .execute(
                "INSERT INTO meta (key, value) VALUES ('probe', 'PLAINTEXT_MARKER')",
                [],
            )
            .unwrap();
        db.conn()
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
            .unwrap();
        drop(db);
        let bytes = std::fs::read(&path).unwrap();
        assert!(
            !bytes.starts_with(b"SQLite format 3\0"),
            "plain SQLite header found"
        );
        assert!(
            !bytes.windows(16).any(|w| w == b"PLAINTEXT_MARKER"),
            "plaintext leaked into the file"
        );
        // And a plain (non-keyed) connection cannot read it.
        let plain = Connection::open(&path).unwrap();
        assert!(verify_key(&plain).is_err());
    }

    #[test]
    fn create_refuses_to_overwrite() {
        let (_d, path) = tmp();
        Database::create(&path, "a").unwrap();
        assert!(matches!(
            Database::create(&path, "b"),
            Err(DbError::AlreadyExists(_))
        ));
        // Original password still works: nothing was overwritten.
        Database::open(&path, "a").unwrap();
    }

    #[test]
    fn open_missing_file_is_not_found() {
        let (_d, path) = tmp();
        assert!(matches!(
            Database::open(&path, "x"),
            Err(DbError::NotFound(_))
        ));
        assert!(!path.exists(), "open must not create a file");
    }

    #[test]
    fn empty_password_is_rejected() {
        let (_d, path) = tmp();
        assert!(matches!(
            Database::create(&path, ""),
            Err(DbError::EmptyPassword)
        ));
        assert!(!path.exists());
    }

    #[test]
    fn passwords_with_quotes_and_unicode_work() {
        let (_d, path) = tmp();
        let pw = "it's \"tricky\"; DROP TABLE meta; -- 🔐 ñ";
        Database::create(&path, pw).unwrap();
        Database::open(&path, pw).unwrap();
        assert!(matches!(
            Database::open(&path, "it's"),
            Err(DbError::WrongPassword)
        ));
    }

    #[test]
    fn change_password_rekeys() {
        let (_d, path) = tmp();
        let db = Database::create(&path, "old").unwrap();
        db.change_password("new").unwrap();
        drop(db);
        assert!(matches!(
            Database::open(&path, "old"),
            Err(DbError::WrongPassword)
        ));
        Database::open(&path, "new").unwrap();
    }

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(DbError::WrongPassword.code(), "wrong_password");
        assert_eq!(DbError::Locked.code(), "locked");
        assert_eq!(
            DbError::PasswordTooShort { min: 8 }.code(),
            "password_too_short"
        );
    }

    #[test]
    fn creates_missing_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/b/c").join(DB_FILE_NAME);
        Database::create(&path, "pw").unwrap();
        assert!(path.is_file());
    }
}
