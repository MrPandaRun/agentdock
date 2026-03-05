use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use agentdock_core::mcp::{
    delete_mcp_server, get_mcp_server, insert_mcp_operation_log, list_mcp_operation_logs,
    list_mcp_servers, update_mcp_server_enabled, update_mcp_server_test_result, upsert_mcp_server,
    McpServer,
};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tauri::Manager;

use crate::payloads::{
    DeleteMcpServerRequest, McpConnectionTestResultPayload, McpFieldErrorPayload,
    McpOperationLogPayload, McpServerPayload, SaveMcpServerRequest, SaveMcpServerResponsePayload,
    SyncMcpConfigsRequest, SyncMcpConfigsResponsePayload, SyncMcpProviderResultPayload,
    TestMcpConnectionRequest, ToggleMcpServerEnabledRequest,
};

const SUPPORTED_PROVIDERS: [&str; 3] = ["claude_code", "codex", "opencode"];
const DEFAULT_SECRET_HEADER: &str = "Authorization";
const HTTP_TEST_TIMEOUT_SECONDS: u64 = 6;
const SENSITIVE_HEADER_KEYS: [&str; 5] = [
    "authorization",
    "proxy-authorization",
    "x-api-key",
    "api-key",
    "x-auth-token",
];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpSecretConfig {
    header_name: Option<String>,
    token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct McpContext {
    db_path: PathBuf,
    home_dir: PathBuf,
}

impl McpContext {
    pub fn from_app_handle(app: &tauri::AppHandle) -> Result<Self, String> {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|error| format!("Failed to get app data directory: {error}"))?;
        let db_path = app_data_dir.join("agentdock.db");
        let home_dir =
            dirs::home_dir().ok_or_else(|| "Failed to resolve home directory".to_string())?;

        Ok(Self { db_path, home_dir })
    }

    fn get_connection(&self) -> Result<rusqlite::Connection, String> {
        rusqlite::Connection::open(&self.db_path)
            .map_err(|error| format!("Failed to open database: {error}"))
    }
}

#[derive(Debug, Clone)]
struct ValidatedSaveRequest {
    id: Option<String>,
    name: String,
    transport: String,
    target: String,
    args_json: String,
    headers_json: String,
    env_json: String,
    scope_providers: Vec<String>,
    enabled: bool,
    version: String,
    secret_header_name: Option<String>,
    secret_token: Option<String>,
    clear_secret: bool,
}

#[derive(Debug, Clone)]
struct ValidatedTestRequest {
    id: Option<String>,
    transport: String,
    target: String,
    args_json: String,
    headers_json: String,
    env_json: String,
    secret_header_name: Option<String>,
    secret_token: Option<String>,
}

#[derive(Debug, Clone)]
struct AppliedSyncWrite {
    config_path: PathBuf,
    backup_path: Option<PathBuf>,
    original_exists: bool,
}

#[derive(Debug, Clone)]
struct SyncExecution {
    success: bool,
    rolled_back: bool,
    message: Option<String>,
    results: Vec<SyncMcpProviderResultPayload>,
}

#[derive(Debug, Clone)]
struct DiscoveredProviderServer {
    provider_id: String,
    name: String,
    transport: String,
    target: String,
    args_json: String,
    headers_json: String,
    env_json: String,
    secret_header_name: Option<String>,
    secret_token: Option<String>,
}

#[derive(Debug, Default)]
struct DiscoverySnapshot {
    servers: Vec<DiscoveredProviderServer>,
    readable_providers: HashSet<String>,
}

pub fn list_mcp_servers_cmd(ctx: &McpContext) -> Result<Vec<McpServerPayload>, String> {
    let mut conn = ctx.get_connection()?;
    if let Err(error) = sync_managed_servers_from_agents(&mut conn, &ctx.home_dir) {
        eprintln!("[MCP] Failed to sync installed MCP servers from providers: {error}");
    }
    let rows =
        list_mcp_servers(&conn).map_err(|error| format!("Failed to list MCP servers: {error}"))?;
    Ok(rows.into_iter().map(server_to_payload).collect())
}

pub fn list_mcp_operation_logs_cmd(
    ctx: &McpContext,
    limit: Option<u32>,
) -> Result<Vec<McpOperationLogPayload>, String> {
    let conn = ctx.get_connection()?;
    let max_items = limit.unwrap_or(50).min(200);
    let logs = list_mcp_operation_logs(&conn, max_items)
        .map_err(|error| format!("Failed to list MCP operation logs: {error}"))?;

    Ok(logs
        .into_iter()
        .map(|log| McpOperationLogPayload {
            id: log.id,
            mcp_id: log.mcp_id,
            action: log.action,
            actor: log.actor,
            details_json: log.details_json,
            created_at: log.created_at,
        })
        .collect())
}

fn sync_managed_servers_from_agents(
    conn: &mut rusqlite::Connection,
    home_dir: &Path,
) -> Result<(), String> {
    let discovered = discover_provider_installed_servers(home_dir);
    if discovered.servers.is_empty() && discovered.readable_providers.is_empty() {
        return Ok(());
    }

    let transaction = conn
        .transaction()
        .map_err(|error| format!("Failed to open discovery sync transaction: {error}"))?;
    let mut existing = list_mcp_servers(&transaction)
        .map_err(|error| format!("Failed to list MCP servers during discovery sync: {error}"))?;

    let mut discovered_by_signature: HashMap<String, Vec<DiscoveredProviderServer>> = HashMap::new();
    for discovered_server in &discovered.servers {
        let signature = discovered_signature_key(discovered_server);
        discovered_by_signature
            .entry(signature)
            .or_default()
            .push(discovered_server.clone());
    }

    for index in 0..existing.len() {
        let mut server = existing[index].clone();
        let mut changed = false;
        let signature = server_signature_key(&server);
        let discovered_matches = discovered_by_signature.get(&signature);

        let mut scope_set = decode_scope_providers(&server.scope)
            .into_iter()
            .collect::<HashSet<_>>();
        for provider_id in &discovered.readable_providers {
            let provider_present = discovered_matches
                .map(|matches| matches.iter().any(|item| item.provider_id == *provider_id))
                .unwrap_or(false);
            if provider_present {
                if scope_set.insert(provider_id.clone()) {
                    changed = true;
                }
            } else if scope_set.remove(provider_id) {
                changed = true;
            }
        }
        if changed {
            server.scope = encode_scope_providers(&ordered_scope_from_set(&scope_set));
        }

        if is_managed_discovered_id(&server.id) {
            if let Some(matches) = discovered_matches {
                let discovered_server = SUPPORTED_PROVIDERS
                    .iter()
                    .find_map(|provider_id| {
                        matches.iter().find(|item| item.provider_id == *provider_id)
                    })
                    .or_else(|| matches.first())
                    .expect("discovered matches should not be empty");

                if server.transport != discovered_server.transport {
                    server.transport = discovered_server.transport.clone();
                    changed = true;
                }
                if server.target != discovered_server.target {
                    server.target = discovered_server.target.clone();
                    server.command = discovered_server.target.clone();
                    changed = true;
                }
                if server.args_json != discovered_server.args_json {
                    server.args_json = discovered_server.args_json.clone();
                    changed = true;
                }
                if server.headers_json != discovered_server.headers_json {
                    server.headers_json = discovered_server.headers_json.clone();
                    changed = true;
                }
                if server.env_json != discovered_server.env_json {
                    server.env_json = discovered_server.env_json.clone();
                    changed = true;
                }

                let merged_secret_json =
                    merge_secret_json_from_discovery(&server.secret_json, discovered_server);
                if server.secret_json != merged_secret_json {
                    server.secret_json = merged_secret_json;
                    changed = true;
                }
            }
        }

        if changed {
            server.updated_at = now_iso_utc();
            upsert_mcp_server(&transaction, &server).map_err(|error| {
                format!("Failed to upsert discovered MCP server {}: {error}", server.id)
            })?;
            existing[index] = server;
        }
    }

    let existing_signatures = existing
        .iter()
        .map(server_signature_key)
        .collect::<HashSet<_>>();

    for (signature, discovered_matches) in discovered_by_signature {
        if existing_signatures.contains(&signature) {
            continue;
        }

        let discovered_server = SUPPORTED_PROVIDERS
            .iter()
            .find_map(|provider_id| {
                discovered_matches
                    .iter()
                    .find(|item| item.provider_id == *provider_id)
            })
            .or_else(|| discovered_matches.first())
            .expect("discovered matches should not be empty");

        let mut provider_scope_set = HashSet::new();
        for match_item in &discovered_matches {
            provider_scope_set.insert(match_item.provider_id.clone());
        }
        let provider_scope = ordered_scope_from_set(&provider_scope_set);

        let id_provider = provider_scope
            .first()
            .cloned()
            .unwrap_or_else(|| discovered_server.provider_id.clone());

        let now = now_iso_utc();
        let server = McpServer {
            id: build_discovered_server_id(
                &id_provider,
                &discovered_server.name,
                &discovered_server.transport,
                &discovered_server.target,
            ),
            name: discovered_server.name.clone(),
            transport: discovered_server.transport.clone(),
            target: discovered_server.target.clone(),
            command: discovered_server.target.clone(),
            args_json: discovered_server.args_json.clone(),
            headers_json: discovered_server.headers_json.clone(),
            env_json: discovered_server.env_json.clone(),
            secret_json: merge_secret_json_from_discovery("{}", discovered_server),
            scope: encode_scope_providers(&provider_scope),
            enabled: true,
            version: "1".to_string(),
            created_at: now.clone(),
            updated_at: now,
            last_tested_at: None,
            last_test_status: None,
            last_test_message: None,
            last_test_duration_ms: None,
        };

        upsert_mcp_server(&transaction, &server).map_err(|error| {
            format!(
                "Failed to insert discovered MCP server {} from {:?}: {error}",
                server.name, provider_scope
            )
        })?;

        let details = json!({
          "summary": format!(
              "import MCP server {} from installed provider config",
              server.name
          ),
          "providerIds": provider_scope,
          "transport": server.transport,
          "target": server.target,
        });
        insert_mcp_operation_log(
            &transaction,
            Some(&server.id),
            "import_installed",
            "desktop_user",
            &details.to_string(),
        )
        .map_err(|error| format!("Failed to log discovered MCP import: {error}"))?;

        existing.push(server);
    }

    transaction
        .commit()
        .map_err(|error| format!("Failed to commit discovery sync transaction: {error}"))?;
    Ok(())
}

