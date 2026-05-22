CREATE TABLE IF NOT EXISTS index_versions (
  version_id TEXT PRIMARY KEY,
  provider TEXT NOT NULL,
  analyzer_version TEXT NOT NULL,
  status TEXT NOT NULL CHECK (
    status IN ('not_built', 'building', 'ready', 'failed')
  ),
  index_directory TEXT NOT NULL,
  document_count INTEGER NOT NULL DEFAULT 0,
  build_started_at TEXT,
  build_finished_at TEXT,
  activated_at TEXT,
  error_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (error_id) REFERENCES errors(error_id)
);

CREATE TABLE IF NOT EXISTS index_active (
  provider TEXT PRIMARY KEY,
  version_id TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (version_id) REFERENCES index_versions(version_id)
);

CREATE INDEX IF NOT EXISTS idx_index_versions_status_updated
  ON index_versions(status, updated_at);
