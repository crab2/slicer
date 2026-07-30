CREATE TABLE document_artifacts_v2 (
  artifact_id TEXT PRIMARY KEY,
  document_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (
    kind IN (
      'canonical_pdf',
      'pdf_structure_json',
      'pdf_structure_html',
      'pdf_structure_markdown',
      'pdf_structure_annotated_pdf',
      'pdf_structure_image'
    )
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

INSERT INTO document_artifacts_v2 (
  artifact_id, document_id, kind, relative_path, content_hash,
  parser_name, parser_version, parser_options_json, created_at, updated_at
)
SELECT
  artifact_id, document_id, kind, relative_path, content_hash,
  parser_name, parser_version, parser_options_json, created_at, updated_at
FROM document_artifacts;

DROP TABLE document_artifacts;
ALTER TABLE document_artifacts_v2 RENAME TO document_artifacts;

CREATE UNIQUE INDEX idx_document_artifacts_singletons
  ON document_artifacts(document_id, kind)
  WHERE kind IN (
    'canonical_pdf',
    'pdf_structure_json',
    'pdf_structure_html',
    'pdf_structure_markdown',
    'pdf_structure_annotated_pdf'
  );

CREATE INDEX idx_document_artifacts_document_kind
  ON document_artifacts(document_id, kind);