fn discover_provider_installed_servers(home_dir: &Path) -> DiscoverySnapshot {
    let mut discovered = Vec::new();
    let mut readable_providers = HashSet::new();

    for provider_id in SUPPORTED_PROVIDERS {
        let mut provider_readable = false;
        for path in provider_discovery_paths(home_dir, provider_id) {
            if !path.exists() || !path.is_file() {
                continue;
            }

            let raw = match fs::read_to_string(&path) {
                Ok(raw) => raw,
                Err(_) => continue,
            };
            provider_readable = true;
            let mut current = parse_provider_config_file(provider_id, &path, &raw);
            discovered.append(&mut current);
        }
        if provider_readable {
            readable_providers.insert(provider_id.to_string());
        }
    }

    DiscoverySnapshot {
        servers: dedupe_discovered_servers(discovered),
        readable_providers,
    }
}

fn parse_provider_config_file(
    provider_id: &str,
    path: &Path,
    raw: &str,
) -> Vec<DiscoveredProviderServer> {
    if let Ok(parsed) = serde_json::from_str::<Value>(raw) {
        return parse_provider_config_document(provider_id, &parsed);
    }

    if provider_id == "codex"
        && path
            .file_name()
            .and_then(|value| value.to_str())
            .map(|name| name.eq_ignore_ascii_case("config.toml"))
            .unwrap_or(false)
    {
        return parse_codex_toml_mcp_servers(raw);
    }

    Vec::new()
}

fn provider_discovery_paths(home_dir: &Path, provider_id: &str) -> Vec<PathBuf> {
    let paths = match provider_id {
        "claude_code" => vec![
            home_dir.join(".claude").join("settings.json"),
            home_dir.join(".claude").join("claude.json"),
        ],
        "codex" => vec![
            home_dir.join(".codex").join("config.toml"),
            home_dir.join(".codex").join("config.json"),
            home_dir.join(".codex").join("settings.json"),
        ],
        "opencode" => vec![
            home_dir.join(".config").join("opencode").join("opencode.json"),
            home_dir.join(".config").join("opencode").join("config.json"),
            home_dir.join(".config").join("opencode").join("settings.json"),
        ],
        _ => Vec::new(),
    };

    let mut seen = HashSet::new();
    paths
        .into_iter()
        .filter(|path| seen.insert(path.clone()))
        .collect()
}

fn parse_provider_config_document(
    provider_id: &str,
    parsed: &Value,
) -> Vec<DiscoveredProviderServer> {
    let mut discovered = Vec::new();

    if let Some(servers_array) = parsed.get("servers").and_then(Value::as_array) {
        for item in servers_array {
            if let Some(server) = parse_discovered_server_value(provider_id, None, item) {
                discovered.push(server);
            }
        }
    }

    for key in ["mcpServers", "mcp_servers", "servers", "mcp"] {
        if let Some(servers_object) = parsed.get(key).and_then(Value::as_object) {
            for (name, value) in servers_object {
                if let Some(server) = parse_discovered_server_value(provider_id, Some(name), value)
                {
                    discovered.push(server);
                }
            }
        }
    }

    if let Some(root_array) = parsed.as_array() {
        for item in root_array {
            if let Some(server) = parse_discovered_server_value(provider_id, None, item) {
                discovered.push(server);
            }
        }
    }

    if let Some(single) = parse_discovered_server_value(provider_id, None, parsed) {
        discovered.push(single);
    }

    dedupe_discovered_servers(discovered)
}

fn parse_codex_toml_mcp_servers(raw: &str) -> Vec<DiscoveredProviderServer> {
    #[derive(Default)]
    struct Entry {
        name: String,
        transport: Option<String>,
        command: Option<String>,
        url: Option<String>,
        target: Option<String>,
        args: Vec<String>,
        headers: BTreeMap<String, String>,
        env: BTreeMap<String, String>,
    }

    let mut entries = Vec::<Entry>::new();
    let mut current: Option<Entry> = None;

    let flush_current = |current: &mut Option<Entry>, entries: &mut Vec<Entry>| {
        if let Some(entry) = current.take() {
            entries.push(entry);
        }
    };

    for raw_line in raw.lines() {
        let line = strip_toml_inline_comment(raw_line).trim().to_string();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            let section = line[1..line.len() - 1].trim();
            if let Some(server_name) = section.strip_prefix("mcp_servers.") {
                flush_current(&mut current, &mut entries);
                current = Some(Entry {
                    name: trim_toml_key_name(server_name),
                    ..Entry::default()
                });
                continue;
            }

            flush_current(&mut current, &mut entries);
            continue;
        }

        let Some(entry) = current.as_mut() else {
            continue;
        };

        let Some((raw_key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let key = raw_key.trim();
        let value = raw_value.trim();

        match key {
            "transport" => {
                if let Some(parsed) = parse_toml_string_value(value) {
                    let normalized = parsed.to_lowercase();
                    if matches!(normalized.as_str(), "stdio" | "http" | "sse") {
                        entry.transport = Some(normalized);
                    }
                }
            }
            "command" => {
                entry.command = parse_toml_string_value(value);
            }
            "url" => {
                entry.url = parse_toml_string_value(value);
            }
            "target" => {
                entry.target = parse_toml_string_value(value);
            }
            "args" => {
                entry.args = parse_toml_string_array(value);
            }
            "env" => {
                entry.env = parse_toml_inline_string_map(value);
            }
            "headers" | "http_headers" => {
                entry.headers = parse_toml_inline_string_map(value);
            }
            _ => {}
        }

        if let Some(raw_env_key) = key.strip_prefix("env.") {
            let env_key = trim_toml_key_name(raw_env_key);
            if let Some(env_value) = parse_toml_string_value(value) {
                if !env_key.is_empty() && !env_value.trim().is_empty() {
                    entry.env.insert(env_key, env_value.trim().to_string());
                }
            }
            continue;
        }

        if let Some(raw_header_key) = key
            .strip_prefix("http_headers.")
            .or_else(|| key.strip_prefix("headers."))
        {
            let header_key = trim_toml_key_name(raw_header_key);
            if let Some(header_value) = parse_toml_string_value(value) {
                if !header_key.is_empty() && !header_value.trim().is_empty() {
                    entry
                        .headers
                        .insert(header_key, header_value.trim().to_string());
                }
            }
            continue;
        }
    }

    flush_current(&mut current, &mut entries);

    let mut discovered = Vec::new();
    for entry in entries {
        if entry.name.trim().is_empty() {
            continue;
        }

        let transport = entry.transport.unwrap_or_else(|| {
            if entry.command.is_some() {
                "stdio".to_string()
            } else if entry.url.is_some() {
                "http".to_string()
            } else {
                "stdio".to_string()
            }
        });

        let target = if transport == "stdio" {
            entry.command.or(entry.target).or(entry.url)
        } else {
            entry.url.or(entry.target).or(entry.command)
        };
        let Some(target) = target.map(|value| value.trim().to_string()) else {
            continue;
        };
        if target.is_empty() {
            continue;
        }

        discovered.push(DiscoveredProviderServer {
            provider_id: "codex".to_string(),
            name: entry.name,
            transport,
            target,
            args_json: serde_json::to_string(&entry.args).unwrap_or_else(|_| "[]".to_string()),
            headers_json: serde_json::to_string(&entry.headers)
                .unwrap_or_else(|_| "{}".to_string()),
            env_json: serde_json::to_string(&entry.env).unwrap_or_else(|_| "{}".to_string()),
            secret_header_name: None,
            secret_token: None,
        });
    }

    dedupe_discovered_servers(discovered)
}

fn strip_toml_inline_comment(raw_line: &str) -> String {
    let mut in_single = false;
    let mut in_double = false;
    let mut result = String::new();

    for ch in raw_line.chars() {
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
                result.push(ch);
            }
            '"' if !in_single => {
                in_double = !in_double;
                result.push(ch);
            }
            '#' if !in_single && !in_double => break,
            _ => result.push(ch),
        }
    }

    result
}

fn trim_toml_key_name(raw: &str) -> String {
    raw.trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

fn parse_toml_string_value(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        return Some(trimmed[1..trimmed.len() - 1].replace("\\\"", "\""));
    }
    if trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2 {
        return Some(trimmed[1..trimmed.len() - 1].to_string());
    }
    Some(trimmed.to_string())
}

fn parse_toml_string_array(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if !(trimmed.starts_with('[') && trimmed.ends_with(']')) {
        return Vec::new();
    }

    if let Ok(parsed) = serde_json::from_str::<Vec<String>>(trimmed) {
        return parsed;
    }

    let content = &trimmed[1..trimmed.len() - 1];
    content
        .split(',')
        .filter_map(|item| parse_toml_string_value(item.trim()))
        .filter(|item| !item.trim().is_empty())
        .collect()
}

fn parse_toml_inline_string_map(raw: &str) -> BTreeMap<String, String> {
    let trimmed = raw.trim();
    if !(trimmed.starts_with('{') && trimmed.ends_with('}')) {
        return BTreeMap::new();
    }

    let content = &trimmed[1..trimmed.len() - 1];
    let mut parsed = BTreeMap::new();
    for part in content.split(',') {
        let Some((raw_key, raw_value)) = part.split_once('=') else {
            continue;
        };
        let key = trim_toml_key_name(raw_key);
        if key.is_empty() {
            continue;
        }
        if let Some(value) = parse_toml_string_value(raw_value) {
            let trimmed_value = value.trim();
            if !trimmed_value.is_empty() {
                parsed.insert(key, trimmed_value.to_string());
            }
        }
    }
    parsed
}

