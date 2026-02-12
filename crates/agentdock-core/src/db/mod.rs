use std::path::Path;

use rusqlite::{params, Connection};
use thiserror::Error;

const MIGRATIONS: &[(&str, &str)] =
    &[("0001_init", include_str!("../../migrations/0001_init.sql"))];

#[derive(Debug, Error)]
pub enum DbError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub fn init_db(path: &Path) -> Result<Connection, DbError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut connection = Connection::open(path)?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    run_migrations(&mut connection)?;
    Ok(connection)
}

pub fn run_migrations(connection: &mut Connection) -> Result<(), DbError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            id TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL
        );",
    )?;

    for (migration_id, migration_sql) in MIGRATIONS {
        let already_applied: i64 = connection.query_row(
            "SELECT COUNT(1) FROM schema_migrations WHERE id = ?1",
            [migration_id],
            |row| row.get(0),
        )?;

        if already_applied > 0 {
            continue;
        }

        let transaction = connection.transaction()?;
        transaction.execute_batch(migration_sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations(id, applied_at) VALUES (?1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![migration_id],
        )?;
        transaction.commit()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{init_db, run_migrations};
    use rusqlite::Connection;

    fn table_exists(conn: &Connection, name: &str) -> bool {
        conn.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type='table' AND name=?1
            )",
            [name],
            |row| row.get::<_, i64>(0),
        )
        .expect("query sqlite_master should succeed")
            == 1
    }

    #[test]
    fn run_migrations_creates_required_tables() {
        let mut conn = Connection::open_in_memory().expect("in-memory sqlite should open");
        run_migrations(&mut conn).expect("migrations should run");

        let expected_tables = [
            "providers",
            "accounts",
            "configs",
            "mcps",
            "skills",
            "threads",
            "thread_messages",
            "switch_events",
            "remote_devices",
            "remote_sessions",
        ];

        for table in expected_tables {
            assert!(table_exists(&conn, table), "table missing: {table}");
        }
    }

    #[test]
    fn run_migrations_is_idempotent() {
        let mut conn = Connection::open_in_memory().expect("in-memory sqlite should open");
        run_migrations(&mut conn).expect("first migration should run");
        run_migrations(&mut conn).expect("second migration should be no-op");

        let applied: i64 = conn
            .query_row("SELECT COUNT(1) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("count query should succeed");
        assert_eq!(applied, 1);
    }

    #[test]
    fn init_db_runs_migrations_for_empty_file() {
        let mut path = std::env::temp_dir();
        path.push(format!("agentdock-test-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let conn = init_db(&path).expect("init_db should create sqlite and run migrations");
        assert!(table_exists(&conn, "threads"));
        assert!(table_exists(&conn, "remote_sessions"));

        drop(conn);
        let _ = std::fs::remove_file(path);
    }
}
