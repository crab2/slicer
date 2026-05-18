CREATE TABLE IF NOT EXISTS settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS errors (
  error_id TEXT PRIMARY KEY,
  code TEXT NOT NULL,
  message TEXT NOT NULL,
  stage TEXT NOT NULL,
  retryable INTEGER NOT NULL CHECK (retryable IN (0, 1)),
  details TEXT,
  correlation_id TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS jobs (
  job_id TEXT PRIMARY KEY,
  job_type TEXT NOT NULL,
  status TEXT NOT NULL CHECK (
    status IN ('queued', 'running', 'succeeded', 'failed', 'cancelled')
  ),
  progress INTEGER NOT NULL DEFAULT 0 CHECK (progress >= 0 AND progress <= 100),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  error_id TEXT,
  error_summary TEXT,
  FOREIGN KEY (error_id) REFERENCES errors(error_id)
);

CREATE INDEX IF NOT EXISTS idx_jobs_status_updated_at
  ON jobs(status, updated_at);

CREATE INDEX IF NOT EXISTS idx_errors_correlation_id
  ON errors(correlation_id);