fn parse_discovered_server_value(
    provider_id: &str,
    fallback_name: Option<&str>,
    value: &Value,
) -> Option<DiscoveredProviderServer> {
    let object = value.as_object()?;
    let mut command = first_non_empty_object_string(object, &["command", "cmd"]);
    let url = first_non_empty_object_string(object, &["url", "target", "endpoint"]);

    let mut args =
        parse_string_array_value(object.get("args").or_else(|| object.get("arguments")));
    if let Some(command_value) = object.get("command") {
        let command_array = parse_string_array_value(Some(command_value));
        if !command_array.is_empty() {
            if command.is_none() {
                command = command_array.first().cloned();
            }
            if args.is_empty() && command_array.len() > 1 {
                args.extend(command_array.into_iter().skip(1));
            }
        }
    }

    let env = parse_string_map_value(object.get("env").or_else(|| object.get("environment")));
    let mut headers = parse_string_map_value(
        object
            .get("headers")
            .or_else(|| object.get("http_headers"))
            .or_else(|| object.get("header")),
    );

    let transport = match first_non_empty_object_string(object, &["transport"]) {
        Some(raw_transport) => {
            let normalized = raw_transport.trim().to_lowercase();
            if matches!(normalized.as_str(), "stdio" | "http" | "sse") {
                normalized
            } else {
                return None;
            }
        }
        None => {
            let type_hint = first_non_empty_object_string(object, &["type", "mode"])
                .map(|value| value.to_lowercase());
            match type_hint.as_deref() {
                Some("stdio") | Some("local") => "stdio".to_string(),
                Some("http") => "http".to_string(),
                Some("sse") => "sse".to_string(),
                Some("remote") => "sse".to_string(),
                Some(_) => return None,
                None => {
                    if command.is_some() {
                        "stdio".to_string()
                    } else if url.is_some() {
                        "http".to_string()
                    } else {
                        return None;
                    }
                }
            }
        }
    };

    let target = if transport == "stdio" {
        command.or(url)?
    } else {
        url.or(command)?
    };
    let target = target.trim().to_string();
    if target.is_empty() {
        return None;
    }

    let (secret_header_name, secret_token) = split_secret_from_headers(&mut headers);

    let mut name = first_non_empty_object_string(object, &["name"]);
    if name.is_none() {
        name = fallback_name
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
    }
    if name.is_none() {
        name = derive_server_name_from_target(&transport, &target);
    }

    Some(DiscoveredProviderServer {
        provider_id: provider_id.to_string(),
        name: name.unwrap_or_else(|| "Imported MCP".to_string()),
        transport,
        target: target.clone(),
        args_json: serde_json::to_string(&args).unwrap_or_else(|_| "[]".to_string()),
        headers_json: serde_json::to_string(&headers).unwrap_or_else(|_| "{}".to_string()),
        env_json: serde_json::to_string(&env).unwrap_or_else(|_| "{}".to_string()),
        secret_header_name,
        secret_token,
    })
}

fn first_non_empty_object_string(object: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        let Some(value) = object.get(*key) else {
            continue;
        };
        if let Some(as_str) = value.as_str() {
            let trimmed = as_str.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
            continue;
        }
        if value.is_number() || value.is_boolean() {
            return Some(value.to_string());
        }
    }
    None
}

fn parse_string_array_value(value: Option<&Value>) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };

    let Some(array) = value.as_array() else {
        return Vec::new();
    };

    let mut parsed = Vec::new();
    for item in array {
        if let Some(as_str) = item.as_str() {
            let trimmed = as_str.trim();
            if !trimmed.is_empty() {
                parsed.push(trimmed.to_string());
            }
            continue;
        }
        if item.is_number() || item.is_boolean() {
            parsed.push(item.to_string());
        }
    }
    parsed
}

fn parse_string_map_value(value: Option<&Value>) -> BTreeMap<String, String> {
    let mut parsed = BTreeMap::new();
    let Some(value) = value else {
        return parsed;
    };

    let Some(object) = value.as_object() else {
        return parsed;
    };

    for (key, value) in object {
        if let Some(as_str) = value.as_str() {
            let trimmed = as_str.trim();
            if !trimmed.is_empty() {
                parsed.insert(key.to_string(), trimmed.to_string());
            }
            continue;
        }

        if value.is_number() || value.is_boolean() {
            parsed.insert(key.to_string(), value.to_string());
        }
    }

    parsed
}

fn split_secret_from_headers(
    headers: &mut BTreeMap<String, String>,
) -> (Option<String>, Option<String>) {
    let keys = headers.keys().cloned().collect::<Vec<_>>();
    for key in keys {
        let normalized = key.to_ascii_lowercase();
        if !SENSITIVE_HEADER_KEYS.contains(&normalized.as_str()) {
            continue;
        }
        if let Some(token) = headers.remove(&key) {
            let trimmed = token.trim().to_string();
            if trimmed.is_empty() {
                return (Some(key), None);
            }
            return (Some(key), Some(trimmed));
        }
    }
    (None, None)
}

fn derive_server_name_from_target(transport: &str, target: &str) -> Option<String> {
    if transport == "stdio" {
        let binary = extract_stdio_binary(target);
        if !binary.is_empty() {
            return Some(binary);
        }
    }

    if let Ok(url) = reqwest::Url::parse(target) {
        if let Some(host) = url.host_str() {
            let trimmed = host.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    None
}

fn dedupe_discovered_servers(
    discovered: Vec<DiscoveredProviderServer>,
) -> Vec<DiscoveredProviderServer> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();

    for server in discovered {
        let key = format!(
            "{}|{}|{}|{}",
            server.provider_id,
            server.transport,
            server.target.to_lowercase(),
            server.name.to_lowercase()
        );
        if seen.insert(key) {
            deduped.push(server);
        }
    }

    deduped
}

fn discovered_signature_key(discovered: &DiscoveredProviderServer) -> String {
    format!(
        "{}|{}|{}",
        discovered.transport.trim().to_ascii_lowercase(),
        discovered.target.trim().to_ascii_lowercase(),
        discovered.name.trim().to_ascii_lowercase(),
    )
}

fn server_signature_key(server: &McpServer) -> String {
    format!(
        "{}|{}|{}",
        server.transport.trim().to_ascii_lowercase(),
        server.target.trim().to_ascii_lowercase(),
        server.name.trim().to_ascii_lowercase(),
    )
}

fn ordered_scope_from_set(scope: &HashSet<String>) -> Vec<String> {
    let mut ordered = Vec::new();
    for provider in SUPPORTED_PROVIDERS {
        let candidate = provider.to_string();
        if scope.contains(&candidate) {
            ordered.push(candidate);
        }
    }
    ordered
}

fn is_managed_discovered_id(id: &str) -> bool {
    id.starts_with("managed-")
}

fn build_discovered_server_id(
    provider_id: &str,
    name: &str,
    transport: &str,
    target: &str,
) -> String {
    let slug = slugify(name);
    let hash = stable_hash_32(&format!("{provider_id}|{transport}|{target}"));
    format!("managed-{provider_id}-{slug}-{hash:08x}")
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut pending_dash = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_dash && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(ch.to_ascii_lowercase());
            pending_dash = false;
            continue;
        }
        if ch == '-' || ch == '_' || ch.is_whitespace() {
            pending_dash = true;
        }
    }

    if slug.is_empty() {
        "mcp".to_string()
    } else {
        slug
    }
}

