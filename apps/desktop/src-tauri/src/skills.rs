use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::Manager;

use agentdock_core::skills::{
    create_skill_from_git_metadata, create_skill_from_metadata, delete_skill, delete_skill_repo,
    get_skill, init_default_skill_repos, insert_skill, insert_skill_repo, list_skill_repos,
    list_skills, parse_skill_metadata, update_skill_enabled, update_skill_enabled_for_provider,
    Skill, SkillRepo,
};

/// Discoverable skill from a repository
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverableSkill {
    pub key: String,
    pub name: String,
    pub description: String,
    pub directory: String,
    pub readme_url: Option<String>,
    pub repo_owner: String,
    pub repo_name: String,
    pub repo_branch: String,
}

impl From<crate::payloads::DiscoverableSkillPayload> for DiscoverableSkill {
    fn from(payload: crate::payloads::DiscoverableSkillPayload) -> Self {
        Self {
            key: payload.key,
            name: payload.name,
            description: payload.description,
            directory: payload.directory,
            readme_url: payload.readme_url,
            repo_owner: payload.repo_owner,
            repo_name: payload.repo_name,
            repo_branch: payload.repo_branch,
        }
    }
}

pub struct SkillsContext {
    pub db_path: PathBuf,
    pub skills_dir: PathBuf,
}

impl SkillsContext {
    pub fn from_app_handle(app: &tauri::AppHandle) -> Result<Self, String> {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("Failed to get app data directory: {e}"))?;

        let db_path = app_data_dir.join("agentdock.db");
        let skills_dir = app_data_dir.join("skills");

        fs::create_dir_all(&skills_dir)
            .map_err(|e| format!("Failed to create skills directory: {e}"))?;

        // Initialize default skill repos from cc-switch on first access
        let ctx = Self {
            db_path: db_path.clone(),
            skills_dir,
        };
        let conn = ctx.get_connection()?;
        init_default_skill_repos(&conn)
            .map_err(|e| format!("Failed to initialize default skill repos: {e}"))?;

        Ok(ctx)
    }

    pub fn get_connection(&self) -> Result<rusqlite::Connection, String> {
        rusqlite::Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open database: {e}"))
    }
}

pub fn list_skills_cmd(ctx: &SkillsContext) -> Result<Vec<Skill>, String> {
    let conn = ctx.get_connection()?;
    let db_skills = list_skills(&conn).map_err(|e| format!("Failed to list skills: {e}"))?;

    // Create a map to track skills and their enabled providers
    let mut skills_map: std::collections::HashMap<String, Skill> = db_skills
        .into_iter()
        .map(|s| (s.id.clone(), s))
        .collect();

    // Scan all provider directories and collect skills by directory name
    let mut provider_skills_map: std::collections::HashMap<String, (PathBuf, Vec<String>)> = std::collections::HashMap::new();

    for (provider, skills_dir) in get_provider_skills_dirs() {
        if !skills_dir.exists() {
            continue;
        }

        if let Ok(entries) = fs::read_dir(&skills_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                let dir_name = entry.file_name().to_string_lossy().to_string();
                if dir_name.starts_with('.') {
                    continue;
                }

                // Add provider to this skill's provider list
                provider_skills_map
                    .entry(dir_name)
                    .or_insert_with(|| (path.clone(), Vec::new()))
                    .1
                    .push(provider.to_string());
            }
        }
    }

    // Get AgentDock skills directory for master copies
    let agentdock_skills_dir = get_agentdock_skills_dir();

    // Merge provider directory findings with database skills
    for (skill_id, (skill_path, providers)) in provider_skills_map {
        if let Some(existing_skill) = skills_map.get_mut(&skill_id) {
            // Update enabled_json to reflect actual presence in provider directories
            let mut state = existing_skill.get_enabled_state()
                .unwrap_or_else(|_| agentdock_core::skills::SkillEnabledState::all_disabled());

            for provider in &providers {
                state.set_enabled(provider, true);
            }
            existing_skill.enabled_json = serde_json::to_string(&state).unwrap_or_default();

            // Ensure master copy exists in AgentDock directory
            let agentdock_copy = agentdock_skills_dir.join(&skill_id);
            if !agentdock_copy.exists() {
                if let Err(e) = fs::create_dir_all(&agentdock_skills_dir)
                    .and_then(|_| copy_dir_all(&skill_path, &agentdock_copy))
                {
                    eprintln!("[SKILL] Failed to create master copy for {}: {}", skill_id, e);
                } else {
                    // Update source to point to AgentDock master copy
                    existing_skill.source = agentdock_copy.to_string_lossy().to_string();
                }
            }
        } else {
            // Create new skill entry from provider directory
            if let Ok(mut skill) = parse_provider_skill_as_skill(&skill_path, &skill_id, &providers[0]) {
                // Mark all providers where this skill was found
                let mut state = agentdock_core::skills::SkillEnabledState::all_disabled();
                for provider in &providers {
                    state.set_enabled(provider, true);
                }
                skill.enabled_json = serde_json::to_string(&state).unwrap_or_default();

                // Create master copy in AgentDock directory
                let agentdock_copy = agentdock_skills_dir.join(&skill_id);
                if !agentdock_copy.exists() {
                    if let Err(e) = fs::create_dir_all(&agentdock_skills_dir)
                        .and_then(|_| copy_dir_all(&skill_path, &agentdock_copy))
                    {
                        eprintln!("[SKILL] Failed to create master copy for {}: {}", skill_id, e);
                    } else {
                        skill.source = agentdock_copy.to_string_lossy().to_string();
                    }
                }

                skills_map.insert(skill_id, skill);
            }
        }
    }

    // Filter out orphan skills (in database but no files anywhere)
    let all_provider_dirs = get_provider_skills_dirs();

    let all_skills: Vec<Skill> = skills_map
        .into_values()
        .filter(|skill| {
            // Check if skill has files somewhere
            let source_exists = PathBuf::from(&skill.source).exists();
            let in_agentdock = agentdock_skills_dir.join(&skill.id).exists();
            let in_any_provider = all_provider_dirs
                .iter()
                .any(|(_, dir)| dir.join(&skill.id).exists());

            if !source_exists && !in_agentdock && !in_any_provider {
                eprintln!(
                    "[SKILL] Filtering out orphan skill: {} (source={})",
                    skill.id, skill.source
                );
                false
            } else {
                true
            }
        })
        .collect();

    // Sort by name
    let mut sorted_skills = all_skills;
    sorted_skills.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Ok(sorted_skills)
}

