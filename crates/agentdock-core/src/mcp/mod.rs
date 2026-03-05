use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum McpError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("mcp server not found: {0}")]
    NotFound(String),
    #[error("validation error: {0}")]
    Validation(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpServer {
    pub id: String,
    pub name: String,
    pub transport: String,
    pub target: String,
    pub command: String,
    pub args_json: String,
    pub headers_json: String,
    pub env_json: String,
    pub secret_json: String,
    pub scope: String,
    pub enabled: bool,
    pub version: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_tested_at: Option<String>,
    pub last_test_status: Option<String>,
    pub last_test_message: Option<String>,
    pub last_test_duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpOperationLog {
    pub id: i64,
    pub mcp_id: Option<String>,
    pub action: String,
    pub actor: String,
    pub details_json: String,
    pub created_at: String,
}

pub fn list_mcp_servers(connection: &Connection) -> Result<Vec<McpServer>, McpError> {
    let mut stmt = connection.prepare(
        "SELECT id, name, transport, target, command, args_json,
                COALESCE(headers_json, '{}'),
                COALESCE(env_json, '{}'),
                COALESCE(secret_json, '{}'),
                scope, enabled, version,
                COALESCE(created_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                COALESCE(updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                last_tested_at,
                last_test_status,
                last_test_message,
                last_test_duration_ms
         FROM mcps
         ORDER BY enabled DESC, name ASC, id ASC",
    )?;

    let rows = stmt
        .query_map([], |row| {
            Ok(McpServer {
                id: row.get(0)?,
                name: row.get(1)?,
                transport: row.get(2)?,
                target: row.get(3)?,
                command: row.get(4)?,
                args_json: row.get(5)?,
                headers_json: row.get(6)?,
                env_json: row.get(7)?,
                secret_json: row.get(8)?,
                scope: row.get(9)?,
                enabled: row.get::<_, i64>(10)? != 0,
                version: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
                last_tested_at: row.get(14)?,
                last_test_status: row.get(15)?,
                last_test_message: row.get(16)?,
                last_test_duration_ms: row.get(17)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

pub fn get_mcp_server(connection: &Connection, id: &str) -> Result<McpServer, McpError> {
    let mut stmt = connection.prepare(
        "SELECT id, name, transport, target, command, args_json,
                COALESCE(headers_json, '{}'),
                COALESCE(env_json, '{}'),
                COALESCE(secret_json, '{}'),
                scope, enabled, version,
                COALESCE(created_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                COALESCE(updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                last_tested_at,
                last_test_status,
                last_test_message,
                last_test_duration_ms
         FROM mcps
         WHERE id = ?1",
    )?;

    let server = stmt
        .query_row(params![id], |row| {
            Ok(McpServer {
                id: row.get(0)?,
                name: row.get(1)?,
                transport: row.get(2)?,
                target: row.get(3)?,
                command: row.get(4)?,
                args_json: row.get(5)?,
                headers_json: row.get(6)?,
                env_json: row.get(7)?,
                secret_json: row.get(8)?,
                scope: row.get(9)?,
                enabled: row.get::<_, i64>(10)? != 0,
                version: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
                last_tested_at: row.get(14)?,
                last_test_status: row.get(15)?,
                last_test_message: row.get(16)?,
                last_test_duration_ms: row.get(17)?,
            })
        })
        .map_err(|_| McpError::NotFound(id.to_string()))?;

    Ok(server)
}

pub fn upsert_mcp_server(connection: &Connection, server: &McpServer) -> Result<(), McpError> {
    validate_mcp_server(server)?;
    connection.execute(
        "INSERT INTO mcps (
            id, name, command, args_json, scope, enabled, version,
            transport, target, headers_json, env_json, secret_json,
            created_at, updated_at,
            last_tested_at, last_test_status, last_test_message, last_test_duration_ms
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7,
            ?8, ?9, ?10, ?11, ?12,
            ?13, ?14,
            ?15, ?16, ?17, ?18
         )
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            command = excluded.command,
            args_json = excluded.args_json,
            scope = excluded.scope,
            enabled = excluded.enabled,
            version = excluded.version,
            transport = excluded.transport,
            target = excluded.target,
            headers_json = excluded.headers_json,
            env_json = excluded.env_json,
            secret_json = excluded.secret_json,
            updated_at = excluded.updated_at,
            last_tested_at = excluded.last_tested_at,
            last_test_status = excluded.last_test_status,
            last_test_message = excluded.last_test_message,
            last_test_duration_ms = excluded.last_test_duration_ms",
        params![
            server.id,
            server.name,
            server.command,
            server.args_json,
            server.scope,
            if server.enabled { 1_i64 } else { 0_i64 },
            server.version,
            server.transport,
            server.target,
            server.headers_json,
            server.env_json,
            server.secret_json,
            server.created_at,
            server.updated_at,
            server.last_tested_at,
            server.last_test_status,
            server.last_test_message,
            server.last_test_duration_ms,
        ],
    )?;
    Ok(())
}

pub fn delete_mcp_server(connection: &Connection, id: &str) -> Result<(), McpError> {
    let rows_affected = connection.execute("DELETE FROM mcps WHERE id = ?1", params![id])?;
    if rows_affected == 0 {
        return Err(McpError::NotFound(id.to_string()));
    }
    Ok(())
}

pub fn update_mcp_server_enabled(
    connection: &Connection,
    id: &str,
    enabled: bool,
    updated_at: &str,
) -> Result<(), McpError> {
    let rows_affected = connection.execute(
        "UPDATE mcps
         SET enabled = ?1, updated_at = ?2
         WHERE id = ?3",
        params![if enabled { 1_i64 } else { 0_i64 }, updated_at, id],
    )?;

    if rows_affected == 0 {
        return Err(McpError::NotFound(id.to_string()));
    }

    Ok(())
}

pub fn update_mcp_server_test_result(
    connection: &Connection,
    id: &str,
    checked_at: &str,
    status: &str,
    message: Option<&str>,
    duration_ms: i64,
) -> Result<(), McpError> {
    let rows_affected = connection.execute(
        "UPDATE mcps
         SET last_tested_at = ?1,
             last_test_status = ?2,
             last_test_message = ?3,
             last_test_duration_ms = ?4,
             updated_at = ?1
         WHERE id = ?5",
        params![checked_at, status, message, duration_ms, id],
    )?;

    if rows_affected == 0 {
        return Err(McpError::NotFound(id.to_string()));
    }

    Ok(())
}

pub fn insert_mcp_operation_log(
    connection: &Connection,
    mcp_id: Option<&str>,
    action: &str,
    actor: &str,
    details_json: &str,
) -> Result<(), McpError> {
    connection.execute(
        "INSERT INTO mcp_operation_logs (mcp_id, action, actor, details_json)
         VALUES (?1, ?2, ?3, ?4)",
        params![mcp_id, action, actor, details_json],
    )?;
    Ok(())
}

pub fn list_mcp_operation_logs(
    connection: &Connection,
    limit: u32,
) -> Result<Vec<McpOperationLog>, McpError> {
    let mut stmt = connection.prepare(
        "SELECT id, mcp_id, action, actor, details_json, created_at
         FROM mcp_operation_logs
         ORDER BY id DESC
         LIMIT ?1",
    )?;

    let limit_value = i64::from(limit);
    let rows = stmt
        .query_map(params![limit_value], |row| {
            Ok(McpOperationLog {
                id: row.get(0)?,
                mcp_id: row.get(1)?,
                action: row.get(2)?,
                actor: row.get(3)?,
                details_json: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

fn validate_mcp_server(server: &McpServer) -> Result<(), McpError> {
    if server.id.trim().is_empty() {
        return Err(McpError::Validation("id must not be empty".to_string()));
    }
    if server.name.trim().is_empty() {
        return Err(McpError::Validation("name must not be empty".to_string()));
    }
    if server.target.trim().is_empty() {
        return Err(McpError::Validation("target must not be empty".to_string()));
    }
    if !matches!(server.transport.as_str(), "stdio" | "http" | "sse") {
        return Err(McpError::Validation(format!(
            "unsupported transport: {}",
            server.transport
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("in-memory sqlite should open");
        crate::db::run_migrations(&mut conn).expect("migrations should run");
        conn
    }

    fn sample_server(id: &str) -> McpServer {
        McpServer {
            id: id.to_string(),
            name: "demo".to_string(),
            transport: "stdio".to_string(),
            target: "npx".to_string(),
            command: "npx".to_string(),
            args_json: r#"["-y","demo-mcp"]"#.to_string(),
            headers_json: "{}".to_string(),
            env_json: "{}".to_string(),
            secret_json: "{}".to_string(),
            scope: r#"["claude_code","codex","opencode"]"#.to_string(),
            enabled: true,
            version: "1".to_string(),
            created_at: "2026-03-05T00:00:00.000Z".to_string(),
            updated_at: "2026-03-05T00:00:00.000Z".to_string(),
            last_tested_at: None,
            last_test_status: None,
            last_test_message: None,
            last_test_duration_ms: None,
        }
    }

    #[test]
    fn mcp_server_crud_works() {
        let conn = setup_test_db();
        let mut server = sample_server("mcp-test-1");
        upsert_mcp_server(&conn, &server).expect("insert should succeed");

        let list = list_mcp_servers(&conn).expect("list should succeed");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "mcp-test-1");
        assert!(list[0].enabled);

        server.name = "renamed".to_string();
        server.updated_at = "2026-03-05T01:00:00.000Z".to_string();
        upsert_mcp_server(&conn, &server).expect("update should succeed");

        let loaded = get_mcp_server(&conn, "mcp-test-1").expect("get should succeed");
        assert_eq!(loaded.name, "renamed");
        assert_eq!(loaded.target, "npx");

        delete_mcp_server(&conn, "mcp-test-1").expect("delete should succeed");
        let list = list_mcp_servers(&conn).expect("list should succeed");
        assert!(list.is_empty());
    }

    #[test]
    fn update_enabled_and_test_result() {
        let conn = setup_test_db();
        let server = sample_server("mcp-test-2");
        upsert_mcp_server(&conn, &server).expect("insert should succeed");

        update_mcp_server_enabled(&conn, "mcp-test-2", false, "2026-03-05T02:00:00.000Z")
            .expect("toggle should succeed");
        update_mcp_server_test_result(
            &conn,
            "mcp-test-2",
            "2026-03-05T02:01:00.000Z",
            "success",
            Some("ok"),
            42,
        )
        .expect("test result should persist");

        let loaded = get_mcp_server(&conn, "mcp-test-2").expect("get should succeed");
        assert!(!loaded.enabled);
        assert_eq!(loaded.last_test_status.as_deref(), Some("success"));
        assert_eq!(loaded.last_test_duration_ms, Some(42));
    }

    #[test]
    fn operation_log_roundtrip() {
        let conn = setup_test_db();
        let server = sample_server("mcp-test-3");
        upsert_mcp_server(&conn, &server).expect("insert should succeed");

        insert_mcp_operation_log(
            &conn,
            Some("mcp-test-3"),
            "create",
            "desktop_user",
            r#"{"summary":"created"}"#,
        )
        .expect("insert log should succeed");

        let logs = list_mcp_operation_logs(&conn, 20).expect("list logs should succeed");
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].action, "create");
        assert_eq!(logs[0].mcp_id.as_deref(), Some("mcp-test-3"));
    }
}