fn stable_hash_32(input: &str) -> u32 {
    let mut hash: u32 = 0x811C_9DC5;
    for byte in input.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

fn merge_secret_json_from_discovery(
    existing_secret_json: &str,
    discovered: &DiscoveredProviderServer,
) -> String {
    let mut secret = parse_secret_config(existing_secret_json);
    if let Some(header_name) = discovered.secret_header_name.as_deref() {
        let trimmed = header_name.trim();
        if !trimmed.is_empty() {
            secret.header_name = Some(trimmed.to_string());
        }
    }
    if let Some(secret_token) = discovered.secret_token.as_deref() {
        let trimmed = secret_token.trim();
        if !trimmed.is_empty() {
            secret.token = Some(trimmed.to_string());
        }
    }
    serde_json::to_string(&secret).unwrap_or_else(|_| "{}".to_string())
}

pub fn save_mcp_server_cmd(
    ctx: &McpContext,
    request: SaveMcpServerRequest,
) -> Result<SaveMcpServerResponsePayload, String> {
    let validated = match validate_save_request(request) {
        Ok(validated) => validated,
        Err(field_errors) => {
            return Ok(SaveMcpServerResponsePayload {
                server: None,
                field_errors,
                message: Some("Validation failed".to_string()),
            });
        }
    };

    let mut conn = ctx.get_connection()?;
    let transaction = conn
        .transaction()
        .map_err(|error| format!("Failed to open database transaction: {error}"))?;

    let now = now_iso_utc();
    let mut existing = validated
        .id
        .as_deref()
        .and_then(|id| get_mcp_server(&transaction, id).ok());

    let server_id = validated
        .id
        .clone()
        .unwrap_or_else(|| generate_mcp_id(&validated.name));

    if existing.is_none() {
        existing = get_mcp_server(&transaction, &server_id).ok();
    }

    let previous_secret = existing
        .as_ref()
        .map(|server| parse_secret_config(&server.secret_json))
        .unwrap_or_default();
    let next_secret = merge_secret_config(
        previous_secret,
        validated.secret_header_name.clone(),
        validated.secret_token.clone(),
        validated.clear_secret,
    );
    let secret_json = serde_json::to_string(&next_secret)
        .map_err(|error| format!("Failed to serialize MCP secret config: {error}"))?;

    let created_at = existing
        .as_ref()
        .map(|server| server.created_at.clone())
        .unwrap_or_else(|| now.clone());

    let before_snapshot = existing.as_ref().map(sanitize_server_for_audit);

    let next_server = McpServer {
        id: server_id.clone(),
        name: validated.name,
        transport: validated.transport,
        target: validated.target.clone(),
        command: validated.target,
        args_json: validated.args_json,
        headers_json: validated.headers_json,
        env_json: validated.env_json,
        secret_json,
        scope: encode_scope_providers(&validated.scope_providers),
        enabled: validated.enabled,
        version: validated.version,
        created_at,
        updated_at: now.clone(),
        last_tested_at: existing
            .as_ref()
            .and_then(|server| server.last_tested_at.clone()),
        last_test_status: existing
            .as_ref()
            .and_then(|server| server.last_test_status.clone()),
        last_test_message: existing
            .as_ref()
            .and_then(|server| server.last_test_message.clone()),
        last_test_duration_ms: existing
            .as_ref()
            .and_then(|server| server.last_test_duration_ms),
    };

    upsert_mcp_server(&transaction, &next_server)
        .map_err(|error| format!("Failed to save MCP server: {error}"))?;

    let action = if existing.is_some() {
        "update"
    } else {
        "create"
    };
    let details = json!({
      "summary": format!("{} MCP server {}", action, next_server.name),
      "before": before_snapshot,
      "after": sanitize_server_for_audit(&next_server),
    });

    insert_mcp_operation_log(
        &transaction,
        Some(&next_server.id),
        action,
        "desktop_user",
        &details.to_string(),
    )
    .map_err(|error| format!("Failed to write MCP audit log: {error}"))?;

    transaction
        .commit()
        .map_err(|error| format!("Failed to commit MCP save transaction: {error}"))?;

    let conn = ctx.get_connection()?;
    let saved = get_mcp_server(&conn, &server_id)
        .map_err(|error| format!("Failed to load saved MCP server: {error}"))?;

    Ok(SaveMcpServerResponsePayload {
        server: Some(server_to_payload(saved)),
        field_errors: Vec::new(),
        message: None,
    })
}

pub fn delete_mcp_server_cmd(
    ctx: &McpContext,
    request: DeleteMcpServerRequest,
) -> Result<(), String> {
    let server_id = request.id.trim();
    if server_id.is_empty() {
        return Err("MCP server id is required".to_string());
    }

    let mut conn = ctx.get_connection()?;
    let transaction = conn
        .transaction()
        .map_err(|error| format!("Failed to open database transaction: {error}"))?;

    let existing = get_mcp_server(&transaction, server_id)
        .map_err(|error| format!("Failed to load MCP server before delete: {error}"))?;

    delete_mcp_server(&transaction, server_id)
        .map_err(|error| format!("Failed to delete MCP server: {error}"))?;

    let details = json!({
      "summary": format!("delete MCP server {}", existing.name),
      "before": sanitize_server_for_audit(&existing)
    });

    insert_mcp_operation_log(
        &transaction,
        Some(server_id),
        "delete",
        "desktop_user",
        &details.to_string(),
    )
    .map_err(|error| format!("Failed to write MCP audit log: {error}"))?;

    transaction
        .commit()
        .map_err(|error| format!("Failed to commit MCP delete transaction: {error}"))?;

    Ok(())
}

pub fn toggle_mcp_server_enabled_cmd(
    ctx: &McpContext,
    request: ToggleMcpServerEnabledRequest,
) -> Result<(), String> {
    let server_id = request.id.trim();
    if server_id.is_empty() {
        return Err("MCP server id is required".to_string());
    }

    let now = now_iso_utc();
    let mut conn = ctx.get_connection()?;
    let transaction = conn
        .transaction()
        .map_err(|error| format!("Failed to open database transaction: {error}"))?;

    update_mcp_server_enabled(&transaction, server_id, request.enabled, &now)
        .map_err(|error| format!("Failed to toggle MCP server status: {error}"))?;

    let details = json!({
      "summary": format!("toggle MCP server {}", server_id),
      "enabled": request.enabled,
    });

    insert_mcp_operation_log(
        &transaction,
        Some(server_id),
        "toggle_enabled",
        "desktop_user",
        &details.to_string(),
    )
    .map_err(|error| format!("Failed to write MCP audit log: {error}"))?;

    transaction
        .commit()
        .map_err(|error| format!("Failed to commit MCP toggle transaction: {error}"))?;

    Ok(())
}

pub fn test_mcp_server_connection_cmd(
    ctx: &McpContext,
    request: TestMcpConnectionRequest,
) -> Result<McpConnectionTestResultPayload, String> {
    let checked_at = now_iso_utc();
    let start = Instant::now();

    let validated = match validate_test_request(request) {
        Ok(validated) => validated,
        Err(field_errors) => {
            return Ok(McpConnectionTestResultPayload {
                success: false,
                error_summary: Some("Validation failed".to_string()),
                duration_ms: start.elapsed().as_millis() as i64,
                checked_at,
                field_errors,
            });
        }
    };

    let (success, error_summary) = run_connection_test(&validated);
    let duration_ms = start.elapsed().as_millis() as i64;

    if let Some(id) = validated
        .id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        if let Ok(mut conn) = ctx.get_connection() {
            if let Ok(transaction) = conn.transaction() {
                let status = if success { "success" } else { "failure" };
                let _ = update_mcp_server_test_result(
                    &transaction,
                    id,
                    &checked_at,
                    status,
                    error_summary.as_deref(),
                    duration_ms,
                );

                let details = json!({
                  "summary": format!("test MCP server {}", id),
                  "success": success,
                  "durationMs": duration_ms,
                  "errorSummary": error_summary,
                });
                let _ = insert_mcp_operation_log(
                    &transaction,
                    Some(id),
                    "test_connection",
                    "desktop_user",
                    &details.to_string(),
                );
                let _ = transaction.commit();
            }
        }
    }

    Ok(McpConnectionTestResultPayload {
        success,
        error_summary,
        duration_ms,
        checked_at,
        field_errors: Vec::new(),
    })
}

pub fn sync_mcp_configs_cmd(
    ctx: &McpContext,
    request: SyncMcpConfigsRequest,
) -> Result<SyncMcpConfigsResponsePayload, String> {
    let provider_ids = resolve_sync_provider_ids(request.provider_ids);
    if provider_ids.is_empty() {
        return Ok(SyncMcpConfigsResponsePayload {
            success: false,
            rolled_back: false,
            message: Some("No valid providers selected for sync.".to_string()),
            results: Vec::new(),
        });
    }

    let mut conn = ctx.get_connection()?;
    if let Err(error) = sync_managed_servers_from_agents(&mut conn, &ctx.home_dir) {
        eprintln!("[MCP] Failed to sync installed MCP servers before write sync: {error}");
    }
    let servers = list_mcp_servers(&conn)
        .map_err(|error| format!("Failed to read MCP servers before sync: {error}"))?;

    let execution = execute_sync(
        &ctx.home_dir,
        &servers,
        &provider_ids,
        request.simulate_failure_provider_id.as_deref(),
    );

    let details = json!({
      "summary": "sync MCP provider config files",
      "providers": provider_ids,
      "success": execution.success,
      "rolledBack": execution.rolled_back,
      "results": execution.results,
    });

    insert_mcp_operation_log(
        &conn,
        None,
        "sync_configs",
        "desktop_user",
        &details.to_string(),
    )
    .map_err(|error| format!("Failed to write MCP sync audit log: {error}"))?;

    Ok(SyncMcpConfigsResponsePayload {
        success: execution.success,
        rolled_back: execution.rolled_back,
        message: execution.message,
        results: execution.results,
    })
}

fn validate_save_request(
    request: SaveMcpServerRequest,
) -> Result<ValidatedSaveRequest, Vec<McpFieldErrorPayload>> {
    let mut field_errors = Vec::new();

    let id = request
        .id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let name = request.name.trim().to_string();
    if name.is_empty() {
        push_field_error(&mut field_errors, "name", "Name is required.");
    }

    let transport = normalize_transport(&request.transport, &mut field_errors);
    let target = request.target.trim().to_string();
    if target.is_empty() {
        push_field_error(&mut field_errors, "target", "Target is required.");
    }

    let args_json = normalize_json_array_string_values(
        request.args_json.as_deref(),
        "argsJson",
        &mut field_errors,
    );
    let headers_json = normalize_json_object_string_values(
        request.headers_json.as_deref(),
        "headersJson",
        &mut field_errors,
    );
    let env_json = normalize_json_object_string_values(
        request.env_json.as_deref(),
        "envJson",
        &mut field_errors,
    );

    if let Some(transport_value) = transport.as_deref() {
        validate_target_for_transport(transport_value, &target, &mut field_errors);
    }

    let scope_providers = normalize_scope_providers(request.scope_providers);
    let version = request
        .version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("1")
        .to_string();

    let secret_token = request
        .secret_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let secret_header_name = normalize_secret_header_name(
        request.secret_header_name,
        secret_token.is_some(),
        &mut field_errors,
    );

    if !field_errors.is_empty() {
        return Err(field_errors);
    }

    Ok(ValidatedSaveRequest {
        id,
        name,
        transport: transport.expect("transport already validated"),
        target,
        args_json,
        headers_json,
        env_json,
        scope_providers,
        enabled: request.enabled,
        version,
        secret_header_name,
        secret_token,
        clear_secret: request.clear_secret.unwrap_or(false),
    })
}

fn validate_test_request(
    request: TestMcpConnectionRequest,
) -> Result<ValidatedTestRequest, Vec<McpFieldErrorPayload>> {
    let mut field_errors = Vec::new();

    let id = request
        .id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let transport = normalize_transport(&request.transport, &mut field_errors);
    let target = request.target.trim().to_string();
    if target.is_empty() {
        push_field_error(&mut field_errors, "target", "Target is required.");
    }

    let args_json = normalize_json_array_string_values(
        request.args_json.as_deref(),
        "argsJson",
        &mut field_errors,
    );
    let headers_json = normalize_json_object_string_values(
        request.headers_json.as_deref(),
        "headersJson",
        &mut field_errors,
    );
    let env_json = normalize_json_object_string_values(
        request.env_json.as_deref(),
        "envJson",
        &mut field_errors,
    );

    if let Some(transport_value) = transport.as_deref() {
        validate_target_for_transport(transport_value, &target, &mut field_errors);
    }

    let secret_token = request
        .secret_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let secret_header_name = normalize_secret_header_name(
        request.secret_header_name,
        secret_token.is_some(),
        &mut field_errors,
    );

    if !field_errors.is_empty() {
        return Err(field_errors);
    }

    Ok(ValidatedTestRequest {
        id,
        transport: transport.expect("transport already validated"),
        target,
        args_json,
        headers_json,
        env_json,
        secret_header_name,
        secret_token,
    })
}