fn parse_provider_skill_as_skill(
    skill_dir: &Path,
    dir_name: &str,
    provider: &str,
) -> Result<Skill, String> {
    // Try SKILL.md first
    let skill_md = skill_dir.join("SKILL.md");
    if skill_md.exists() {
        return parse_skill_md_as_skill(&skill_md, dir_name, skill_dir, provider);
    }

    // Try skill.json
    let skill_json = skill_dir.join("skill.json");
    if skill_json.exists() {
        return parse_skill_json_as_skill(&skill_json, dir_name, skill_dir, provider);
    }

    // Return a basic skill with just the directory name
    let mut state = agentdock_core::skills::SkillEnabledState::all_disabled();
    state.set_enabled(provider, true);

    Ok(Skill {
        id: dir_name.to_string(),
        name: dir_name.to_string(),
        description: None,
        source: skill_dir.to_string_lossy().to_string(),
        version: "1.0.0".to_string(),
        enabled_json: serde_json::to_string(&state).unwrap_or_default(),
        compatibility_json: "{}".to_string(),
        readme_url: None,
        repo_owner: None,
        repo_name: None,
        repo_branch: None,
        installed_at: 0,
    })
}

fn parse_skill_md_as_skill(
    skill_md: &Path,
    dir_name: &str,
    skill_dir: &Path,
    provider: &str,
) -> Result<Skill, String> {
    let content = fs::read_to_string(skill_md)
        .map_err(|e| format!("Failed to read SKILL.md: {e}"))?;
    let content = content.trim_start_matches('\u{feff}');

    let parts: Vec<&str> = content.splitn(3, "---").collect();
    let (name, description) = if parts.len() >= 3 {
        let front_matter = parts[1].trim();
        let mut name = None;
        let mut description = None;

        for line in front_matter.lines() {
            let line = line.trim();
            if let Some(value) = line.strip_prefix("name:") {
                name = Some(value.trim().to_string());
            } else if let Some(value) = line.strip_prefix("description:") {
                description = Some(value.trim().trim_matches('"').to_string());
            }
        }

        (
            name.unwrap_or_else(|| dir_name.to_string()),
            description,
        )
    } else {
        (dir_name.to_string(), None)
    };

    let mut state = agentdock_core::skills::SkillEnabledState::all_disabled();
    state.set_enabled(provider, true);

    Ok(Skill {
        id: dir_name.to_string(),
        name,
        description,
        source: skill_dir.to_string_lossy().to_string(),
        version: "1.0.0".to_string(),
        enabled_json: serde_json::to_string(&state).unwrap_or_default(),
        compatibility_json: "{}".to_string(),
        readme_url: None,
        repo_owner: None,
        repo_name: None,
        repo_branch: None,
        installed_at: 0,
    })
}

fn parse_skill_json_as_skill(
    skill_json: &Path,
    dir_name: &str,
    skill_dir: &Path,
    provider: &str,
) -> Result<Skill, String> {
    let content = fs::read_to_string(skill_json)
        .map_err(|e| format!("Failed to read skill.json: {e}"))?;

    #[derive(Deserialize)]
    struct SkillJson {
        name: Option<String>,
        description: Option<String>,
        version: Option<String>,
    }

    let meta: SkillJson = serde_json::from_str(&content)
        .unwrap_or(SkillJson {
            name: None,
            description: None,
            version: None,
        });

    let mut state = agentdock_core::skills::SkillEnabledState::all_disabled();
    state.set_enabled(provider, true);

    Ok(Skill {
        id: dir_name.to_string(),
        name: meta.name.unwrap_or_else(|| dir_name.to_string()),
        description: meta.description,
        source: skill_dir.to_string_lossy().to_string(),
        version: meta.version.unwrap_or_else(|| "1.0.0".to_string()),
        enabled_json: serde_json::to_string(&state).unwrap_or_default(),
        compatibility_json: "{}".to_string(),
        readme_url: None,
        repo_owner: None,
        repo_name: None,
        repo_branch: None,
        installed_at: 0,
    })
}

