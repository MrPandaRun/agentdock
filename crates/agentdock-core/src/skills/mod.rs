use std::path::Path;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SkillError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("skill not found: {0}")]
    NotFound(String),
    #[error("invalid skill metadata: {0}")]
    InvalidMetadata(String),
}

/// Per-provider enable/disable state for a skill
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillEnabledState {
    pub claude_code: bool,
    pub codex: bool,
    pub opencode: bool,
}

impl SkillEnabledState {
    pub fn all_enabled() -> Self {
        Self {
            claude_code: true,
            codex: true,
            opencode: true,
        }
    }

    pub fn all_disabled() -> Self {
        Self {
            claude_code: false,
            codex: false,
            opencode: false,
        }
    }

    pub fn is_enabled_for(&self, provider: &str) -> bool {
        match provider {
            "claude_code" => self.claude_code,
            "codex" => self.codex,
            "opencode" => self.opencode,
            _ => false,
        }
    }

    pub fn set_enabled(&mut self, provider: &str, enabled: bool) {
        match provider {
            "claude_code" => self.claude_code = enabled,
            "codex" => self.codex = enabled,
            "opencode" => self.opencode = enabled,
            _ => {}
        }
    }

    pub fn is_any_enabled(&self) -> bool {
        self.claude_code || self.codex || self.opencode
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub source: String,
    pub version: String,
    pub enabled_json: String,
    pub compatibility_json: String,
    #[serde(rename = "readmeUrl")]
    pub readme_url: Option<String>,
    #[serde(rename = "repoOwner")]
    pub repo_owner: Option<String>,
    #[serde(rename = "repoName")]
    pub repo_name: Option<String>,
    #[serde(rename = "repoBranch")]
    pub repo_branch: Option<String>,
    pub installed_at: i64,
}

impl Skill {
    pub fn get_enabled_state(&self) -> Result<SkillEnabledState, SkillError> {
        if self.enabled_json.is_empty() || self.enabled_json == "{}" {
            return Ok(SkillEnabledState::all_enabled());
        }
        serde_json::from_str(&self.enabled_json).map_err(SkillError::Json)
    }

