CREATE TABLE IF NOT EXISTS analysis_results (
  analysis_id TEXT PRIMARY KEY,
  page_id TEXT NOT NULL,
  schema_version TEXT NOT NULL,
  provider TEXT NOT NULL,
  model_name TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('succeeded', 'failed')),
  result_json TEXT,
  error_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (page_id) REFERENCES page_records(page_id),
  FOREIGN KEY (error_id) REFERENCES errors(error_id),
  UNIQUE(page_id)
);

CREATE INDEX IF NOT EXISTS idx_analysis_results_page_id
  ON analysis_results(page_id);