fn normalize_transport(raw: &str, field_errors: &mut Vec<McpFieldErrorPayload>) -> Option<String> {
    let transport = raw.trim().to_lowercase();
    if matches!(transport.as_str(), "stdio" | "http" | "sse") {
        return Some(transport);
    }

    push_field_error(
        field_errors,
        "transport",
        "Transport must be one of: stdio, http, sse.",
    );
    None
}

fn validate_target_for_transport(
    transport: &str,
    target: &str,
    field_errors: &mut Vec<McpFieldErrorPayload>,
) {
    if target.is_empty() {
        return;
    }

    if transport == "stdio" {
        return;
    }

    match reqwest::Url::parse(target) {
        Ok(url) => {
            if !matches!(url.scheme(), "http" | "https") {
                push_field_error(
                    field_errors,
                    "target",
                    "HTTP/SSE target must use http:// or https://.",
                );
            }
        }
        Err(_) => {
            push_field_error(
                field_errors,
                "target",
                "HTTP/SSE target must be a valid URL.",
            );
        }
    }
}

fn normalize_json_array_string_values(
    raw: Option<&str>,
    field_name: &str,
    field_errors: &mut Vec<McpFieldErrorPayload>,
) -> String {
    let source = raw.unwrap_or("").trim();
    if source.is_empty() {
        return "[]".to_string();
    }

    let parsed: Value = match serde_json::from_str(source) {
        Ok(value) => value,
        Err(error) => {
            push_field_error(
                field_errors,
                field_name,
                &format!("Invalid JSON array: {error}"),
            );
            return "[]".to_string();
        }
    };

    let array = match parsed.as_array() {
        Some(array) => array,
        None => {
            push_field_error(field_errors, field_name, "Value must be a JSON array.");
            return "[]".to_string();
        }
    };

    let mut normalized = Vec::new();
    for value in array {
        match value.as_str() {
            Some(string_value) => normalized.push(string_value.to_string()),
            None => {
                push_field_error(field_errors, field_name, "Array items must all be strings.");
                return "[]".to_string();
            }
        }
    }

    serde_json::to_string(&normalized).unwrap_or_else(|_| "[]".to_string())
}

fn normalize_json_object_string_values(
    raw: Option<&str>,
    field_name: &str,
    field_errors: &mut Vec<McpFieldErrorPayload>,
) -> String {
    let source = raw.unwrap_or("").trim();
    if source.is_empty() {
        return "{}".to_string();
    }

    let parsed: Value = match serde_json::from_str(source) {
        Ok(value) => value,
        Err(error) => {
            push_field_error(
                field_errors,
                field_name,
                &format!("Invalid JSON object: {error}"),
            );
            return "{}".to_string();
        }
    };

    let object = match parsed.as_object() {
        Some(object) => object,
        None => {
            push_field_error(field_errors, field_name, "Value must be a JSON object.");
            return "{}".to_string();
        }
    };

    let mut normalized = BTreeMap::new();
    for (key, value) in object {
        match value.as_str() {
            Some(string_value) => {
                normalized.insert(key.to_string(), string_value.to_string());
            }
            None => {
                push_field_error(
                    field_errors,
                    field_name,
                    "Object values must all be strings.",
                );
                return "{}".to_string();
            }
        }
    }

    serde_json::to_string(&normalized).unwrap_or_else(|_| "{}".to_string())
}

fn normalize_scope_providers(raw: Option<Vec<String>>) -> Vec<String> {
    let requested = match raw {
        Some(values) => values,
        None => {
            return SUPPORTED_PROVIDERS
                .iter()
                .map(|provider| (*provider).to_string())
                .collect();
        }
    };
    let mut selected = Vec::new();
    let mut seen = HashSet::new();

    for value in requested {
        let candidate = value.trim();
        if candidate.is_empty() {
            continue;
        }
        if candidate == "all" {
            return SUPPORTED_PROVIDERS
                .iter()
                .map(|provider| (*provider).to_string())
                .collect();
        }
        if SUPPORTED_PROVIDERS.contains(&candidate) && seen.insert(candidate.to_string()) {
            selected.push(candidate.to_string());
        }
    }

    selected
}

fn normalize_secret_header_name(
    raw: Option<String>,
    has_secret_token: bool,
    field_errors: &mut Vec<McpFieldErrorPayload>,
) -> Option<String> {
    let normalized = raw
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    if let Some(header_name) = normalized {
        if header_name.contains(':') || header_name.contains('\n') || header_name.contains('\r') {
            push_field_error(
                field_errors,
                "secretHeaderName",
                "Secret header name contains invalid characters.",
            );
            return None;
        }
        return Some(header_name);
    }

    if has_secret_token {
        return Some(DEFAULT_SECRET_HEADER.to_string());
    }

    None
}

fn merge_secret_config(
    mut previous: McpSecretConfig,
    header_name: Option<String>,
    secret_token: Option<String>,
    clear_secret: bool,
) -> McpSecretConfig {
    if clear_secret {
        previous.token = None;
    }

    if let Some(next_token) = secret_token {
        previous.token = Some(next_token);
    }

    if let Some(next_header_name) = header_name {
        previous.header_name = Some(next_header_name);
    }

    if previous
        .token
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        previous.token = None;
    }

    if previous.token.is_some() && previous.header_name.is_none() {
        previous.header_name = Some(DEFAULT_SECRET_HEADER.to_string());
    }

    previous
}

fn parse_secret_config(raw: &str) -> McpSecretConfig {
    serde_json::from_str::<McpSecretConfig>(raw).unwrap_or_default()
}

fn encode_scope_providers(scope_providers: &[String]) -> String {
    serde_json::to_string(scope_providers).unwrap_or_else(|_| "[]".to_string())
}

fn decode_scope_providers(raw_scope: &str) -> Vec<String> {
    if raw_scope.trim().is_empty() {
        return SUPPORTED_PROVIDERS
            .iter()
            .map(|provider| (*provider).to_string())
            .collect();
    }

    if let Ok(parsed) = serde_json::from_str::<Vec<String>>(raw_scope) {
        return normalize_scope_providers(Some(parsed));
    }

    let from_csv = raw_scope
        .split(',')
        .map(|value| value.trim().to_string())
        .collect::<Vec<_>>();
    normalize_scope_providers(Some(from_csv))
}

fn scope_includes_provider(raw_scope: &str, provider_id: &str) -> bool {
    let scope = decode_scope_providers(raw_scope);
    scope.iter().any(|provider| provider == provider_id)
}

fn server_to_payload(server: McpServer) -> McpServerPayload {
    let secret = parse_secret_config(&server.secret_json);
    McpServerPayload {
        id: server.id,
        name: server.name,
        transport: server.transport,
        target: server.target,
        args_json: server.args_json,
        headers_json: server.headers_json,
        env_json: server.env_json,
        scope_providers: decode_scope_providers(&server.scope),
        enabled: server.enabled,
        version: server.version,
        created_at: server.created_at,
        updated_at: server.updated_at,
        has_secret: secret
            .token
            .as_deref()
            .map(str::trim)
            .map(|value| !value.is_empty())
            .unwrap_or(false),
        secret_header_name: secret.header_name,
        last_tested_at: server.last_tested_at,
        last_test_status: server.last_test_status,
        last_test_message: server.last_test_message,
        last_test_duration_ms: server.last_test_duration_ms,
    }
}

fn sanitize_server_for_audit(server: &McpServer) -> Value {
    let secret = parse_secret_config(&server.secret_json);
    json!({
      "id": server.id,
      "name": server.name,
      "transport": server.transport,
      "target": server.target,
      "enabled": server.enabled,
      "scopeProviders": decode_scope_providers(&server.scope),
      "version": server.version,
      "hasSecret": secret.token.as_deref().map(str::trim).map(|value| !value.is_empty()).unwrap_or(false),
      "secretHeaderName": secret.header_name,
      "lastTestStatus": server.last_test_status,
      "lastTestedAt": server.last_tested_at,
    })
}

fn run_connection_test(validated: &ValidatedTestRequest) -> (bool, Option<String>) {
    match validated.transport.as_str() {
        "stdio" => run_stdio_connection_test(validated),
        "http" | "sse" => run_http_connection_test(validated),
        _ => (false, Some("Unsupported transport type".to_string())),
    }
}

fn run_stdio_connection_test(validated: &ValidatedTestRequest) -> (bool, Option<String>) {
    let binary = extract_stdio_binary(&validated.target);
    if binary.is_empty() {
        return (
            false,
            Some("Stdio target must include a command.".to_string()),
        );
    }

    if command_exists(&binary) {
        (true, None)
    } else {
        (false, Some(format!("Command not found: {binary}")))
    }
}

fn extract_stdio_binary(target: &str) -> String {
    target
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

fn command_exists(binary: &str) -> bool {
    if binary.is_empty() {
        return false;
    }

    let path = Path::new(binary);
    if path.components().count() > 1 || path.is_absolute() {
        return path.exists();
    }

    match Command::new("which").arg(binary).output() {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

fn run_http_connection_test(validated: &ValidatedTestRequest) -> (bool, Option<String>) {
    let client = match Client::builder()
        .timeout(Duration::from_secs(HTTP_TEST_TIMEOUT_SECONDS))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return (
                false,
                Some(format!("Failed to initialize HTTP client: {error}")),
            );
        }
    };

    let headers_map = decode_object_string_map(&validated.headers_json);
    let env_map = decode_object_string_map(&validated.env_json);
    let args_values = decode_string_array(&validated.args_json);

    let mut request = client.get(&validated.target);

    for (key, value) in headers_map {
        request = request.header(key, value);
    }

    if validated.transport == "sse" {
        request = request.header("Accept", "text/event-stream");
    }

    if let Some(secret_token) = validated
        .secret_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let header_name = validated
            .secret_header_name
            .as_deref()
            .unwrap_or(DEFAULT_SECRET_HEADER)
            .to_string();
        request = request.header(header_name, secret_token.to_string());
    }

    // The test endpoint may use query hints from args/env in upstream gateways.
    // We only surface that these values are parsed and available for diagnostics.
    let _diagnostic = (args_values.len(), env_map.len());

    match request.send() {
        Ok(response) => {
            if response.status().is_success() {
                (true, None)
            } else {
                (
                    false,
                    Some(format!(
                        "Target returned non-success status: {}",
                        response.status()
                    )),
                )
            }
        }
        Err(error) => (false, Some(format!("Connection test failed: {error}"))),
    }
}