pub fn install_skill_from_path_cmd(
    ctx: &SkillsContext,
    source_path: &str,
) -> Result<Skill, String> {
    let source = PathBuf::from(source_path);

    if !source.exists() {
        return Err(format!("Source path does not exist: {source_path}"));
    }

    if !source.is_dir() {
        return Err(format!("Source path is not a directory: {source_path}"));
    }

    let metadata = parse_skill_metadata(&source)
        .map_err(|e| format!("Failed to parse skill metadata: {e}"))?;

    // Create skill record with all providers enabled by default
    let skill = create_skill_from_metadata(&metadata, source_path)
        .map_err(|e| format!("Failed to create skill from metadata: {e}"))?;

    // Save to database
    let conn = ctx.get_connection()?;
    insert_skill(&conn, &skill).map_err(|e| format!("Failed to insert skill: {e}"))?;

    // Copy to AgentDock skills directory (as master copy)
    let agentdock_skill_dir = get_agentdock_skills_dir().join(&metadata.id);
    fs::create_dir_all(&agentdock_skill_dir)
        .map_err(|e| format!("Failed to create skill directory: {e}"))?;
    copy_dir_all(&source, &agentdock_skill_dir)
        .map_err(|e| format!("Failed to copy skill files: {e}"))?;

    // Copy to all enabled provider directories
    let enabled_state = skill.get_enabled_state()
        .map_err(|e| format!("Failed to get enabled state: {e}"))?;

    for provider in ["claude_code", "codex", "opencode"] {
        if enabled_state.is_enabled_for(provider) {
            let provider_dir = get_provider_skills_dir(provider);
            let dest = provider_dir.join(&metadata.id);
            fs::create_dir_all(&provider_dir)
                .map_err(|e| format!("Failed to create provider directory: {e}"))?;
            if !dest.exists() {
                copy_dir_all(&source, &dest)
                    .map_err(|e| format!("Failed to copy skill to {provider}: {e}"))?;
            }
        }
    }

    Ok(skill)
}

pub fn install_skill_from_git_cmd(
    ctx: &SkillsContext,
    git_url: &str,
) -> Result<Skill, String> {
    let temp_dir = tempfile::tempdir()
        .map_err(|e| format!("Failed to create temp directory: {e}"))?;

    let status = std::process::Command::new("git")
        .args(["clone", "--depth", "1", git_url, temp_dir.path().to_str().unwrap()])
        .status()
        .map_err(|e| format!("Failed to run git clone: {e}"))?;

    if !status.success() {
        return Err(format!("Git clone failed for: {git_url}"));
    }

    let metadata = parse_skill_metadata(temp_dir.path())
        .map_err(|e| format!("Failed to parse skill metadata: {e}"))?;

    // Create skill record with all providers enabled by default
    let skill = create_skill_from_git_metadata(&metadata, git_url)
        .map_err(|e| format!("Failed to create skill from metadata: {e}"))?;

    // Save to database
    let conn = ctx.get_connection()?;
    insert_skill(&conn, &skill).map_err(|e| format!("Failed to insert skill: {e}"))?;

    // Copy to AgentDock skills directory (as master copy)
    let agentdock_skill_dir = get_agentdock_skills_dir().join(&metadata.id);
    fs::create_dir_all(&agentdock_skill_dir)
        .map_err(|e| format!("Failed to create skill directory: {e}"))?;
    copy_dir_all(temp_dir.path(), &agentdock_skill_dir)
        .map_err(|e| format!("Failed to copy skill files: {e}"))?;

    // Copy to all enabled provider directories
    let enabled_state = skill.get_enabled_state()
        .map_err(|e| format!("Failed to get enabled state: {e}"))?;

    for provider in ["claude_code", "codex", "opencode"] {
        if enabled_state.is_enabled_for(provider) {
            let provider_dir = get_provider_skills_dir(provider);
            let dest = provider_dir.join(&metadata.id);
            fs::create_dir_all(&provider_dir)
                .map_err(|e| format!("Failed to create provider directory: {e}"))?;
            if !dest.exists() {
                copy_dir_all(temp_dir.path(), &dest)
                    .map_err(|e| format!("Failed to copy skill to {provider}: {e}"))?;
            }
        }
    }

    Ok(skill)
}

