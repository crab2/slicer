use crate::errors::{AppError, AppResult};
use crate::repositories::db::block_on_db;
use sqlx::SqliteConnection;

const MAX_VIEWER_ENRICHMENT_RECORDS: i64 = 10_000;
const MAX_VIEWER_ENRICHMENT_BYTES: i64 = 32 * 1024 * 1024;
const MAX_VIEWER_ENRICHMENT_RECORD_BYTES: i64 = 256_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentViewerDocumentRecord {
    pub document_id: String,
    pub original_filename: String,
    pub page_count: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentViewerArtifactRecord {
    pub kind: String,
    pub relative_path: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentViewerVisualEnrichmentRecord {
    pub block_id: String,
    pub enrichment_json: String,
}

pub struct DocumentViewerRepository;

impl DocumentViewerRepository {
    pub fn find_document(
        conn: &mut SqliteConnection,
        document_id: &str,
    ) -> AppResult<Option<DocumentViewerDocumentRecord>> {
        block_on_db(async {
            sqlx::query_as::<_, (String, String, Option<i64>)>(
                "SELECT document_id, original_filename, page_count
                 FROM documents
                 WHERE document_id = ?1",
            )
            .bind(document_id)
            .fetch_optional(conn)
            .await
            .map(|record| {
                record.map(|(document_id, original_filename, page_count)| {
                    DocumentViewerDocumentRecord {
                        document_id,
                        original_filename,
                        page_count,
                    }
                })
            })
            .map_err(|err| db_error("document_viewer_document_query_failed", err))
        })
    }

    pub fn list_artifacts(
        conn: &mut SqliteConnection,
        document_id: &str,
    ) -> AppResult<Vec<DocumentViewerArtifactRecord>> {
        block_on_db(async {
            sqlx::query_as::<_, (String, String, String)>(
                "SELECT kind, relative_path, content_hash
                 FROM document_artifacts
                 WHERE document_id = ?1
                 ORDER BY kind, relative_path",
            )
            .bind(document_id)
            .fetch_all(conn)
            .await
            .map(|records| {
                records
                    .into_iter()
                    .map(
                        |(kind, relative_path, content_hash)| DocumentViewerArtifactRecord {
                            kind,
                            relative_path,
                            content_hash,
                        },
                    )
                    .collect()
            })
            .map_err(|err| db_error("document_viewer_artifact_list_failed", err))
        })
    }

    pub fn find_artifact(
        conn: &mut SqliteConnection,
        document_id: &str,
        kind: &str,
    ) -> AppResult<Option<DocumentViewerArtifactRecord>> {
        block_on_db(async {
            sqlx::query_as::<_, (String, String, String)>(
                "SELECT kind, relative_path, content_hash
                 FROM document_artifacts
                 WHERE document_id = ?1 AND kind = ?2
                 ORDER BY relative_path
                 LIMIT 1",
            )
            .bind(document_id)
            .bind(kind)
            .fetch_optional(conn)
            .await
            .map(|record| {
                record.map(
                    |(kind, relative_path, content_hash)| DocumentViewerArtifactRecord {
                        kind,
                        relative_path,
                        content_hash,
                    },
                )
            })
            .map_err(|err| db_error("document_viewer_artifact_query_failed", err))
        })
    }

    pub fn list_visual_enrichments(
        conn: &mut SqliteConnection,
        document_id: &str,
        raw_json_path: &str,
    ) -> AppResult<Vec<DocumentViewerVisualEnrichmentRecord>> {
        block_on_db(async {
            let (record_count, total_bytes) = sqlx::query_as::<_, (i64, i64)>(
                "SELECT COUNT(*), COALESCE(SUM(LENGTH(CAST(enrichment_json AS BLOB))), 0)
                 FROM content_blocks
                 WHERE document_id = ?1
                   AND is_visual = 1
                   AND TYPEOF(enrichment_json) = 'text'
                   AND LENGTH(CAST(enrichment_json AS BLOB)) <= ?3
                   AND parse_id = (
                     SELECT parse_id
                     FROM pdf_parse_runs
                     WHERE document_id = ?1
                       AND status = 'succeeded'
                       AND raw_json_path = ?2
                     ORDER BY updated_at DESC, parse_id DESC
                     LIMIT 1
                   )",
            )
            .bind(document_id)
            .bind(raw_json_path)
            .bind(MAX_VIEWER_ENRICHMENT_RECORD_BYTES)
            .fetch_one(&mut *conn)
            .await
            .map_err(|err| db_error("document_viewer_enrichment_budget_query_failed", err))?;
            if record_count > MAX_VIEWER_ENRICHMENT_RECORDS
                || total_bytes > MAX_VIEWER_ENRICHMENT_BYTES
            {
                return Ok(Vec::new());
            }

            sqlx::query_as::<_, (String, String)>(
                "SELECT content_blocks.block_id, content_blocks.enrichment_json
                 FROM content_blocks
                 WHERE content_blocks.document_id = ?1
                   AND content_blocks.is_visual = 1
                   AND TYPEOF(content_blocks.enrichment_json) = 'text'
                   AND LENGTH(CAST(content_blocks.enrichment_json AS BLOB)) <= ?3
                   AND content_blocks.parse_id = (
                     SELECT parse_id
                     FROM pdf_parse_runs
                     WHERE document_id = ?1
                       AND status = 'succeeded'
                       AND raw_json_path = ?2
                     ORDER BY updated_at DESC, parse_id DESC
                     LIMIT 1
                   )
                 ORDER BY content_blocks.ordinal",
            )
            .bind(document_id)
            .bind(raw_json_path)
            .bind(MAX_VIEWER_ENRICHMENT_RECORD_BYTES)
            .fetch_all(conn)
            .await
            .map(|records| {
                records
                    .into_iter()
                    .map(
                        |(block_id, enrichment_json)| DocumentViewerVisualEnrichmentRecord {
                            block_id,
                            enrichment_json,
                        },
                    )
                    .collect()
            })
            .map_err(|err| db_error("document_viewer_enrichment_query_failed", err))
        })
    }
}

fn db_error(code: &str, err: sqlx::Error) -> AppError {
    AppError::new(code, "读取文档查看制品登记失败。", "document_viewer", true)
        .with_details(err.to_string())
}