fn decode_string_array(raw: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(raw).unwrap_or_default()
}

fn decode_object_string_map(raw: &str) -> BTreeMap<String, String> {
    serde_json::from_str::<BTreeMap<String, String>>(raw).unwrap_or_default()
}

fn execute_sync(
    home_dir: &Path,
    servers: &[McpServer],
    provider_ids: &[String],
    simulate_failure_provider_id: Option<&str>,
) -> SyncExecution {
    let mut applied = Vec::<AppliedSyncWrite>::new();
    let mut results = Vec::<SyncMcpProviderResultPayload>::new();
    let mut failure_message: Option<String> = None;

    let enabled_servers = servers
        .iter()
        .filter(|server| server.enabled)
        .cloned()
        .collect::<Vec<_>>();

    for provider_id in provider_ids {
        let scoped_servers = enabled_servers
            .iter()
            .filter(|server| scope_includes_provider(&server.scope, provider_id))
            .cloned()
            .collect::<Vec<_>>();

        let server_count = scoped_servers.len();

        if simulate_failure_provider_id
            .as_deref()
            .map(str::trim)
            .map(|value| value == provider_id)
            .unwrap_or(false)
        {
            failure_message = Some(format!(
                "Simulated sync failure for provider: {provider_id}"
            ));
            results.push(SyncMcpProviderResultPayload {
                provider_id: provider_id.clone(),
                success: false,
                message: failure_message.clone(),
                backup_path: None,
                server_count,
            });
            break;
        }

        let config_path = resolve_provider_sync_path(home_dir, provider_id);
        let existing_config = if config_path.exists() {
            match fs::read_to_string(&config_path) {
                Ok(content) => Some(content),
                Err(error) => {
                    let message = format!(
                        "Failed to read existing provider config {}: {error}",
                        config_path.display()
                    );
                    failure_message = Some(message.clone());
                    results.push(SyncMcpProviderResultPayload {
                        provider_id: provider_id.clone(),
                        success: false,
                        message: Some(message),
                        backup_path: None,
                        server_count,
                    });
                    break;
                }
            }
        } else {
            None
        };

        let document = match build_provider_sync_document(
            provider_id,
            &scoped_servers,
            existing_config.as_deref(),
        ) {
            Ok(document) => document,
            Err(error) => {
                failure_message = Some(error.clone());
                results.push(SyncMcpProviderResultPayload {
                    provider_id: provider_id.clone(),
                    success: false,
                    message: Some(error),
                    backup_path: None,
                    server_count,
                });
                break;
            }
        };

        match write_file_with_backup(&config_path, document.as_bytes()) {
            Ok((backup_path, original_exists)) => {
                applied.push(AppliedSyncWrite {
                    config_path: config_path.clone(),
                    backup_path: backup_path.clone(),
                    original_exists,
                });
                results.push(SyncMcpProviderResultPayload {
                    provider_id: provider_id.clone(),
                    success: true,
                    message: Some("Synced successfully.".to_string()),
                    backup_path: backup_path.map(|path| path.display().to_string()),
                    server_count,
                });
            }
            Err(error) => {
                failure_message = Some(error.clone());
                results.push(SyncMcpProviderResultPayload {
                    provider_id: provider_id.clone(),
                    success: false,
                    message: Some(error),
                    backup_path: None,
                    server_count,
                });
                break;
            }
        }
    }

    let rolled_back = failure_message.is_some() && !applied.is_empty();
    if rolled_back {
        rollback_sync_writes(&applied);

        for result in &mut results {
            if result.success {
                result.success = false;
                result.message =
                    Some("Rolled back due to sync failure in another provider.".to_string());
            }
        }
    }

    SyncExecution {
        success: failure_message.is_none(),
        rolled_back,
        message: failure_message,
        results,
    }
}

fn build_provider_sync_document(
    provider_id: &str,
    servers: &[McpServer],
    existing_config: Option<&str>,
) -> Result<String, String> {
    match provider_id {
        "claude_code" => build_claude_sync_document(servers, existing_config),
        "codex" => build_codex_sync_document(servers, existing_config),
        "opencode" => build_opencode_sync_document(servers, existing_config),
        _ => build_fallback_sync_document(provider_id, servers),
    }
}

fn resolve_provider_server_key(server: &McpServer) -> String {
    let key = server.name.trim();
    if key.is_empty() {
        server.id.trim().to_string()
    } else {
        key.to_string()
    }
}

fn parse_existing_json_root(
    provider_id: &str,
    existing_config: Option<&str>,
) -> Result<Map<String, Value>, String> {
    let Some(raw) = existing_config else {
        return Ok(Map::new());
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Map::new());
    }

    let parsed = serde_json::from_str::<Value>(trimmed).map_err(|error| {
        format!("Failed to parse existing {provider_id} JSON config for MCP sync: {error}")
    })?;

    let object = parsed
        .as_object()
        .ok_or_else(|| format!("Existing {provider_id} config root must be a JSON object."))?;
    Ok(object.clone())
}

fn build_unified_server_spec(server: &McpServer) -> Result<Value, String> {
    let transport = server.transport.trim().to_lowercase();
    if !matches!(transport.as_str(), "stdio" | "http" | "sse") {
        return Err(format!(
            "Unsupported transport '{}' for MCP '{}'.",
            server.transport, server.name
        ));
    }

    let target = server.target.trim();
    if target.is_empty() {
        return Err(format!(
            "MCP '{}' has an empty target and cannot be synced.",
            server.name
        ));
    }

    let mut spec = Map::<String, Value>::new();
    spec.insert("type".to_string(), Value::String(transport.clone()));

    if transport == "stdio" {
        spec.insert("command".to_string(), Value::String(target.to_string()));

        let args = decode_string_array(&server.args_json);
        if !args.is_empty() {
            spec.insert(
                "args".to_string(),
                Value::Array(args.into_iter().map(Value::String).collect()),
            );
        }

        let env = decode_object_string_map(&server.env_json)
            .into_iter()
            .map(|(key, value)| (key, Value::String(value)))
            .collect::<Map<String, Value>>();
        if !env.is_empty() {
            spec.insert("env".to_string(), Value::Object(env));
        }
    } else {
        spec.insert("url".to_string(), Value::String(target.to_string()));

        let mut headers = decode_object_string_map(&server.headers_json);
        let secret = parse_secret_config(&server.secret_json);
        if let Some(secret_token) = secret
            .token
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let header_name = secret
                .header_name
                .as_deref()
                .unwrap_or(DEFAULT_SECRET_HEADER)
                .to_string();
            headers.insert(header_name, secret_token.to_string());
        }

        if !headers.is_empty() {
            let headers_object = headers
                .into_iter()
                .map(|(key, value)| (key, Value::String(value)))
                .collect::<Map<String, Value>>();
            spec.insert("headers".to_string(), Value::Object(headers_object));
        }
    }

    Ok(Value::Object(spec))
}

fn collect_provider_server_specs(servers: &[McpServer]) -> Result<BTreeMap<String, Value>, String> {
    let mut specs = BTreeMap::<String, Value>::new();
    for server in servers {
        let key = resolve_provider_server_key(server);
        if key.is_empty() {
            return Err("MCP server key cannot be empty.".to_string());
        }
        if specs.contains_key(&key) {
            return Err(format!(
                "Duplicate MCP server key '{}' in scoped servers. Please use unique names.",
                key
            ));
        }
        let spec = build_unified_server_spec(server)?;
        specs.insert(key, spec);
    }
    Ok(specs)
}

fn build_claude_sync_document(
    servers: &[McpServer],
    existing_config: Option<&str>,
) -> Result<String, String> {
    let mut root = parse_existing_json_root("claude_code", existing_config)?;
    let specs = collect_provider_server_specs(servers)?;

    let mcp_servers = specs
        .into_iter()
        .collect::<Map<String, Value>>();
    root.insert("mcpServers".to_string(), Value::Object(mcp_servers));

    serde_json::to_string_pretty(&Value::Object(root))
        .map_err(|error| format!("Failed to serialize Claude MCP config: {error}"))
}

fn convert_unified_spec_to_opencode(spec: &Value) -> Result<Value, String> {
    let object = spec
        .as_object()
        .ok_or_else(|| "Unified MCP spec must be a JSON object.".to_string())?;

    let transport = object
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("stdio")
        .to_lowercase();

    let mut converted = Map::<String, Value>::new();
    match transport.as_str() {
        "stdio" => {
            let command = object
                .get("command")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "stdio MCP spec requires a non-empty command.".to_string())?;

            let mut command_items = vec![Value::String(command.to_string())];
            for arg in parse_string_array_value(object.get("args")) {
                command_items.push(Value::String(arg));
            }

            converted.insert("type".to_string(), Value::String("local".to_string()));
            converted.insert("command".to_string(), Value::Array(command_items));
            converted.insert("enabled".to_string(), Value::Bool(true));

            let environment = parse_string_map_value(object.get("env"))
                .into_iter()
                .map(|(key, value)| (key, Value::String(value)))
                .collect::<Map<String, Value>>();
            if !environment.is_empty() {
                converted.insert("environment".to_string(), Value::Object(environment));
            }
        }
        "http" | "sse" => {
            let url = object
                .get("url")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "remote MCP spec requires a non-empty url.".to_string())?;

            converted.insert("type".to_string(), Value::String("remote".to_string()));
            converted.insert("url".to_string(), Value::String(url.to_string()));
            converted.insert("enabled".to_string(), Value::Bool(true));

            let headers = parse_string_map_value(object.get("headers"))
                .into_iter()
                .map(|(key, value)| (key, Value::String(value)))
                .collect::<Map<String, Value>>();
            if !headers.is_empty() {
                converted.insert("headers".to_string(), Value::Object(headers));
            }
        }
        other => {
            return Err(format!(
                "Unsupported MCP transport '{other}' for OpenCode sync."
            ));
        }
    }

    Ok(Value::Object(converted))
}

