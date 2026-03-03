-- Enhanced skills table with per-provider enable/disable and additional metadata
-- This replaces the single 'enabled' column with 'enabled_json' for per-provider toggles

-- Add new columns to skills table
ALTER TABLE skills ADD COLUMN description TEXT;
ALTER TABLE skills ADD COLUMN readme_url TEXT;
ALTER TABLE skills ADD COLUMN repo_owner TEXT;
ALTER TABLE skills ADD COLUMN repo_name TEXT;
ALTER TABLE skills ADD COLUMN repo_branch TEXT;
ALTER TABLE skills ADD COLUMN installed_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000);
ALTER TABLE skills ADD COLUMN enabled_json TEXT NOT NULL DEFAULT '{"claude_code":true,"codex":true,"opencode":true}';

-- Create a new table with the updated schema (SQLite doesn't support dropping columns easily)
-- We'll migrate data in the application layer if needed

-- Skills repositories for discovery
CREATE TABLE IF NOT EXISTS skill_repos (
  id TEXT PRIMARY KEY,
  owner TEXT NOT NULL,
  name TEXT NOT NULL,
  branch TEXT NOT NULL DEFAULT 'main',
  enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
  created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000)
);
