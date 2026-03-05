-- MCP management enhancements:
-- - Expand legacy mcps schema with transport/target/metadata fields.
-- - Add operation log table for audit trails.

ALTER TABLE mcps ADD COLUMN transport TEXT NOT NULL DEFAULT 'stdio';
ALTER TABLE mcps ADD COLUMN target TEXT NOT NULL DEFAULT '';
ALTER TABLE mcps ADD COLUMN headers_json TEXT NOT NULL DEFAULT '{}';
ALTER TABLE mcps ADD COLUMN env_json TEXT NOT NULL DEFAULT '{}';
ALTER TABLE mcps ADD COLUMN secret_json TEXT NOT NULL DEFAULT '{}';
ALTER TABLE mcps ADD COLUMN created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
ALTER TABLE mcps ADD COLUMN updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
ALTER TABLE mcps ADD COLUMN last_tested_at TEXT;
ALTER TABLE mcps ADD COLUMN last_test_status TEXT;
ALTER TABLE mcps ADD COLUMN last_test_message TEXT;
ALTER TABLE mcps ADD COLUMN last_test_duration_ms INTEGER;

-- Backfill new target field from legacy command field when possible.
UPDATE mcps
SET target = command
WHERE trim(COALESCE(target, '')) = '' AND trim(COALESCE(command, '')) != '';

CREATE TABLE IF NOT EXISTS mcp_operation_logs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  mcp_id TEXT,
  action TEXT NOT NULL,
  actor TEXT NOT NULL,
  details_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY(mcp_id) REFERENCES mcps(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_mcp_operation_logs_created_at
ON mcp_operation_logs(created_at DESC);
