//! Schema migrations.
//!
//! Migrations are embedded SQL files applied in order. The applied version is
//! tracked in SQLite's `PRAGMA user_version`. Rules:
//! - Append new migrations to [`MIGRATIONS`] with the next number.
//! - Never edit or reorder a migration that has shipped in a release.

use rusqlite::Connection;

use super::DbError;

/// Ordered list of `(version, sql)`. Version N must be at index N-1.
pub(crate) const MIGRATIONS: &[(u32, &str)] = &[
    (1, include_str!("../../migrations/0001_meta.sql")),
    (2, include_str!("../../migrations/0002_accounts.sql")),
    (3, include_str!("../../migrations/0003_transactions.sql")),
];

/// Schema version a fully migrated database reports.
pub const LATEST_VERSION: u32 = MIGRATIONS.len() as u32;

pub(crate) fn current_version(conn: &Connection) -> Result<u32, DbError> {
    Ok(conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

/// Apply every migration newer than the database's current version. Each
/// migration runs in its own transaction together with its version bump, so
/// a failure leaves the database at the last good version.
pub(crate) fn migrate(conn: &mut Connection) -> Result<(), DbError> {
    let current = current_version(conn)?;
    if current > LATEST_VERSION {
        return Err(DbError::NewerSchema {
            found: current,
            supported: LATEST_VERSION,
        });
    }
    for &(version, sql) in MIGRATIONS.iter().filter(|(v, _)| *v > current) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)
            .map_err(|source| DbError::Migration { version, source })?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_versions_are_sequential() {
        for (i, (v, _)) in MIGRATIONS.iter().enumerate() {
            assert_eq!(*v as usize, i + 1, "migration at index {i} has version {v}");
        }
    }

    #[test]
    fn migrate_is_idempotent_on_plain_connection() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        migrate(&mut conn).unwrap();
        assert_eq!(current_version(&conn).unwrap(), LATEST_VERSION);
        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM meta WHERE key = 'created_at'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "seed rows must not be duplicated by re-running");
    }

    #[test]
    fn upgrades_from_every_older_version() {
        // Databases left at each earlier schema by previous releases.
        for from in 1..LATEST_VERSION {
            let mut conn = Connection::open_in_memory().unwrap();
            for (_, sql) in &MIGRATIONS[..from as usize] {
                conn.execute_batch(sql).unwrap();
            }
            conn.pragma_update(None, "user_version", from).unwrap();
            conn.execute("INSERT INTO meta (key, value) VALUES ('probe', 'kept')", [])
                .unwrap();
            migrate(&mut conn).unwrap_or_else(|e| panic!("upgrade from v{from}: {e}"));
            assert_eq!(
                current_version(&conn).unwrap(),
                LATEST_VERSION,
                "from v{from}"
            );
            let v: String = conn
                .query_row("SELECT value FROM meta WHERE key = 'probe'", [], |r| {
                    r.get(0)
                })
                .unwrap();
            assert_eq!(v, "kept", "from v{from}");
        }
    }

    #[test]
    fn upgrades_a_v1_database_keeping_its_data() {
        // A database created by Plutus v0.1.0 (schema v1).
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATIONS[0].1).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        conn.execute("INSERT INTO meta (key, value) VALUES ('probe', 'v1')", [])
            .unwrap();

        migrate(&mut conn).unwrap();
        assert_eq!(current_version(&conn).unwrap(), LATEST_VERSION);
        let v: String = conn
            .query_row("SELECT value FROM meta WHERE key = 'probe'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(v, "v1");
        let n: i64 = conn
            .query_row("SELECT count(*) FROM accounts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn refuses_database_from_newer_app() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", LATEST_VERSION + 1)
            .unwrap();
        assert!(matches!(
            migrate(&mut conn),
            Err(DbError::NewerSchema { .. })
        ));
    }
}