fn build_opencode_sync_document(
    servers: &[McpServer],
    existing_config: Option<&str>,
) -> Result<String, String> {
    let mut root = parse_existing_json_root("opencode", existing_config)?;
    let specs = collect_provider_server_specs(servers)?;

    let mut mcp = Map::<String, Value>::new();
    for (key, spec) in specs {
        mcp.insert(key, convert_unified_spec_to_opencode(&spec)?);
    }
    root.insert("mcp".to_string(), Value::Object(mcp));

    serde_json::to_string_pretty(&Value::Object(root))
        .map_err(|error| format!("Failed to serialize OpenCode MCP config: {error}"))
}

fn escape_toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn encode_toml_array(values: &[String]) -> String {
    let rendered = values
        .iter()
        .map(|value| format!("\"{}\"", escape_toml_string(value)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{rendered}]")
}

fn is_codex_mcp_section(section_name: &str) -> bool {
    let normalized = section_name.trim();
    normalized == "mcp_servers"
        || normalized.starts_with("mcp_servers.")
        || normalized == "mcp.servers"
        || normalized.starts_with("mcp.servers.")
}

fn strip_existing_codex_mcp_sections(existing_config: &str) -> String {
    let mut kept = Vec::<String>::new();
    let mut skipping_mcp_section = false;

    for line in existing_config.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let section = trimmed[1..trimmed.len() - 1].trim();
            skipping_mcp_section = is_codex_mcp_section(section);
            if skipping_mcp_section {
                continue;
            }
        }

        if skipping_mcp_section {
            continue;
        }

        kept.push(line.to_string());
    }

    kept.join("\n")
}

fn parse_value_string_map(value: Option<&Value>) -> BTreeMap<String, String> {
    parse_string_map_value(value)
}

fn render_codex_mcp_sections(specs: &BTreeMap<String, Value>) -> Result<String, String> {
    let mut sections = Vec::<String>::new();

    for (server_key, spec) in specs {
        let object = spec
            .as_object()
            .ok_or_else(|| format!("MCP spec for '{server_key}' must be a JSON object."))?;
        let transport = object
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("stdio")
            .to_lowercase();

        let mut lines = Vec::<String>::new();
        lines.push(format!(
            "[mcp_servers.\"{}\"]",
            escape_toml_string(server_key)
        ));
        lines.push(format!("type = \"{}\"", escape_toml_string(&transport)));

        match transport.as_str() {
            "stdio" => {
                let command = object
                    .get("command")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        format!("Codex stdio MCP '{server_key}' requires a non-empty command.")
                    })?;
                lines.push(format!("command = \"{}\"", escape_toml_string(command)));

                let args = parse_string_array_value(object.get("args"));
                if !args.is_empty() {
                    lines.push(format!("args = {}", encode_toml_array(&args)));
                }

                let env = parse_value_string_map(object.get("env"));
                for (key, value) in env {
                    lines.push(format!(
                        "env.\"{}\" = \"{}\"",
                        escape_toml_string(&key),
                        escape_toml_string(&value)
                    ));
                }
            }
            "http" | "sse" => {
                let url = object
                    .get("url")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        format!("Codex remote MCP '{server_key}' requires a non-empty url.")
                    })?;
                lines.push(format!("url = \"{}\"", escape_toml_string(url)));

                let headers = parse_value_string_map(object.get("headers"));
                for (key, value) in headers {
                    lines.push(format!(
                        "http_headers.\"{}\" = \"{}\"",
                        escape_toml_string(&key),
                        escape_toml_string(&value)
                    ));
                }
            }
            _ => {
                return Err(format!(
                    "Unsupported transport '{transport}' for Codex MCP '{server_key}'."
                ));
            }
        }

        sections.push(lines.join("\n"));
    }

    Ok(sections.join("\n\n"))
}

fn build_codex_sync_document(
    servers: &[McpServer],
    existing_config: Option<&str>,
) -> Result<String, String> {
    let specs = collect_provider_server_specs(servers)?;
    let existing_text = existing_config.unwrap_or("");
    let mut cleaned = strip_existing_codex_mcp_sections(existing_text)
        .trim_end()
        .to_string();
    let rendered_sections = render_codex_mcp_sections(&specs)?;

    if !rendered_sections.trim().is_empty() {
        if !cleaned.is_empty() {
            cleaned.push_str("\n\n");
        }
        cleaned.push_str(&rendered_sections);
    }

    if !cleaned.is_empty() && !cleaned.ends_with('\n') {
        cleaned.push('\n');
    }

    Ok(cleaned)
}

fn build_fallback_sync_document(provider_id: &str, servers: &[McpServer]) -> Result<String, String> {
    let specs = collect_provider_server_specs(servers)?;
    let document = json!({
      "providerId": provider_id,
      "mcpServers": specs,
      "generatedBy": "agentdock",
      "generatedAt": now_iso_utc(),
    });
    serde_json::to_string_pretty(&document)
        .map_err(|error| format!("Failed to serialize MCP sync config document: {error}"))
}

fn resolve_provider_sync_path(home_dir: &Path, provider_id: &str) -> PathBuf {
    match provider_id {
        "claude_code" => {
            let settings_path = home_dir.join(".claude").join("settings.json");
            let legacy_path = home_dir.join(".claude").join("claude.json");
            if settings_path.exists() {
                settings_path
            } else if legacy_path.exists() {
                legacy_path
            } else {
                settings_path
            }
        }
        "codex" => home_dir.join(".codex").join("config.toml"),
        "opencode" => {
            let opencode_path = home_dir.join(".config").join("opencode").join("opencode.json");
            let config_path = home_dir.join(".config").join("opencode").join("config.json");
            let settings_path = home_dir.join(".config").join("opencode").join("settings.json");

            if opencode_path.exists() {
                opencode_path
            } else if config_path.exists() {
                config_path
            } else if settings_path.exists() {
                settings_path
            } else {
                opencode_path
            }
        }
        _ => home_dir
            .join(".agentdock")
            .join("mcp")
            .join(format!("{provider_id}.json")),
    }
}

fn write_file_with_backup(path: &Path, content: &[u8]) -> Result<(Option<PathBuf>, bool), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create config directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let original_exists = path.exists();
    let backup_path = if original_exists {
        let backup = path.with_extension(format!("bak.{}", chrono::Utc::now().timestamp_millis()));
        fs::copy(path, &backup)
            .map_err(|error| format!("Failed to create backup for {}: {error}", path.display()))?;
        Some(backup)
    } else {
        None
    };

    let temp_path = path.with_extension(format!("tmp.{}", chrono::Utc::now().timestamp_millis()));
    fs::write(&temp_path, content).map_err(|error| {
        format!(
            "Failed to write temporary sync file {}: {error}",
            temp_path.display()
        )
    })?;

    if let Err(error) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(format!(
            "Failed to replace sync file {}: {error}",
            path.display()
        ));
    }

    Ok((backup_path, original_exists))
}

fn rollback_sync_writes(applied: &[AppliedSyncWrite]) {
    for state in applied.iter().rev() {
        if state.original_exists {
            if let Some(backup_path) = &state.backup_path {
                let _ = fs::copy(backup_path, &state.config_path);
            }
        } else if state.config_path.exists() {
            let _ = fs::remove_file(&state.config_path);
        }
    }
}

fn resolve_sync_provider_ids(provider_ids: Option<Vec<String>>) -> Vec<String> {
    let mut selected = Vec::new();
    let mut seen = HashSet::new();

    let raw_values = provider_ids.unwrap_or_else(|| {
        SUPPORTED_PROVIDERS
            .iter()
            .map(|provider| (*provider).to_string())
            .collect()
    });

    for raw in raw_values {
        let provider_id = raw.trim();
        if provider_id.is_empty() {
            continue;
        }
        if !SUPPORTED_PROVIDERS.contains(&provider_id) {
            continue;
        }
        if seen.insert(provider_id.to_string()) {
            selected.push(provider_id.to_string());
        }
    }

    selected
}

fn push_field_error(field_errors: &mut Vec<McpFieldErrorPayload>, field: &str, message: &str) {
    field_errors.push(McpFieldErrorPayload {
        field: field.to_string(),
        message: message.to_string(),
    });
}

fn generate_mcp_id(name: &str) -> String {
    let mut slug = String::new();
    let mut pending_dash = false;

    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_dash && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(ch.to_ascii_lowercase());
            pending_dash = false;
            continue;
        }

        if ch == '-' || ch == '_' || ch.is_whitespace() {
            pending_dash = true;
        }
    }

    if slug.is_empty() {
        slug = "mcp".to_string();
    }

    format!("mcp-{}-{}", slug, chrono::Utc::now().timestamp_millis())
}