    pub fn set_enabled_state(&mut self, state: &SkillEnabledState) -> Result<(), SkillError> {
        self.enabled_json = serde_json::to_string(state)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub compatibility: SkillCompatibility,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillCompatibility {
    #[serde(default)]
    pub providers: Vec<String>,
}

/// Skill repository for discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRepo {
    pub id: String,
    pub owner: String,
    pub name: String,
    pub branch: String,
    pub enabled: bool,
    pub created_at: i64,
}

pub fn list_skills(connection: &Connection) -> Result<Vec<Skill>, SkillError> {
    let mut stmt = connection.prepare(
        "SELECT id, name, COALESCE(description, ''), source, version,
                COALESCE(enabled_json, '{\"claude_code\":true,\"codex\":true,\"opencode\":true}'),
                COALESCE(compatibility_json, '{}'),
                COALESCE(readme_url, ''),
                COALESCE(repo_owner, ''),
                COALESCE(repo_name, ''),
                COALESCE(repo_branch, ''),
                COALESCE(installed_at, 0)
         FROM skills ORDER BY name",
    )?;

    let skills = stmt
        .query_map([], |row| {
            Ok(Skill {
                id: row.get(0)?,
                name: row.get(1)?,
                description: nullable_string_from_row(row, 2),
                source: row.get(3)?,
                version: row.get(4)?,
                enabled_json: row.get(5)?,
                compatibility_json: row.get(6)?,
                readme_url: nullable_string_from_row(row, 7),
                repo_owner: nullable_string_from_row(row, 8),
                repo_name: nullable_string_from_row(row, 9),
                repo_branch: nullable_string_from_row(row, 10),
                installed_at: row.get(11)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(skills)
}

fn nullable_string_from_row(row: &rusqlite::Row, idx: usize) -> Option<String> {
    let val: String = row.get(idx).unwrap_or_default();
    if val.is_empty() {
        None
    } else {
        Some(val)
    }
}

pub fn get_skill(connection: &Connection, id: &str) -> Result<Skill, SkillError> {
    let mut stmt = connection.prepare(
        "SELECT id, name, COALESCE(description, ''), source, version,
                COALESCE(enabled_json, '{\"claude_code\":true,\"codex\":true,\"opencode\":true}'),
                COALESCE(compatibility_json, '{}'),
                COALESCE(readme_url, ''),
                COALESCE(repo_owner, ''),
                COALESCE(repo_name, ''),
                COALESCE(repo_branch, ''),
                COALESCE(installed_at, 0)
         FROM skills WHERE id = ?1",
    )?;

    let skill = stmt
        .query_row(params![id], |row| {
            Ok(Skill {
                id: row.get(0)?,
                name: row.get(1)?,
                description: nullable_string_from_row(row, 2),
                source: row.get(3)?,
                version: row.get(4)?,
                enabled_json: row.get(5)?,
                compatibility_json: row.get(6)?,
                readme_url: nullable_string_from_row(row, 7),
                repo_owner: nullable_string_from_row(row, 8),
                repo_name: nullable_string_from_row(row, 9),
                repo_branch: nullable_string_from_row(row, 10),
                installed_at: row.get(11)?,
            })
        })
        .map_err(|_| SkillError::NotFound(id.to_string()))?;

    Ok(skill)
}

pub fn insert_skill(connection: &Connection, skill: &Skill) -> Result<(), SkillError> {
    connection.execute(
        "INSERT INTO skills (id, name, description, source, version, enabled_json, compatibility_json,
                             readme_url, repo_owner, repo_name, repo_branch, installed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(id) DO UPDATE SET
           name = excluded.name,
           description = excluded.description,
           source = excluded.source,
           version = excluded.version,
           enabled_json = excluded.enabled_json,
           compatibility_json = excluded.compatibility_json,
           readme_url = excluded.readme_url,
           repo_owner = excluded.repo_owner,
           repo_name = excluded.repo_name,
           repo_branch = excluded.repo_branch,
           installed_at = excluded.installed_at",
        params![
            skill.id,
            skill.name,
            skill.description,
            skill.source,
            skill.version,
            skill.enabled_json,
            skill.compatibility_json,
            skill.readme_url,
            skill.repo_owner,
            skill.repo_name,
            skill.repo_branch,
            skill.installed_at,
        ],
    )?;

    Ok(())
}

pub fn update_skill_enabled_for_provider(
    connection: &Connection,
    id: &str,
    provider: &str,
    enabled: bool,
) -> Result<(), SkillError> {
    let skill = get_skill(connection, id)?;
    let mut state = skill.get_enabled_state()?;
    state.set_enabled(provider, enabled);
    let enabled_json = serde_json::to_string(&state)?;

    let rows_affected = connection.execute(
        "UPDATE skills SET enabled_json = ?1 WHERE id = ?2",
        params![enabled_json, id],
    )?;

    if rows_affected == 0 {
        return Err(SkillError::NotFound(id.to_string()));
    }

    Ok(())
}

pub fn update_skill_enabled(
    connection: &Connection,
    id: &str,
    enabled: bool,
) -> Result<(), SkillError> {
    let state = if enabled {
        SkillEnabledState::all_enabled()
    } else {
        SkillEnabledState::all_disabled()
    };
    let enabled_json = serde_json::to_string(&state)?;

    let rows_affected = connection.execute(
        "UPDATE skills SET enabled_json = ?1 WHERE id = ?2",
        params![enabled_json, id],
    )?;

    if rows_affected == 0 {
        return Err(SkillError::NotFound(id.to_string()));
    }

    Ok(())
}

pub fn delete_skill(connection: &Connection, id: &str) -> Result<(), SkillError> {
    let rows_affected = connection.execute("DELETE FROM skills WHERE id = ?1", params![id])?;

    if rows_affected == 0 {
        return Err(SkillError::NotFound(id.to_string()));
    }

    Ok(())
}

pub fn parse_skill_metadata(skill_dir: &Path) -> Result<SkillMetadata, SkillError> {
    let metadata_path = skill_dir.join("skill.json");

    if metadata_path.exists() {
        let metadata_content = std::fs::read_to_string(&metadata_path)?;
        let metadata: SkillMetadata = serde_json::from_str(&metadata_content)?;
        return validate_skill_metadata(metadata);
    }

    let skill_md_path = skill_dir.join("SKILL.md");
    if skill_md_path.exists() {
        return parse_skill_md_metadata(skill_dir, &skill_md_path);
    }

    Err(SkillError::InvalidMetadata(format!(
        "skill.json or SKILL.md not found in {}",
        skill_dir.display()
    )))
}

fn parse_skill_md_metadata(skill_dir: &Path, skill_md_path: &Path) -> Result<SkillMetadata, SkillError> {
    let content = std::fs::read_to_string(skill_md_path)?;
    let content = content.trim_start_matches('\u{feff}');

    let mut id: Option<String> = None;
    let mut name: Option<String> = None;
    let mut version: Option<String> = None;
    let mut description: Option<String> = None;

    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() >= 3 {
        let front_matter = parts[1].trim();
        for line in front_matter.lines() {
            let line = line.trim();
            if let Some(value) = line.strip_prefix("id:") {
                id = Some(normalize_front_matter_value(value));
            } else if let Some(value) = line.strip_prefix("name:") {
                name = Some(normalize_front_matter_value(value));
            } else if let Some(value) = line.strip_prefix("version:") {
                version = Some(normalize_front_matter_value(value));
            } else if let Some(value) = line.strip_prefix("description:") {
                description = Some(normalize_front_matter_value(value));
            }
        }
    }

    if name.as_deref().unwrap_or("").is_empty() {
        name = first_markdown_heading(content);
    }

    let fallback_name = skill_dir
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Skill".to_string());

    let name = name
        .filter(|v| !v.trim().is_empty())
        .unwrap_or(fallback_name);

    let fallback_id = slugify_skill_id(&name);
    let id = id
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            if fallback_id.is_empty() {
                None
            } else {
                Some(fallback_id)
            }
        })
        .or_else(|| {
            skill_dir
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "skill".to_string());

    let metadata = SkillMetadata {
        id,
        name,
        version: version
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "1.0.0".to_string()),
        description: description.filter(|v| !v.trim().is_empty()),
        compatibility: SkillCompatibility::default(),
    };

    validate_skill_metadata(metadata)
}

fn validate_skill_metadata(metadata: SkillMetadata) -> Result<SkillMetadata, SkillError> {
    if metadata.id.trim().is_empty() {
        return Err(SkillError::InvalidMetadata(
            "skill metadata must have a non-empty 'id' field".to_string(),
        ));
    }

    if metadata.name.trim().is_empty() {
        return Err(SkillError::InvalidMetadata(
            "skill metadata must have a non-empty 'name' field".to_string(),
        ));
    }

    if metadata.version.trim().is_empty() {
        return Err(SkillError::InvalidMetadata(
            "skill metadata must have a non-empty 'version' field".to_string(),
        ));
    }

    Ok(metadata)
}

fn normalize_front_matter_value(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

fn first_markdown_heading(content: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if let Some(heading) = line.strip_prefix("# ") {
            let heading = heading.trim();
            if !heading.is_empty() {
                return Some(heading.to_string());
            }
        }
    }
    None
}

fn slugify_skill_id(input: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;

    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            out.push(ch.to_ascii_lowercase());
            pending_dash = false;
        } else if ch == '-' || ch == '_' || ch.is_whitespace() {
            pending_dash = true;
        }
    }

    out
}

pub fn create_skill_from_metadata(
    metadata: &SkillMetadata,
    source: &str,
) -> Result<Skill, SkillError> {
    let compatibility_json = serde_json::to_string(&metadata.compatibility)?;
    let enabled_json = serde_json::to_string(&SkillEnabledState::all_enabled())?;

    Ok(Skill {
        id: metadata.id.clone(),
        name: metadata.name.clone(),
        description: metadata.description.clone(),
        source: source.to_string(),
        version: metadata.version.clone(),
        enabled_json,
        compatibility_json,
        readme_url: None,
        repo_owner: None,
        repo_name: None,
        repo_branch: None,
        installed_at: chrono::Utc::now().timestamp_millis(),
    })
}

pub fn create_skill_from_git_metadata(
    metadata: &SkillMetadata,
    git_url: &str,
) -> Result<Skill, SkillError> {
    let mut skill = create_skill_from_metadata(metadata, git_url)?;

    // Parse Git URL to extract owner and repo name
    // Supports: https://github.com/owner/repo, git@github.com:owner/repo
    if let Some((owner, repo_name)) = parse_git_url(git_url) {
        skill.repo_owner = Some(owner.clone());
        skill.repo_name = Some(repo_name.clone());
        skill.repo_branch = Some("main".to_string());
        skill.readme_url = Some(format!(
            "https://github.com/{}/{}/blob/main/README.md",
            owner, repo_name
        ));
    }

    Ok(skill)
}

fn parse_git_url(url: &str) -> Option<(String, String)> {
    // Handle https://github.com/owner/repo format
    if url.starts_with("https://github.com/") || url.starts_with("http://github.com/") {
        let parts: Vec<&str> = url
            .trim_start_matches("https://github.com/")
            .trim_start_matches("http://github.com/")
            .split('/')
            .collect();
        if parts.len() >= 2 {
            let repo_name = parts[1].trim_end_matches(".git").to_string();
            return Some((parts[0].to_string(), repo_name));
        }
    }

    // Handle git@github.com:owner/repo format
    if url.starts_with("git@github.com:") {
        let parts: Vec<&str> = url
            .trim_start_matches("git@github.com:")
            .split('/')
            .collect();
        if parts.len() >= 2 {
            let repo_name = parts[1].trim_end_matches(".git").to_string();
            return Some((parts[0].to_string(), repo_name));
        }
    }

    None
}

// Skill repository management

pub fn list_skill_repos(connection: &Connection) -> Result<Vec<SkillRepo>, SkillError> {
    let mut stmt = connection.prepare(
        "SELECT id, owner, name, branch, enabled, created_at FROM skill_repos ORDER BY created_at DESC",
    )?;

    let repos = stmt
        .query_map([], |row| {
            Ok(SkillRepo {
                id: row.get(0)?,
                owner: row.get(1)?,
                name: row.get(2)?,
                branch: row.get(3)?,
                enabled: row.get::<_, i64>(4)? != 0,
                created_at: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(repos)
}

pub fn insert_skill_repo(connection: &Connection, repo: &SkillRepo) -> Result<(), SkillError> {
    connection.execute(
        "INSERT INTO skill_repos (id, owner, name, branch, enabled, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET
           owner = excluded.owner,
           name = excluded.name,
           branch = excluded.branch,
           enabled = excluded.enabled",
        params![
            repo.id,
            repo.owner,
            repo.name,
            repo.branch,
            repo.enabled as i64,
            repo.created_at,
        ],
    )?;

    Ok(())
}

pub fn delete_skill_repo(connection: &Connection, id: &str) -> Result<(), SkillError> {
    let rows_affected = connection.execute("DELETE FROM skill_repos WHERE id = ?1", params![id])?;

    if rows_affected == 0 {
        return Err(SkillError::NotFound(id.to_string()));
    }

    Ok(())
}

/// Default skill repositories from cc-switch
const DEFAULT_SKILL_REPOS: &[(&str, &str, &str)] = &[
    ("anthropics", "skills", "main"),
    ("ComposioHQ", "awesome-claude-skills", "master"),
    ("cexll", "myclaude", "master"),
    ("JimLiu", "baoyu-skills", "main"),
];

/// Initialize default skill repositories if they don't exist
pub fn init_default_skill_repos(connection: &Connection) -> Result<usize, SkillError> {
    let existing = list_skill_repos(connection)?;
    let existing_keys: std::collections::HashSet<(String, String)> = existing
        .iter()
        .map(|r| (r.owner.clone(), r.name.clone()))
        .collect();

    let mut count = 0;
    for (owner, name, branch) in DEFAULT_SKILL_REPOS {
        let key = (owner.to_string(), name.to_string());
        if !existing_keys.contains(&key) {
            let repo = SkillRepo {
                id: format!("{}/{}", owner, name),
                owner: owner.to_string(),
                name: name.to_string(),
                branch: branch.to_string(),
                enabled: true,
                created_at: chrono::Utc::now().timestamp_millis(),
            };
            insert_skill_repo(connection, &repo)?;
            count += 1;
        }
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::fs;

    fn setup_test_db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("in-memory sqlite should open");
        crate::db::run_migrations(&mut conn).expect("migrations should run");
        conn
    }

    #[test]
    fn list_skills_returns_empty_for_new_db() {
        let conn = setup_test_db();
        let skills = list_skills(&conn).expect("list_skills should succeed");
        assert!(skills.is_empty());
    }

    #[test]
    fn insert_and_list_skills() {
        let conn = setup_test_db();

        let skill = Skill {
            id: "test-skill".to_string(),
            name: "Test Skill".to_string(),
            description: Some("A test skill".to_string()),
            source: "/path/to/skill".to_string(),
            version: "1.0.0".to_string(),
            enabled_json: r#"{"claude_code":true,"codex":true,"opencode":true}"#.to_string(),
            compatibility_json: r#"{"providers":["claude_code"]}"#.to_string(),
            readme_url: None,
            repo_owner: None,
            repo_name: None,
            repo_branch: None,
            installed_at: 1234567890,
        };

        insert_skill(&conn, &skill).expect("insert_skill should succeed");

        let skills = list_skills(&conn).expect("list_skills should succeed");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].id, "test-skill");
        assert_eq!(skills[0].name, "Test Skill");
        assert_eq!(skills[0].description, Some("A test skill".to_string()));
    }

    #[test]
    fn update_skill_enabled_for_provider_toggles_state() {
        let conn = setup_test_db();

        let skill = Skill {
            id: "toggle-skill".to_string(),
            name: "Toggle Skill".to_string(),
            description: None,
            source: "/path".to_string(),
            version: "1.0.0".to_string(),
            enabled_json: r#"{"claude_code":true,"codex":true,"opencode":true}"#.to_string(),
            compatibility_json: "{}".to_string(),
            readme_url: None,
            repo_owner: None,
            repo_name: None,
            repo_branch: None,
            installed_at: 1234567890,
        };

        insert_skill(&conn, &skill).expect("insert should succeed");

        update_skill_enabled_for_provider(&conn, "toggle-skill", "codex", false)
            .expect("update should succeed");

        let skills = list_skills(&conn).expect("list should succeed");
        let state = skills[0].get_enabled_state().expect("should parse state");
        assert!(state.claude_code);
        assert!(!state.codex);
        assert!(state.opencode);
    }

    #[test]
    fn delete_skill_removes_entry() {
        let conn = setup_test_db();

        let skill = Skill {
            id: "delete-skill".to_string(),
            name: "Delete Skill".to_string(),
            description: None,
            source: "/path".to_string(),
            version: "1.0.0".to_string(),
            enabled_json: "{}".to_string(),
            compatibility_json: "{}".to_string(),
            readme_url: None,
            repo_owner: None,
            repo_name: None,
            repo_branch: None,
            installed_at: 1234567890,
        };

        insert_skill(&conn, &skill).expect("insert should succeed");

        delete_skill(&conn, "delete-skill").expect("delete should succeed");

        let skills = list_skills(&conn).expect("list should succeed");
        assert!(skills.is_empty());
    }

    #[test]
    fn skill_enabled_state_helpers() {
        let all = SkillEnabledState::all_enabled();
        assert!(all.is_enabled_for("claude_code"));
        assert!(all.is_enabled_for("codex"));
        assert!(all.is_enabled_for("opencode"));
        assert!(all.is_any_enabled());

        let none = SkillEnabledState::all_disabled();
        assert!(!none.is_enabled_for("claude_code"));
        assert!(!none.is_any_enabled());

        let mut partial = SkillEnabledState::all_disabled();
        partial.set_enabled("codex", true);
        assert!(!partial.is_enabled_for("claude_code"));
        assert!(partial.is_enabled_for("codex"));
        assert!(partial.is_any_enabled());
    }

    #[test]
    fn parse_git_url_extracts_owner_and_repo() {
        let result = parse_git_url("https://github.com/owner/repo");
        assert_eq!(result, Some(("owner".to_string(), "repo".to_string())));

        let result = parse_git_url("https://github.com/owner/repo.git");
        assert_eq!(result, Some(("owner".to_string(), "repo".to_string())));

        let result = parse_git_url("git@github.com:owner/repo.git");
        assert_eq!(result, Some(("owner".to_string(), "repo".to_string())));

        let result = parse_git_url("invalid-url");
        assert!(result.is_none());
    }

    #[test]
    fn skill_repo_crud() {
        let conn = setup_test_db();

        let repo = SkillRepo {
            id: "test-repo".to_string(),
            owner: "farion1231".to_string(),
            name: "skills".to_string(),
            branch: "main".to_string(),
            enabled: true,
            created_at: 1234567890,
        };

        insert_skill_repo(&conn, &repo).expect("insert should succeed");

        let repos = list_skill_repos(&conn).expect("list should succeed");
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].owner, "farion1231");

        delete_skill_repo(&conn, "test-repo").expect("delete should succeed");
        let repos = list_skill_repos(&conn).expect("list should succeed");
        assert!(repos.is_empty());
    }

    #[test]
    fn init_default_skill_repos_adds_missing_repos() {
        let conn = setup_test_db();

        // First call should add all 4 default repos
        let count = init_default_skill_repos(&conn).expect("init should succeed");
        assert_eq!(count, 4);

        let repos = list_skill_repos(&conn).expect("list should succeed");
        assert_eq!(repos.len(), 4);

        // Verify the expected repos are present
        let repo_ids: Vec<&str> = repos.iter().map(|r| r.id.as_str()).collect();
        assert!(repo_ids.contains(&"anthropics/skills"));
        assert!(repo_ids.contains(&"ComposioHQ/awesome-claude-skills"));
        assert!(repo_ids.contains(&"cexll/myclaude"));
        assert!(repo_ids.contains(&"JimLiu/baoyu-skills"));

        // Second call should not add any repos
        let count = init_default_skill_repos(&conn).expect("init should be idempotent");
        assert_eq!(count, 0);

        let repos = list_skill_repos(&conn).expect("list should succeed");
        assert_eq!(repos.len(), 4);
    }

    #[test]
    fn parse_skill_metadata_reads_skill_json() {
        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let skill_json = temp_dir.path().join("skill.json");

        fs::write(
            skill_json,
            r#"{
  "id": "json-skill",
  "name": "JSON Skill",
  "version": "2.3.4",
  "description": "from json"
}"#,
        )
        .expect("skill.json should be written");

        let metadata = parse_skill_metadata(temp_dir.path()).expect("metadata should parse");
        assert_eq!(metadata.id, "json-skill");
        assert_eq!(metadata.name, "JSON Skill");
        assert_eq!(metadata.version, "2.3.4");
        assert_eq!(metadata.description, Some("from json".to_string()));
    }

