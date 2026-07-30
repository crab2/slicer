ALTER TABLE page_records ADD COLUMN pdf_width_points REAL;
ALTER TABLE page_records ADD COLUMN pdf_height_points REAL;
ALTER TABLE page_records ADD COLUMN crop_left_points REAL;
ALTER TABLE page_records ADD COLUMN crop_bottom_points REAL;
ALTER TABLE page_records ADD COLUMN crop_right_points REAL;
ALTER TABLE page_records ADD COLUMN crop_top_points REAL;
ALTER TABLE page_records ADD COLUMN rotation_degrees INTEGER NOT NULL DEFAULT 0 CHECK (
  rotation_degrees IN (0, 90, 180, 270)
);
ALTER TABLE page_records ADD COLUMN preview_width_px INTEGER;
ALTER TABLE page_records ADD COLUMN preview_height_px INTEGER;

ALTER TABLE index_versions ADD COLUMN content_schema_version TEXT NOT NULL DEFAULT 'page_v1';
ALTER TABLE index_versions ADD COLUMN content_fingerprint TEXT NOT NULL DEFAULT '';

CREATE TABLE document_artifacts (
  artifact_id TEXT PRIMARY KEY,
  document_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (
    kind IN ('canonical_pdf', 'pdf_structure_json', 'pdf_structure_image')
  ),
  relative_path TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  parser_name TEXT,
  parser_version TEXT,
  parser_options_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (document_id) REFERENCES documents(document_id) ON DELETE CASCADE,
  UNIQUE(document_id, kind, relative_path)
);

CREATE UNIQUE INDEX idx_document_artifacts_singletons
  ON document_artifacts(document_id, kind)
  WHERE kind IN ('canonical_pdf', 'pdf_structure_json');

CREATE INDEX idx_document_artifacts_document_kind
  ON document_artifacts(document_id, kind);

CREATE TABLE pdf_parse_runs (
  parse_id TEXT PRIMARY KEY,
  document_id TEXT NOT NULL,
  parser_name TEXT NOT NULL,
  parser_version TEXT NOT NULL,
  schema_version TEXT NOT NULL,
  parser_options_json TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed')),
  raw_json_path TEXT,
  error_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (document_id) REFERENCES documents(document_id) ON DELETE CASCADE,
  FOREIGN KEY (error_id) REFERENCES errors(error_id)
);

CREATE INDEX idx_pdf_parse_runs_document_status
  ON pdf_parse_runs(document_id, status, updated_at);

CREATE TABLE content_blocks (
  block_id TEXT PRIMARY KEY,
  parse_id TEXT NOT NULL,
  document_id TEXT NOT NULL,
  page_id TEXT NOT NULL,
  page_number INTEGER NOT NULL CHECK (page_number > 0),
  parent_block_id TEXT,
  source_element_id TEXT,
  ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
  block_type TEXT NOT NULL,
  source_text TEXT NOT NULL DEFAULT '',
  enrichment_json TEXT,
  raw_json TEXT NOT NULL,
  source_image_path TEXT,
  is_indexable INTEGER NOT NULL DEFAULT 1 CHECK (is_indexable IN (0, 1)),
  is_visual INTEGER NOT NULL DEFAULT 0 CHECK (is_visual IN (0, 1)),
  is_decorative INTEGER NOT NULL DEFAULT 0 CHECK (is_decorative IN (0, 1)),
  bbox_x REAL,
  bbox_y REAL,
  bbox_width REAL,
  bbox_height REAL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (parse_id) REFERENCES pdf_parse_runs(parse_id) ON DELETE CASCADE,
  FOREIGN KEY (document_id) REFERENCES documents(document_id) ON DELETE CASCADE,
  FOREIGN KEY (page_id) REFERENCES page_records(page_id) ON DELETE CASCADE,
  FOREIGN KEY (parent_block_id) REFERENCES content_blocks(block_id) ON DELETE CASCADE,
  CHECK (
    (bbox_x IS NULL AND bbox_y IS NULL AND bbox_width IS NULL AND bbox_height IS NULL)
    OR
    (bbox_x IS NOT NULL AND bbox_y IS NOT NULL AND bbox_width IS NOT NULL AND bbox_height IS NOT NULL
      AND bbox_x >= 0.0 AND bbox_y >= 0.0 AND bbox_width > 0.0 AND bbox_height > 0.0
      AND bbox_x <= 1.0 AND bbox_y <= 1.0
      AND bbox_x + bbox_width <= 1.000001
      AND bbox_y + bbox_height <= 1.000001)
  )
);

CREATE INDEX idx_content_blocks_document_page_ordinal
  ON content_blocks(document_id, page_number, ordinal);
CREATE INDEX idx_content_blocks_page
  ON content_blocks(page_id);
CREATE INDEX idx_content_blocks_parse
  ON content_blocks(parse_id);
CREATE INDEX idx_content_blocks_visual_queue
  ON content_blocks(is_visual, is_decorative, page_id);

CREATE TABLE visual_module_analysis (
  analysis_id TEXT PRIMARY KEY,
  block_id TEXT NOT NULL UNIQUE,
  schema_version TEXT NOT NULL,
  provider TEXT NOT NULL,
  model_name TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('pending', 'succeeded', 'failed', 'skipped')),
  result_json TEXT,
  error_id TEXT,
  attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (block_id) REFERENCES content_blocks(block_id) ON DELETE CASCADE,
  FOREIGN KEY (error_id) REFERENCES errors(error_id)
);

CREATE INDEX idx_visual_module_analysis_status
  ON visual_module_analysis(status, updated_at);