/// Install a specific skill discovered from a repository
pub fn install_discovered_skill_cmd(
    ctx: &SkillsContext,
    discovered: &DiscoverableSkill,
    progress: &mut dyn FnMut(&str, &str),
) -> Result<Skill, String> {
    progress("downloading", "Downloading repository archive...");

    let temp_dir = tempfile::tempdir()
        .map_err(|e| format!("Failed to create temp directory: {e}"))?;

    // Download the repo as ZIP
    let repo_url = format!(
        "https://github.com/{}/{}/archive/refs/heads/{}.zip",
        discovered.repo_owner, discovered.repo_name, discovered.repo_branch
    );

    let response = reqwest::blocking::Client::new()
        .get(&repo_url)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .map_err(|e| format!("Failed to download: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Download failed with status: {}", response.status()));
    }

    let bytes = response
        .bytes()
        .map_err(|e| format!("Failed to read response: {e}"))?;

    let cursor = Cursor::new(bytes.as_ref());
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| format!("Failed to open ZIP: {e}"))?;

    if archive.is_empty() {
        return Err("Empty archive".to_string());
    }

    progress("extracting", "Extracting skill files...");

    // Get root directory name
    let root_name = {
        let first_file = archive
            .by_index(0)
            .map_err(|e| format!("Failed to read archive: {e}"))?;
        first_file
            .name()
            .split('/')
            .next()
            .unwrap_or("")
            .to_string()
    };

    // Extract the specific skill directory
    let skill_subdir = &discovered.directory;
    let skill_dir = temp_dir.path().join(skill_subdir);

    // Ensure the skill directory exists before extraction
    fs::create_dir_all(&skill_dir)
        .map_err(|e| format!("Failed to create skill directory: {e}"))?;

    // Extract files from the archive that are within the skill subdirectory
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read file {}: {}", i, e))?;
        let file_path = file.name();

        // Get relative path after root directory
        let relative_path = match file_path.strip_prefix(&format!("{root_name}/")) {
            Some(p) => p,
            None => continue,
        };

        // Check if this file is within the skill subdirectory
        if !relative_path.starts_with(&format!("{}/", skill_subdir)) {
            continue;
        }

        // Remove the skill subdirectory prefix to get the final path
        let final_relative = match relative_path.strip_prefix(&format!("{}/", skill_subdir)) {
            Some(p) => p,
            None => continue,
        };

        // Skip if final_relative is empty (the directory entry itself)
        if final_relative.is_empty() {
            continue;
        }

        let outpath = skill_dir.join(final_relative);

        if file.is_dir() {
            fs::create_dir_all(&outpath)
                .map_err(|e| format!("Failed to create directory: {e}"))?;
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create parent directory: {e}"))?;
            }
            let mut outfile = fs::File::create(&outpath)
                .map_err(|e| format!("Failed to create file: {e}"))?;
            std::io::copy(&mut file, &mut outfile)
                .map_err(|e| format!("Failed to write file: {e}"))?;
        }
    }

    // Parse skill metadata from the extracted directory
    progress("parsing_metadata", "Parsing skill metadata...");

    let metadata = parse_skill_metadata(&skill_dir)
        .map_err(|e| format!("Failed to parse skill metadata: {e}"))?;

    // Create skill record with all providers enabled by default
    let skill = create_skill_from_git_metadata(
        &metadata,
        &format!("https://github.com/{}/{}", discovered.repo_owner, discovered.repo_name),
    )
    .map_err(|e| format!("Failed to create skill from metadata: {e}"))?;

    // Update with repo info
    let mut skill = skill;
    skill.repo_owner = Some(discovered.repo_owner.clone());
    skill.repo_name = Some(discovered.repo_name.clone());
    skill.repo_branch = Some(discovered.repo_branch.clone());
    skill.readme_url = discovered.readme_url.clone();

    // Save to database
    progress("saving_record", "Saving skill record...");

    let conn = ctx.get_connection()?;
    insert_skill(&conn, &skill).map_err(|e| format!("Failed to insert skill: {e}"))?;

    // Copy to AgentDock skills directory (as master copy)
    progress("syncing_files", "Syncing skill files...");

    let agentdock_skill_dir = get_agentdock_skills_dir().join(&metadata.id);
    fs::create_dir_all(&agentdock_skill_dir)
        .map_err(|e| format!("Failed to create skill directory: {e}"))?;
    copy_dir_all(&skill_dir, &agentdock_skill_dir)
        .map_err(|e| format!("Failed to copy skill files: {e}"))?;

    // Copy to all enabled provider directories
    progress("syncing_providers", "Syncing provider directories...");

    let enabled_state = skill.get_enabled_state()
        .map_err(|e| format!("Failed to get enabled state: {e}"))?;

    for provider in ["claude_code", "codex", "opencode"] {
        if enabled_state.is_enabled_for(provider) {
            let provider_dir = get_provider_skills_dir(provider);
            let dest = provider_dir.join(&metadata.id);
            fs::create_dir_all(&provider_dir)
                .map_err(|e| format!("Failed to create provider directory: {e}"))?;
            if !dest.exists() {
                copy_dir_all(&skill_dir, &dest)
                    .map_err(|e| format!("Failed to copy skill to {provider}: {e}"))?;
            }
        }
    }

    Ok(skill)
}

pub fn toggle_skill_enabled_cmd(
    ctx: &SkillsContext,
    id: &str,
    enabled: bool,
) -> Result<(), String> {
    let conn = ctx.get_connection()?;
    update_skill_enabled(&conn, id, enabled)
        .map_err(|e| format!("Failed to update skill: {e}"))?;
    Ok(())
}

