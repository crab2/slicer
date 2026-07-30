use crate::errors::{AppError, AppResult};
use crate::repositories::db::block_on_db;
use sqlx::SqliteConnection;

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
}

fn db_error(code: &str, err: sqlx::Error) -> AppError {
    AppError::new(code, "读取文档查看制品登记失败。", "document_viewer", true)
        .with_details(err.to_string())
}
