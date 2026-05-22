CREATE TABLE IF NOT EXISTS documents (
  document_id TEXT PRIMARY KEY,
  original_filename TEXT NOT NULL,
  file_type TEXT NOT NULL,
  file_hash TEXT NOT NULL,
  original_path TEXT NOT NULL,
  page_count INTEGER,
  status TEXT NOT NULL CHECK (
    status IN ('pending', 'importing', 'ready', 'failed')
  ),
  error_summary TEXT,
  job_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (job_id) REFERENCES jobs(job_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_documents_file_hash
  ON documents(file_hash);

CREATE TABLE IF NOT EXISTS image_assets (
  image_hash TEXT PRIMARY KEY,
  file_path TEXT NOT NULL,
  file_size INTEGER NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS page_records (
  page_id TEXT PRIMARY KEY,
  document_id TEXT NOT NULL,
  page_number INTEGER NOT NULL,
  image_hash TEXT NOT NULL,
  status TEXT NOT NULL CHECK (
    status IN ('pending', 'rendered', 'analysis_pending', 'analyzed', 'failed')
  ),
  error_summary TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (document_id) REFERENCES documents(document_id),
  FOREIGN KEY (image_hash) REFERENCES image_assets(image_hash)
);

CREATE INDEX IF NOT EXISTS idx_page_records_document_id
  ON page_records(document_id);

CREATE INDEX IF NOT EXISTS idx_page_records_image_hash
  ON page_records(image_hash);