pub fn toggle_skill_enabled_for_provider_cmd(
    ctx: &SkillsContext,
    id: &str,
    provider: &str,
    enabled: bool,
) -> Result<(), String> {
    let conn = ctx.get_connection()?;

    eprintln!(
        "[SKILL] toggle_skill_enabled_for_provider_cmd: id={}, provider={}, enabled={}",
        id, provider, enabled
    );

    // Try to get skill from database, if not found, find it in provider directories
    let skill = match get_skill(&conn, id) {
        Ok(s) => {
            eprintln!("[SKILL] Found skill in database: {:?}", s.source);
            s
        }
        Err(_) => {
            // Skill not in database, find it in provider directories
            eprintln!("[SKILL] Skill not in database, searching provider directories...");
            let found = find_skill_in_provider_dirs(id)?;
            match found {
                Some((skill_path, found_provider)) => {
                    eprintln!(
                        "[SKILL] Found skill in provider directory: {:?}, provider={}",
                        skill_path, found_provider
                    );
                    // Create skill record from file system
                    let skill = parse_provider_skill_as_skill(&skill_path, id, &found_provider)?;
                    // Save to database
                    insert_skill(&conn, &skill)
                        .map_err(|e| format!("Failed to save skill to database: {e}"))?;
                    skill
                }
                None => {
                    eprintln!("[SKILL] Skill not found anywhere: {}", id);
                    return Err(format!("Skill not found: {id}"));
                }
            }
        }
    };

    // Find the skill source directory
    let source_dir = find_skill_source_dir(&skill);
    eprintln!("[SKILL] Source directory found: {:?}", source_dir);

    // Update database
    update_skill_enabled_for_provider(&conn, id, provider, enabled)
        .map_err(|e| format!("Failed to update skill for provider: {e}"))?;

    // Sync to/from provider directory
    if let Some(source) = source_dir {
        let provider_skills_dir = get_provider_skills_dir(provider);
        eprintln!("[SKILL] Provider skills dir: {:?}", provider_skills_dir);

        if enabled {
            // Copy skill to provider directory
            if source.exists() {
                let dest = provider_skills_dir.join(id);
                eprintln!("[SKILL] Copying from {:?} to {:?}", source, dest);
                fs::create_dir_all(&provider_skills_dir)
                    .map_err(|e| format!("Failed to create provider skills directory: {e}"))?;
                copy_dir_all(&source, &dest)
                    .map_err(|e| format!("Failed to copy skill to provider directory: {e}"))?;
                eprintln!("[SKILL] Successfully copied skill to {}", provider);
            } else {
                eprintln!("[SKILL] Source directory does not exist: {:?}", source);
            }
        } else {
            // Remove skill from provider directory
            let dest = provider_skills_dir.join(id);
            eprintln!("[SKILL] Removing from {:?}", dest);
            if dest.exists() {
                fs::remove_dir_all(&dest)
                    .map_err(|e| format!("Failed to remove skill from provider directory: {e}"))?;
                eprintln!("[SKILL] Successfully removed skill from {}", provider);
            }
        }
    } else {
        eprintln!("[SKILL] No source directory found for skill: {}", id);
    }

    Ok(())
}

fn find_skill_in_provider_dirs(skill_id: &str) -> Result<Option<(PathBuf, String)>, String> {
    for (provider, skills_dir) in get_provider_skills_dirs() {
        let skill_path = skills_dir.join(skill_id);
        if skill_path.exists() {
            return Ok(Some((skill_path, provider.to_string())));
        }
    }
    Ok(None)
}

fn find_skill_source_dir(skill: &Skill) -> Option<PathBuf> {
    // Check if source path exists
    let source = PathBuf::from(&skill.source);
    if source.exists() {
        return Some(source);
    }

    // Check in AgentDock skills directory
    let agentdock_skill_dir = get_agentdock_skills_dir().join(&skill.id);
    if agentdock_skill_dir.exists() {
        return Some(agentdock_skill_dir);
    }

    // Check in provider directories
    for (_, provider_dir) in get_provider_skills_dirs() {
        let skill_dir = provider_dir.join(&skill.id);
        if skill_dir.exists() {
            return Some(skill_dir);
        }
    }

    None
}

fn get_provider_skills_dir(provider: &str) -> PathBuf {
    let home = dirs::home_dir().unwrap_or_default();

    match provider {
        "claude_code" => home.join(".claude").join("skills"),
        "codex" => home.join(".codex").join("skills"),
        "opencode" => home.join(".config").join("opencode").join("skills"),
        _ => home.join(".claude").join("skills"), // Default to claude
    }
}

fn get_provider_skills_dirs() -> Vec<(&'static str, PathBuf)> {
    let home = dirs::home_dir().unwrap_or_default();

    vec![
        ("claude_code", home.join(".claude").join("skills")),
        ("codex", home.join(".codex").join("skills")),
        ("opencode", home.join(".config").join("opencode").join("skills")),
    ]
}

fn get_agentdock_skills_dir() -> PathBuf {
    // Return the AgentDock skills directory
    dirs::home_dir()
        .unwrap_or_default()
        .join(".agentdock")
        .join("skills")
}

pub fn uninstall_skill_cmd(ctx: &SkillsContext, id: &str) -> Result<(), String> {
    let conn = ctx.get_connection()?;
    delete_skill(&conn, id).map_err(|e| format!("Failed to delete skill from database: {e}"))?;

    let skill_dir = ctx.skills_dir.join(id);
    if skill_dir.exists() {
        fs::remove_dir_all(&skill_dir)
            .map_err(|e| format!("Failed to remove skill directory: {e}"))?;
    }

    Ok(())
}

pub fn get_skill_cmd(ctx: &SkillsContext, id: &str) -> Result<Skill, String> {
    let conn = ctx.get_connection()?;
    get_skill(&conn, id).map_err(|e| format!("Failed to get skill: {e}"))
}

// Skill repository management

pub fn list_skill_repos_cmd(ctx: &SkillsContext) -> Result<Vec<SkillRepo>, String> {
    let conn = ctx.get_connection()?;
    list_skill_repos(&conn).map_err(|e| format!("Failed to list skill repos: {e}"))
}

pub fn add_skill_repo_cmd(
    ctx: &SkillsContext,
    owner: &str,
    name: &str,
    branch: &str,
) -> Result<SkillRepo, String> {
    let conn = ctx.get_connection()?;

    let id = format!("{}/{}", owner, name);
    let repo = SkillRepo {
        id: id.clone(),
        owner: owner.to_string(),
        name: name.to_string(),
        branch: branch.to_string(),
        enabled: true,
        created_at: chrono::Utc::now().timestamp_millis(),
    };

    insert_skill_repo(&conn, &repo).map_err(|e| format!("Failed to add skill repo: {e}"))?;

    Ok(repo)
}