fn now_iso_utc() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentdock_core::db::run_migrations;
    use rusqlite::Connection;

    fn sample_server(id: &str) -> McpServer {
        McpServer {
            id: id.to_string(),
            name: format!("{id}-name"),
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
    fn validation_rejects_bad_http_target() {
        let result = validate_save_request(SaveMcpServerRequest {
            id: None,
            name: "Demo".to_string(),
            transport: "http".to_string(),
            target: "not-a-url".to_string(),
            args_json: Some("[]".to_string()),
            headers_json: Some("{}".to_string()),
            env_json: Some("{}".to_string()),
            scope_providers: Some(vec!["codex".to_string()]),
            enabled: true,
            version: Some("1".to_string()),
            secret_header_name: None,
            secret_token: None,
            clear_secret: Some(false),
        });

        assert!(result.is_err());
        let errors = result.expect_err("validation should fail");
        assert!(errors.iter().any(|error| error.field == "target"));
    }

    #[test]
    fn sync_rolls_back_previous_writes_on_failure() {
        let temp_home = tempfile::tempdir().expect("temp home should be created");
        let home_path = temp_home.path();

        let claude_path = resolve_provider_sync_path(home_path, "claude_code");
        let codex_path = resolve_provider_sync_path(home_path, "codex");

        if let Some(parent) = claude_path.parent() {
            fs::create_dir_all(parent).expect("claude parent should exist");
        }
        if let Some(parent) = codex_path.parent() {
            fs::create_dir_all(parent).expect("codex parent should exist");
        }

        fs::write(&claude_path, r#"{"mcpServers":{"legacy":{"command":"uvx"}}}"#)
            .expect("seed claude config");
        fs::write(&codex_path, "model = \"gpt-5\"\n").expect("seed codex config");

        let execution = execute_sync(
            home_path,
            &[sample_server("demo")],
            &["claude_code".to_string(), "codex".to_string()],
            Some("codex"),
        );

        assert!(!execution.success);
        assert!(execution.rolled_back);
        assert_eq!(execution.results.len(), 2);

        let claude_after =
            fs::read_to_string(&claude_path).expect("claude config should remain readable");
        let codex_after =
            fs::read_to_string(&codex_path).expect("codex config should remain readable");

        assert_eq!(claude_after, r#"{"mcpServers":{"legacy":{"command":"uvx"}}}"#);
        assert_eq!(codex_after, "model = \"gpt-5\"\n");
    }

    #[test]
    fn sync_writes_provider_specific_files() {
        let temp_home = tempfile::tempdir().expect("temp home should be created");
        let home_path = temp_home.path();

        let execution = execute_sync(
            home_path,
            &[sample_server("demo")],
            &["claude_code".to_string()],
            None,
        );

        assert!(execution.success);
        assert!(!execution.rolled_back);
        assert_eq!(execution.results.len(), 1);
        assert!(execution.results[0].success);

        let claude_path = resolve_provider_sync_path(home_path, "claude_code");
        let saved = fs::read_to_string(claude_path).expect("sync file should be written");
        assert!(saved.contains("\"mcpServers\""));
        assert!(saved.contains("\"type\": \"stdio\""));
        assert!(saved.contains("\"command\": \"npx\""));
    }

    #[test]
    fn parses_claude_native_mcp_servers_document() {
        let parsed = json!({
            "mcpServers": {
                "filesystem": {
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-filesystem", "."],
                    "env": {
                        "NODE_ENV": "production"
                    }
                },
                "remote-api": {
                    "transport": "http",
                    "url": "https://example.com/mcp",
                    "headers": {
                        "Authorization": "Bearer test-token",
                        "x-client": "agentdock"
                    }
                }
            }
        });

        let discovered = parse_provider_config_document("claude_code", &parsed);
        assert_eq!(discovered.len(), 2);

        let stdio = discovered
            .iter()
            .find(|item| item.name == "filesystem")
            .expect("stdio server should be parsed");
        assert_eq!(stdio.transport, "stdio");
        assert_eq!(stdio.target, "npx");

        let http = discovered
            .iter()
            .find(|item| item.name == "remote-api")
            .expect("http server should be parsed");
        assert_eq!(http.transport, "http");
        assert_eq!(http.target, "https://example.com/mcp");
        assert_eq!(http.secret_header_name.as_deref(), Some("Authorization"));
        assert_eq!(http.secret_token.as_deref(), Some("Bearer test-token"));

        let headers_map = decode_object_string_map(&http.headers_json);
        assert!(!headers_map.contains_key("Authorization"));
        assert_eq!(
            headers_map.get("x-client").map(String::as_str),
            Some("agentdock")
        );
    }

    #[test]
    fn parses_opencode_mcp_document() {
        let parsed = json!({
            "mcp": {
                "filesystem": {
                    "type": "local",
                    "command": ["npx", "-y", "@modelcontextprotocol/server-filesystem", "."],
                    "environment": {
                        "NODE_ENV": "production"
                    }
                },
                "remote-api": {
                    "type": "remote",
                    "url": "https://example.com/mcp",
                    "headers": {
                        "Authorization": "Bearer test-token",
                        "x-client": "agentdock"
                    }
                }
            }
        });

        let discovered = parse_provider_config_document("opencode", &parsed);
        assert_eq!(discovered.len(), 2);

        let local = discovered
            .iter()
            .find(|item| item.name == "filesystem")
            .expect("local server should be parsed");
        assert_eq!(local.transport, "stdio");
        assert_eq!(local.target, "npx");
        assert!(local.args_json.contains("@modelcontextprotocol/server-filesystem"));

        let remote = discovered
            .iter()
            .find(|item| item.name == "remote-api")
            .expect("remote server should be parsed");
        assert_eq!(remote.transport, "sse");
        assert_eq!(remote.target, "https://example.com/mcp");
        assert_eq!(remote.secret_header_name.as_deref(), Some("Authorization"));
        assert_eq!(remote.secret_token.as_deref(), Some("Bearer test-token"));
    }

    #[test]
    fn discovery_sync_imports_provider_installed_servers() {
        let temp_home = tempfile::tempdir().expect("temp home should be created");
        let home_path = temp_home.path();

        let claude_dir = home_path.join(".claude");
        fs::create_dir_all(&claude_dir).expect("claude directory should be created");
        fs::write(
            claude_dir.join("settings.json"),
            r#"{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "."]
    }
  }
}"#,
        )
        .expect("settings.json should be written");

        let mut conn = Connection::open_in_memory().expect("in-memory sqlite should open");
        run_migrations(&mut conn).expect("migrations should run");

        sync_managed_servers_from_agents(&mut conn, home_path)
            .expect("discovery sync should succeed");
        let servers = list_mcp_servers(&conn).expect("servers should load");

        assert_eq!(servers.len(), 1);
        assert!(servers[0].id.starts_with("managed-claude_code-"));
        assert_eq!(servers[0].name, "filesystem");
        assert_eq!(servers[0].transport, "stdio");
        assert_eq!(servers[0].target, "npx");
        assert!(scope_includes_provider(&servers[0].scope, "claude_code"));
    }

    #[test]
    fn discovery_sync_merges_readable_provider_scope_for_manual_servers() {
        let temp_home = tempfile::tempdir().expect("temp home should be created");
        let home_path = temp_home.path();

        let claude_dir = home_path.join(".claude");
        fs::create_dir_all(&claude_dir).expect("claude directory should be created");
        fs::write(
            claude_dir.join("settings.json"),
            r#"{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "."]
    }
  }
}"#,
        )
        .expect("settings.json should be written");

        let mut conn = Connection::open_in_memory().expect("in-memory sqlite should open");
        run_migrations(&mut conn).expect("migrations should run");

        let now = "2026-03-05T00:00:00.000Z".to_string();
        let manual = McpServer {
            id: "manual-filesystem".to_string(),
            name: "filesystem".to_string(),
            transport: "stdio".to_string(),
            target: "npx".to_string(),
            command: "npx".to_string(),
            args_json: r#"["-y","@modelcontextprotocol/server-filesystem","."]"#.to_string(),
            headers_json: "{}".to_string(),
            env_json: "{}".to_string(),
            secret_json: "{}".to_string(),
            scope: r#"["codex"]"#.to_string(),
            enabled: true,
            version: "1".to_string(),
            created_at: now.clone(),
            updated_at: now,
            last_tested_at: None,
            last_test_status: None,
            last_test_message: None,
            last_test_duration_ms: None,
        };
        upsert_mcp_server(&conn, &manual).expect("manual mcp should be inserted");

        sync_managed_servers_from_agents(&mut conn, home_path)
            .expect("discovery sync should succeed");
        let servers = list_mcp_servers(&conn).expect("servers should load");

        let updated = servers
            .iter()
            .find(|server| server.id == "manual-filesystem")
            .expect("manual mcp should still exist");
        assert!(scope_includes_provider(&updated.scope, "codex"));
        assert!(scope_includes_provider(&updated.scope, "claude_code"));
    }

    #[test]
    fn discovery_sync_removes_provider_scope_when_readable_config_no_longer_contains_server() {
        let temp_home = tempfile::tempdir().expect("temp home should be created");
        let home_path = temp_home.path();

        let codex_dir = home_path.join(".codex");
        fs::create_dir_all(&codex_dir).expect("codex directory should be created");
        fs::write(
            codex_dir.join("config.toml"),
            r#"
[profile.default]
model = "gpt-5"
"#,
        )
        .expect("config.toml should be written");

        let mut conn = Connection::open_in_memory().expect("in-memory sqlite should open");
        run_migrations(&mut conn).expect("migrations should run");

        let now = "2026-03-05T00:00:00.000Z".to_string();
        let manual = McpServer {
            id: "manual-filesystem".to_string(),
            name: "filesystem".to_string(),
            transport: "stdio".to_string(),
            target: "npx".to_string(),
            command: "npx".to_string(),
            args_json: r#"["-y","@modelcontextprotocol/server-filesystem","."]"#.to_string(),
            headers_json: "{}".to_string(),
            env_json: "{}".to_string(),
            secret_json: "{}".to_string(),
            scope: r#"["codex","claude_code"]"#.to_string(),
            enabled: true,
            version: "1".to_string(),
            created_at: now.clone(),
            updated_at: now,
            last_tested_at: None,
            last_test_status: None,
            last_test_message: None,
            last_test_duration_ms: None,
        };
        upsert_mcp_server(&conn, &manual).expect("manual mcp should be inserted");

        sync_managed_servers_from_agents(&mut conn, home_path)
            .expect("discovery sync should succeed");
        let servers = list_mcp_servers(&conn).expect("servers should load");

        let updated = servers
            .iter()
            .find(|server| server.id == "manual-filesystem")
            .expect("manual mcp should still exist");
        assert!(!scope_includes_provider(&updated.scope, "codex"));
        assert!(scope_includes_provider(&updated.scope, "claude_code"));
    }

    #[test]
    fn parses_codex_config_toml_mcp_sections() {
        let parsed = parse_codex_toml_mcp_servers(
            r#"
[mcp_servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "."]

[mcp_servers."remote-api"]
transport = "sse"
url = "https://example.com/sse"
"#,
        );

        assert_eq!(parsed.len(), 2);
        let filesystem = parsed
            .iter()
            .find(|item| item.name == "filesystem")
            .expect("filesystem server should exist");
        assert_eq!(filesystem.transport, "stdio");
        assert_eq!(filesystem.target, "npx");

        let remote = parsed
            .iter()
            .find(|item| item.name == "remote-api")
            .expect("remote server should exist");
        assert_eq!(remote.transport, "sse");
        assert_eq!(remote.target, "https://example.com/sse");
    }
}