    #[test]
    fn parse_skill_metadata_falls_back_to_skill_md_front_matter() {
        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let skill_md = temp_dir.path().join("SKILL.md");

        fs::write(
            skill_md,
            r#"---
name: "Ably Automation"
description: "Automate Ably workflows"
---

# Ably Automation
"#,
        )
        .expect("SKILL.md should be written");

        let metadata = parse_skill_metadata(temp_dir.path()).expect("metadata should parse");
        assert_eq!(metadata.id, "ably-automation");
        assert_eq!(metadata.name, "Ably Automation");
        assert_eq!(metadata.version, "1.0.0");
        assert_eq!(
            metadata.description,
            Some("Automate Ably workflows".to_string())
        );
    }

    #[test]
    fn parse_skill_metadata_uses_heading_when_front_matter_missing() {
        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let skill_md = temp_dir.path().join("SKILL.md");

        fs::write(
            skill_md,
            r#"# Fallback Skill Name

Content
"#,
        )
        .expect("SKILL.md should be written");

        let metadata = parse_skill_metadata(temp_dir.path()).expect("metadata should parse");
        assert_eq!(metadata.id, "fallback-skill-name");
        assert_eq!(metadata.name, "Fallback Skill Name");
        assert_eq!(metadata.version, "1.0.0");
        assert_eq!(metadata.description, None);
    }

    #[test]
    fn parse_skill_metadata_returns_error_when_metadata_missing() {
        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let error = parse_skill_metadata(temp_dir.path()).expect_err("missing metadata should fail");
        match error {
            SkillError::InvalidMetadata(message) => {
                assert!(message.contains("skill.json or SKILL.md not found"));
            }
            other => panic!("expected invalid metadata error, got {other:?}"),
        }
    }
}