pub fn remove_skill_repo_cmd(ctx: &SkillsContext, id: &str) -> Result<(), String> {
    let conn = ctx.get_connection()?;
    delete_skill_repo(&conn, id).map_err(|e| format!("Failed to remove skill repo: {e}"))
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

// Skill discovery functions

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiscoverCache {
    skills: Vec<DiscoverableSkill>,
    cached_at: i64,
    repo_ids: Vec<String>,
}

const CACHE_TTL_SECS: i64 = 3600; // 1 hour

fn get_discover_cache_path(ctx: &SkillsContext) -> PathBuf {
    ctx.skills_dir.parent().unwrap_or(&ctx.skills_dir).join("discover_cache.json")
}

fn load_discover_cache(ctx: &SkillsContext) -> Option<DiscoverCache> {
    let cache_path = get_discover_cache_path(ctx);
    if !cache_path.exists() {
        return None;
    }

    let content = fs::read_to_string(&cache_path).ok()?;
    let cache: DiscoverCache = serde_json::from_str(&content).ok()?;

    // Check if cache is still valid (within TTL)
    let now = chrono::Utc::now().timestamp();
    if now - cache.cached_at > CACHE_TTL_SECS {
        return None;
    }

    Some(cache)
}

fn save_discover_cache(ctx: &SkillsContext, cache: &DiscoverCache) {
    let cache_path = get_discover_cache_path(ctx);
    if let Ok(content) = serde_json::to_string_pretty(cache) {
        let _ = fs::write(&cache_path, content);
    }
}

pub fn discover_skills_cmd(ctx: &SkillsContext) -> Result<Vec<DiscoverableSkill>, String> {
    discover_skills_cmd_with_cache(ctx, false)
}

pub fn discover_skills_cmd_with_cache(ctx: &SkillsContext, force_refresh: bool) -> Result<Vec<DiscoverableSkill>, String> {
    let conn = ctx.get_connection()?;
    let repos = list_skill_repos(&conn).map_err(|e| format!("Failed to list repos: {e}"))?;

    let repo_ids: Vec<String> = repos.iter().filter(|r| r.enabled).map(|r| r.id.clone()).collect();

    // Try to load from cache if not forcing refresh
    if !force_refresh {
        if let Some(cache) = load_discover_cache(ctx) {
            // Check if repos haven't changed
            if cache.repo_ids == repo_ids {
                eprintln!("[SKILL] Using cached discover results ({} skills)", cache.skills.len());
                return Ok(cache.skills);
            }
        }
    }

    // Fetch fresh data
    let mut all_skills = Vec::new();

    for repo in &repos {
        if !repo.enabled {
            continue;
        }

        match discover_repo_skills(repo) {
            Ok(skills) => all_skills.extend(skills),
            Err(e) => {
                eprintln!("Failed to discover skills from {}: {}", repo.id, e);
                // Continue with other repos
            }
        }
    }

    // Sort by name
    all_skills.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    // Save to cache
    let cache = DiscoverCache {
        skills: all_skills.clone(),
        cached_at: chrono::Utc::now().timestamp(),
        repo_ids,
    };
    save_discover_cache(ctx, &cache);
    eprintln!("[SKILL] Cached discover results ({} skills)", all_skills.len());

    Ok(all_skills)
}

fn discover_repo_skills(repo: &SkillRepo) -> Result<Vec<DiscoverableSkill>, String> {
    let temp_dir = tempfile::tempdir()
        .map_err(|e| format!("Failed to create temp directory: {e}"))?;
    let temp_path = temp_dir.path().to_path_buf();

    // Try multiple branches
    let branches = if repo.branch.is_empty() {
        vec!["main", "master"]
    } else {
        vec![repo.branch.as_str(), "main", "master"]
    };

    let mut last_error = None;

    for branch in branches {
        let url = format!(
            "https://github.com/{}/{}/archive/refs/heads/{}.zip",
            repo.owner, repo.name, branch
        );

        match download_and_extract_zip(&url, &temp_path) {
            Ok(_) => {
                let mut skills = Vec::new();
                scan_dir_for_skills(&temp_path, &temp_path, repo, &mut skills)?;
                let _ = fs::remove_dir_all(&temp_path);
                return Ok(skills);
            }
            Err(e) => {
                last_error = Some(e);
                continue;
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "All branches failed".to_string()))
}

fn download_and_extract_zip(url: &str, dest: &Path) -> Result<(), String> {
    let response = reqwest::blocking::Client::new()
        .get(url)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .map_err(|e| format!("Failed to download: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Download failed with status: {}", response.status()));
    }

    let bytes = response
        .bytes()
        .map_err(|e| format!("Failed to read response: {e}"))?;

    let cursor = Cursor::new(bytes.as_ref());
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| format!("Failed to open ZIP: {e}"))?;

    if archive.is_empty() {
        return Err("Empty archive".to_string());
    }

    // Get root directory name first
    let root_name = {
        let first_file = archive
            .by_index(0)
            .map_err(|e| format!("Failed to read archive: {e}"))?;
        first_file
            .name()
            .split('/')
            .next()
            .unwrap_or("")
            .to_string()
    };

    // Now iterate through all files
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read file {}: {}", i, e))?;
        let file_path = file.name();

        let relative_path = if let Some(stripped) = file_path.strip_prefix(&format!("{root_name}/"))
        {
            stripped
        } else {
            continue;
        };

        if relative_path.is_empty() {
            continue;
        }

        let outpath = dest.join(relative_path);

        if file.is_dir() {
            fs::create_dir_all(&outpath)
                .map_err(|e| format!("Failed to create directory: {e}"))?;
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create parent directory: {e}"))?;
            }
            let mut outfile = fs::File::create(&outpath)
                .map_err(|e| format!("Failed to create file: {e}"))?;
            std::io::copy(&mut file, &mut outfile)
                .map_err(|e| format!("Failed to write file: {e}"))?;
        }
    }

    Ok(())
}

