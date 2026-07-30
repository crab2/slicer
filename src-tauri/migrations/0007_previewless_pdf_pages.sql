CREATE TABLE page_records_new (
  page_id TEXT PRIMARY KEY,
  document_id TEXT NOT NULL,
  page_number INTEGER NOT NULL,
  image_hash TEXT,
  status TEXT NOT NULL CHECK (
    status IN ('pending', 'rendered', 'structured', 'analysis_pending', 'analyzed', 'failed')
  ),
  error_summary TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  pdf_width_points REAL,
  pdf_height_points REAL,
  crop_left_points REAL,
  crop_bottom_points REAL,
  crop_right_points REAL,
  crop_top_points REAL,
  rotation_degrees INTEGER NOT NULL DEFAULT 0 CHECK (
    rotation_degrees IN (0, 90, 180, 270)
  ),
  preview_width_px INTEGER,
  preview_height_px INTEGER,
  FOREIGN KEY (document_id) REFERENCES documents(document_id),
  FOREIGN KEY (image_hash) REFERENCES image_assets(image_hash)
);

INSERT INTO page_records_new (
  page_id, document_id, page_number, image_hash, status, error_summary,
  created_at, updated_at, pdf_width_points, pdf_height_points,
  crop_left_points, crop_bottom_points, crop_right_points, crop_top_points,
  rotation_degrees, preview_width_px, preview_height_px
)
SELECT
  page_id, document_id, page_number, image_hash, status, error_summary,
  created_at, updated_at, pdf_width_points, pdf_height_points,
  crop_left_points, crop_bottom_points, crop_right_points, crop_top_points,
  rotation_degrees, preview_width_px, preview_height_px
FROM page_records;

CREATE TABLE analysis_results_new (
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
  FOREIGN KEY (page_id) REFERENCES page_records_new(page_id),
  FOREIGN KEY (error_id) REFERENCES errors(error_id),
  UNIQUE(page_id)
);

INSERT INTO analysis_results_new
SELECT analysis_id, page_id, schema_version, provider, model_name, status,
       result_json, error_id, created_at, updated_at
FROM analysis_results;

CREATE TABLE content_blocks_new (
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
  FOREIGN KEY (page_id) REFERENCES page_records_new(page_id) ON DELETE CASCADE,
  FOREIGN KEY (parent_block_id) REFERENCES content_blocks_new(block_id) ON DELETE CASCADE,
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

INSERT INTO content_blocks_new
SELECT block_id, parse_id, document_id, page_id, page_number,
       parent_block_id, source_element_id, ordinal, block_type, source_text,
       enrichment_json, raw_json, source_image_path, is_indexable, is_visual,
       is_decorative, bbox_x, bbox_y, bbox_width, bbox_height, created_at, updated_at
FROM content_blocks;

CREATE TABLE visual_module_analysis_new (
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
  FOREIGN KEY (block_id) REFERENCES content_blocks_new(block_id) ON DELETE CASCADE,
  FOREIGN KEY (error_id) REFERENCES errors(error_id)
);

INSERT INTO visual_module_analysis_new
SELECT analysis_id, block_id, schema_version, provider, model_name, status,
       result_json, error_id, attempt_count, created_at, updated_at
FROM visual_module_analysis;

DROP TABLE visual_module_analysis;
DROP TABLE content_blocks;
DROP TABLE analysis_results;
DROP TABLE page_records;

ALTER TABLE page_records_new RENAME TO page_records;
ALTER TABLE analysis_results_new RENAME TO analysis_results;
ALTER TABLE content_blocks_new RENAME TO content_blocks;
ALTER TABLE visual_module_analysis_new RENAME TO visual_module_analysis;

CREATE INDEX idx_page_records_document_id ON page_records(document_id);
CREATE INDEX idx_page_records_image_hash ON page_records(image_hash);
CREATE INDEX idx_analysis_results_page_id ON analysis_results(page_id);
CREATE INDEX idx_content_blocks_document_page_ordinal
  ON content_blocks(document_id, page_number, ordinal);
CREATE INDEX idx_content_blocks_page ON content_blocks(page_id);
CREATE INDEX idx_content_blocks_parse ON content_blocks(parse_id);
CREATE INDEX idx_content_blocks_visual_queue
  ON content_blocks(is_visual, is_decorative, page_id);
CREATE INDEX idx_visual_module_analysis_status
  ON visual_module_analysis(status, updated_at);
