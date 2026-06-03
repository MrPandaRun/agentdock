-- Dock Agent orchestration foundation.
-- Adds durable task, schedule, chat connector, remote command, and audit tables.

CREATE TABLE IF NOT EXISTS dock_agent_tasks (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK(kind IN ('orchestration', 'scheduled_check', 'chat_command', 'remote_command')),
  status TEXT NOT NULL CHECK(status IN ('queued', 'running', 'succeeded', 'failed', 'canceled')),
  title TEXT NOT NULL,
  objective TEXT NOT NULL,
  provider_id TEXT,
  target_thread_id TEXT,
  schedule_id TEXT,
  chat_connector_id TEXT,
  remote_command_id TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  started_at TEXT,
  completed_at TEXT,
  FOREIGN KEY(provider_id) REFERENCES providers(id) ON DELETE SET NULL,
  FOREIGN KEY(target_thread_id) REFERENCES threads(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS dock_agent_schedules (
  id TEXT PRIMARY KEY,
  task_id TEXT,
  cadence TEXT NOT NULL CHECK(cadence IN ('hourly', 'weekly', 'cron')),
  cron_expression TEXT,
  timezone TEXT NOT NULL DEFAULT 'UTC',
  enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
  next_run_at TEXT,
  last_run_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY(task_id) REFERENCES dock_agent_tasks(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS dock_agent_chat_connectors (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  kind TEXT NOT NULL CHECK(kind IN ('webhook')),
  enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
  config_json TEXT NOT NULL DEFAULT '{}',
  secret_ref TEXT,
  last_seen_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS dock_agent_remote_commands (
  id TEXT PRIMARY KEY,
  task_id TEXT,
  device_id TEXT,
  status TEXT NOT NULL CHECK(status IN ('requested', 'approved', 'running', 'succeeded', 'failed', 'rejected')),
  command TEXT NOT NULL,
  args_json TEXT NOT NULL DEFAULT '[]',
  working_dir TEXT,
  policy_json TEXT NOT NULL DEFAULT '{}',
  requested_by TEXT NOT NULL,
  approved_at TEXT,
  executed_at TEXT,
  result_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY(task_id) REFERENCES dock_agent_tasks(id) ON DELETE SET NULL,
  FOREIGN KEY(device_id) REFERENCES remote_devices(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS dock_agent_audit_logs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  actor TEXT NOT NULL,
  action TEXT NOT NULL,
  subject_type TEXT NOT NULL,
  subject_id TEXT,
  outcome TEXT NOT NULL CHECK(outcome IN ('allowed', 'denied', 'succeeded', 'failed')),
  details_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_dock_agent_tasks_status_updated_at
ON dock_agent_tasks(status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_dock_agent_tasks_provider_thread
ON dock_agent_tasks(provider_id, target_thread_id);

CREATE INDEX IF NOT EXISTS idx_dock_agent_schedules_next_run
ON dock_agent_schedules(enabled, next_run_at);

CREATE INDEX IF NOT EXISTS idx_dock_agent_remote_commands_status_created_at
ON dock_agent_remote_commands(status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_dock_agent_audit_logs_created_at
ON dock_agent_audit_logs(created_at DESC);