fn scan_dir_for_skills(
    current_dir: &Path,
    base_dir: &Path,
    repo: &SkillRepo,
    skills: &mut Vec<DiscoverableSkill>,
) -> Result<(), String> {
    // Check for SKILL.md in current directory
    let skill_md = current_dir.join("SKILL.md");
    if skill_md.exists() {
        let directory = if current_dir == base_dir {
            repo.name.clone()
        } else {
            current_dir
                .strip_prefix(base_dir)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| repo.name.clone())
        };

        if let Ok(skill) = build_skill_from_skill_md(&skill_md, &directory, repo) {
            skills.push(skill);
        }
        return Ok(());
    }

    // Also check for skill.json
    let skill_json = current_dir.join("skill.json");
    if skill_json.exists() {
        let directory = if current_dir == base_dir {
            repo.name.clone()
        } else {
            current_dir
                .strip_prefix(base_dir)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| repo.name.clone())
        };

        if let Ok(skill) = build_skill_from_skill_json(&skill_json, &directory, repo) {
            skills.push(skill);
        }
        return Ok(());
    }

    // Recursively scan subdirectories
    let entries = fs::read_dir(current_dir)
        .map_err(|e| format!("Failed to read directory: {e}"))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
        let path = entry.path();

        if path.is_dir() {
            // Skip hidden directories
            let dir_name = entry.file_name().to_string_lossy().to_string();
            if dir_name.starts_with('.') {
                continue;
            }
            scan_dir_for_skills(&path, base_dir, repo, skills)?;
        }
    }

    Ok(())
}

fn build_skill_from_skill_md(
    skill_md: &Path,
    directory: &str,
    repo: &SkillRepo,
) -> Result<DiscoverableSkill, String> {
    let content = fs::read_to_string(skill_md)
        .map_err(|e| format!("Failed to read SKILL.md: {e}"))?;
    let content = content.trim_start_matches('\u{feff}');

    // Parse YAML front matter
    let parts: Vec<&str> = content.splitn(3, "---").collect();

    let (name, description) = if parts.len() >= 3 {
        // Try to parse front matter as simple key:value pairs
        let front_matter = parts[1].trim();
        let mut name = None;
        let mut description = None;

        for line in front_matter.lines() {
            let line = line.trim();
            if let Some(value) = line.strip_prefix("name:") {
                name = Some(value.trim().to_string());
            } else if let Some(value) = line.strip_prefix("description:") {
                description = Some(value.trim().trim_matches('"').to_string());
            }
        }

        (
            name.unwrap_or_else(|| directory.to_string()),
            description.unwrap_or_default(),
        )
    } else {
        (directory.to_string(), String::new())
    };

    Ok(DiscoverableSkill {
        key: format!("{}/{}:{}", repo.owner, repo.name, directory),
        name,
        description,
        directory: directory.to_string(),
        readme_url: Some(format!(
            "https://github.com/{}/{}/tree/{}/{}",
            repo.owner, repo.name, repo.branch, directory
        )),
        repo_owner: repo.owner.clone(),
        repo_name: repo.name.clone(),
        repo_branch: repo.branch.clone(),
    })
}

fn build_skill_from_skill_json(
    skill_json: &Path,
    directory: &str,
    repo: &SkillRepo,
) -> Result<DiscoverableSkill, String> {
    let content = fs::read_to_string(skill_json)
        .map_err(|e| format!("Failed to read skill.json: {e}"))?;

    #[derive(Deserialize)]
    struct SkillJson {
        name: Option<String>,
        description: Option<String>,
    }

    let meta: SkillJson = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse skill.json: {e}"))?;

    Ok(DiscoverableSkill {
        key: format!("{}/{}:{}", repo.owner, repo.name, directory),
        name: meta.name.unwrap_or_else(|| directory.to_string()),
        description: meta.description.unwrap_or_default(),
        directory: directory.to_string(),
        readme_url: Some(format!(
            "https://github.com/{}/{}/tree/{}/{}",
            repo.owner, repo.name, repo.branch, directory
        )),
        repo_owner: repo.owner.clone(),
        repo_name: repo.name.clone(),
        repo_branch: repo.branch.clone(),
    })
}

// Provider skill discovery - scan existing skills from provider directories

/// Provider skill found in provider's skills directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSkill {
    pub key: String,
    pub name: String,
    pub description: String,
    pub directory: String,
    pub provider: String,
    pub path: String,
}

/// Scan provider directories for existing skills
pub fn scan_provider_skills_cmd(ctx: &SkillsContext) -> Result<Vec<ProviderSkill>, String> {
    let conn = ctx.get_connection()?;
    let installed_skills = list_skills(&conn).map_err(|e| format!("Failed to list skills: {e}"))?;
    let installed_dirs: std::collections::HashSet<String> = installed_skills
        .iter()
        .map(|s| s.id.clone())
        .collect();

    let mut found_skills = Vec::new();

    for (provider, skills_dir) in get_provider_skills_dirs() {
        if !skills_dir.exists() {
            continue;
        }

        let entries = fs::read_dir(&skills_dir)
            .map_err(|e| format!("Failed to read directory: {e}"))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let dir_name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden directories
            if dir_name.starts_with('.') {
                continue;
            }

            // Skip already installed skills
            if installed_dirs.contains(&dir_name) {
                continue;
            }

            // Try to parse skill metadata
            if let Ok(skill) = parse_provider_skill(&path, &dir_name, provider) {
                found_skills.push(skill);
            }
        }
    }

    // Sort by name
    found_skills.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Ok(found_skills)
}

fn parse_provider_skill(skill_dir: &Path, dir_name: &str, provider: &str) -> Result<ProviderSkill, String> {
    // Try SKILL.md first
    let skill_md = skill_dir.join("SKILL.md");
    if skill_md.exists() {
        return parse_skill_md_file(&skill_md, dir_name, provider, skill_dir);
    }

    // Try skill.json
    let skill_json = skill_dir.join("skill.json");
    if skill_json.exists() {
        return parse_skill_json_file(&skill_json, dir_name, provider, skill_dir);
    }

    Err(format!("No SKILL.md or skill.json found in {}", skill_dir.display()))
}

fn parse_skill_md_file(
    skill_md: &Path,
    dir_name: &str,
    provider: &str,
    skill_dir: &Path,
) -> Result<ProviderSkill, String> {
    let content = fs::read_to_string(skill_md)
        .map_err(|e| format!("Failed to read SKILL.md: {e}"))?;
    let content = content.trim_start_matches('\u{feff}');

    // Parse YAML front matter
    let parts: Vec<&str> = content.splitn(3, "---").collect();

    let (name, description) = if parts.len() >= 3 {
        let front_matter = parts[1].trim();
        let mut name = None;
        let mut description = None;

        for line in front_matter.lines() {
            let line = line.trim();
            if let Some(value) = line.strip_prefix("name:") {
                name = Some(value.trim().to_string());
            } else if let Some(value) = line.strip_prefix("description:") {
                description = Some(value.trim().trim_matches('"').to_string());
            }
        }

        (
            name.unwrap_or_else(|| dir_name.to_string()),
            description.unwrap_or_default(),
        )
    } else {
        (dir_name.to_string(), String::new())
    };

    Ok(ProviderSkill {
        key: format!("local:{}", dir_name),
        name,
        description,
        directory: dir_name.to_string(),
        provider: provider.to_string(),
        path: skill_dir.to_string_lossy().to_string(),
    })
}

fn parse_skill_json_file(
    skill_json: &Path,
    dir_name: &str,
    provider: &str,
    skill_dir: &Path,
) -> Result<ProviderSkill, String> {
    let content = fs::read_to_string(skill_json)
        .map_err(|e| format!("Failed to read skill.json: {e}"))?;

    #[derive(Deserialize)]
    struct SkillJson {
        name: Option<String>,
        description: Option<String>,
    }

    let meta: SkillJson = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse skill.json: {e}"))?;

    Ok(ProviderSkill {
        key: format!("local:{}", dir_name),
        name: meta.name.unwrap_or_else(|| dir_name.to_string()),
        description: meta.description.unwrap_or_default(),
        directory: dir_name.to_string(),
        provider: provider.to_string(),
        path: skill_dir.to_string_lossy().to_string(),
    })
}

/// Import skills from provider directories
pub fn import_provider_skills_cmd(
    ctx: &SkillsContext,
    skill_keys: Vec<String>,
) -> Result<Vec<Skill>, String> {
    let conn = ctx.get_connection()?;
    let mut imported = Vec::new();

    for key in skill_keys {
        // Find the skill in provider directories
        let found = find_provider_skill_by_key(&key)?;

        if let Some((provider_skill, provider)) = found {
            // Copy skill to AgentDock skills directory
            let skill_dir = ctx.skills_dir.join(&provider_skill.directory);
            if skill_dir.exists() {
                fs::remove_dir_all(&skill_dir)
                    .map_err(|e| format!("Failed to remove existing skill directory: {e}"))?;
            }

            fs::create_dir_all(&skill_dir)
                .map_err(|e| format!("Failed to create skill directory: {e}"))?;

            let source_path = PathBuf::from(&provider_skill.path);
            copy_dir_all(&source_path, &skill_dir)
                .map_err(|e| format!("Failed to copy skill files: {e}"))?;

            // Create skill record
            let skill = Skill {
                id: provider_skill.directory.clone(),
                name: provider_skill.name.clone(),
                description: if provider_skill.description.is_empty() {
                    None
                } else {
                    Some(provider_skill.description.clone())
                },
                source: provider_skill.path.clone(),
                version: "1.0.0".to_string(),
                enabled_json: serde_json::to_string(&agentdock_core::skills::SkillEnabledState::all_enabled()).unwrap_or_default(),
                compatibility_json: "{}".to_string(),
                readme_url: None,
                repo_owner: None,
                repo_name: None,
                repo_branch: None,
                installed_at: chrono::Utc::now().timestamp_millis(),
            };

            insert_skill(&conn, &skill)
                .map_err(|e| format!("Failed to insert skill: {e}"))?;

            imported.push(skill);
        }
    }

    Ok(imported)
}

fn find_provider_skill_by_key(key: &str) -> Result<Option<(ProviderSkill, String)>, String> {
    // Parse key format: "local:directory"
    if let Some(directory) = key.strip_prefix("local:") {
        for (provider, skills_dir) in get_provider_skills_dirs() {
            let skill_path = skills_dir.join(directory);
            if skill_path.exists() {
                if let Ok(skill) = parse_provider_skill(&skill_path, directory, provider) {
                    return Ok(Some((skill, provider.to_string())));
                }
            }
        }
    }
    Ok(None)
}
