use crate::domain::document::DocumentDto;
use crate::domain::page::{ImageAssetDto, PageRecordDto};
use crate::errors::AppResult;
use crate::repositories::db::block_on_db;
use chrono::Utc;
use sqlx::SqliteConnection;
use std::collections::HashMap;
use uuid::Uuid;

pub struct DocumentRepository;

pub struct DeletedDocumentArtifacts {
    pub original_path: String,
    pub removable_image_paths: Vec<String>,
}

impl DocumentRepository {
    pub fn create_document(
        conn: &mut SqliteConnection,
        original_filename: &str,
        file_type: &str,
        file_hash: &str,
        original_path: &str,
        job_id: Option<&str>,
    ) -> AppResult<DocumentDto> {
        let document_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        block_on_db(async {
            sqlx::query(
                "INSERT INTO documents (document_id, original_filename, file_type, file_hash, original_path, status, job_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'importing', ?6, ?7, ?8)"
            )
            .bind(&document_id)
            .bind(original_filename)
            .bind(file_type)
            .bind(file_hash)
            .bind(original_path)
            .bind(job_id)
            .bind(&now)
            .bind(&now)
            .execute(&mut *conn)
            .await
            .map_err(|err| super::db::database_error("import", "document_create_failed", err))?;
            Ok(())
        })?;

        Ok(DocumentDto {
            document_id,
            original_filename: original_filename.to_string(),
            file_type: file_type.to_string(),
            file_hash: file_hash.to_string(),
            original_path: original_path.to_string(),
            page_count: None,
            status: "importing".to_string(),
            error_summary: None,
            job_id: job_id.map(|s| s.to_string()),
            analysis_succeeded_pages: 0,
            analysis_failed_pages: 0,
            last_analyzed_at: None,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn update_document_status(
        conn: &mut SqliteConnection,
        document_id: &str,
        status: &str,
        page_count: Option<i64>,
        error_summary: Option<&str>,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        block_on_db(async {
            sqlx::query(
                "UPDATE documents SET status = ?1, page_count = ?2, error_summary = ?3, updated_at = ?4 WHERE document_id = ?5"
            )
            .bind(status)
            .bind(page_count)
            .bind(error_summary)
            .bind(&now)
            .bind(document_id)
            .execute(&mut *conn)
            .await
            .map_err(|err| super::db::database_error("import", "document_update_failed", err))?;
            Ok(())
        })
    }

    pub fn find_document_by_hash(
        conn: &mut SqliteConnection,
        file_hash: &str,
    ) -> AppResult<Option<DocumentDto>> {
        block_on_db(async {
            let row = sqlx::query_as::<_, DocumentRow>(
                "SELECT documents.document_id, documents.original_filename, documents.file_type, documents.file_hash, documents.original_path,
                        documents.page_count, documents.status, documents.error_summary, documents.job_id,
                        COALESCE(SUM(CASE WHEN analysis_results.status = 'succeeded' THEN 1 ELSE 0 END), 0) AS analysis_succeeded_pages,
                        COALESCE(SUM(CASE WHEN analysis_results.status = 'failed' THEN 1 ELSE 0 END), 0) AS analysis_failed_pages,
                        MAX(analysis_results.updated_at) AS last_analyzed_at,
                        documents.created_at, documents.updated_at
                 FROM documents
                 LEFT JOIN page_records ON page_records.document_id = documents.document_id
                 LEFT JOIN analysis_results ON analysis_results.page_id = page_records.page_id
                 WHERE documents.file_hash = ?1
                 GROUP BY documents.document_id"
            )
            .bind(file_hash)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|err| super::db::database_error("import", "document_lookup_failed", err))?;

            Ok(row.map(|r| r.to_dto()))
        })
    }

    pub fn find_document_by_id(
        conn: &mut SqliteConnection,
        document_id: &str,
    ) -> AppResult<Option<DocumentDto>> {
        block_on_db(async {
            let row = sqlx::query_as::<_, DocumentRow>(
                "SELECT documents.document_id, documents.original_filename, documents.file_type, documents.file_hash, documents.original_path,
                        documents.page_count, documents.status, documents.error_summary, documents.job_id,
                        COALESCE(SUM(CASE WHEN analysis_results.status = 'succeeded' THEN 1 ELSE 0 END), 0) AS analysis_succeeded_pages,
                        COALESCE(SUM(CASE WHEN analysis_results.status = 'failed' THEN 1 ELSE 0 END), 0) AS analysis_failed_pages,
                        MAX(analysis_results.updated_at) AS last_analyzed_at,
                        documents.created_at, documents.updated_at
                 FROM documents
                 LEFT JOIN page_records ON page_records.document_id = documents.document_id
                 LEFT JOIN analysis_results ON analysis_results.page_id = page_records.page_id
                 WHERE documents.document_id = ?1
                 GROUP BY documents.document_id"
            )
            .bind(document_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|err| super::db::database_error("import", "document_lookup_failed", err))?;

            Ok(row.map(|r| r.to_dto()))
        })
    }

    pub fn find_image_asset_by_hash(
        conn: &mut SqliteConnection,
        image_hash: &str,
    ) -> AppResult<Option<ImageAssetDto>> {
        block_on_db(async {
            let row = sqlx::query_as::<_, ImageAssetRow>(
                "SELECT image_hash, file_path, file_size, created_at FROM image_assets WHERE image_hash = ?1"
            )
            .bind(image_hash)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|err| super::db::database_error("query", "image_asset_lookup_failed", err))?;

            Ok(row.map(|r| r.to_dto()))
        })
    }

    pub fn create_image_asset(
        conn: &mut SqliteConnection,
        image_hash: &str,
        file_path: &str,
        file_size: i64,
    ) -> AppResult<ImageAssetDto> {
        let now = Utc::now().to_rfc3339();
        block_on_db(async {
            sqlx::query(
                "INSERT OR IGNORE INTO image_assets (image_hash, file_path, file_size, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(image_hash)
            .bind(file_path)
            .bind(file_size)
            .bind(&now)
            .execute(&mut *conn)
            .await
            .map_err(|err| super::db::database_error("import", "image_asset_create_failed", err))?;
            Ok(())
        })?;

        Ok(ImageAssetDto {
            image_hash: image_hash.to_string(),
            file_path: file_path.to_string(),
            file_size,
            created_at: now,
        })
    }

    pub fn create_page_record(
        conn: &mut SqliteConnection,
        document_id: &str,
        page_number: i64,
        image_hash: &str,
    ) -> AppResult<PageRecordDto> {
        let page_id = format!("{}_{}", document_id, page_number);
        let now = Utc::now().to_rfc3339();

        block_on_db(async {
            sqlx::query(
                "INSERT INTO page_records (page_id, document_id, page_number, image_hash, status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 'rendered', ?5, ?6)"
            )
            .bind(&page_id)
            .bind(document_id)
            .bind(page_number)
            .bind(image_hash)
            .bind(&now)
            .bind(&now)
            .execute(&mut *conn)
            .await
            .map_err(|err| super::db::database_error("import", "page_record_create_failed", err))?;
            Ok(())
        })?;

        Ok(PageRecordDto {
            page_id,
            document_id: document_id.to_string(),
            page_number,
            image_hash: image_hash.to_string(),
            status: "rendered".to_string(),
            error_summary: None,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn find_page_by_id(
        conn: &mut SqliteConnection,
        page_id: &str,
    ) -> AppResult<Option<PageRecordDto>> {
        block_on_db(async {
            let row = sqlx::query_as::<_, PageRow>(
                "SELECT page_id, document_id, page_number, image_hash, status, error_summary, created_at, updated_at
                 FROM page_records WHERE page_id = ?1"
            )
            .bind(page_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|err| super::db::database_error("query", "page_lookup_failed", err))?;

            Ok(row.map(|r| r.to_dto()))
        })
    }

    pub fn update_page_status(
        conn: &mut SqliteConnection,
        page_id: &str,
        status: &str,
        error_summary: Option<&str>,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        block_on_db(async {
            sqlx::query(
                "UPDATE page_records SET status = ?1, error_summary = ?2, updated_at = ?3 WHERE page_id = ?4",
            )
            .bind(status)
            .bind(error_summary)
            .bind(&now)
            .bind(page_id)
            .execute(&mut *conn)
            .await
            .map_err(|err| super::db::database_error("analysis", "page_status_update_failed", err))?;
            Ok(())
        })
    }

    pub fn list_documents(conn: &mut SqliteConnection) -> AppResult<Vec<DocumentDto>> {
        block_on_db(async {
            let rows = sqlx::query_as::<_, DocumentRow>(
                "SELECT documents.document_id, documents.original_filename, documents.file_type, documents.file_hash, documents.original_path,
                        documents.page_count, documents.status, documents.error_summary, documents.job_id,
                        COALESCE(SUM(CASE WHEN analysis_results.status = 'succeeded' THEN 1 ELSE 0 END), 0) AS analysis_succeeded_pages,
                        COALESCE(SUM(CASE WHEN analysis_results.status = 'failed' THEN 1 ELSE 0 END), 0) AS analysis_failed_pages,
                        MAX(analysis_results.updated_at) AS last_analyzed_at,
                        documents.created_at, documents.updated_at
                 FROM documents
                 LEFT JOIN page_records ON page_records.document_id = documents.document_id
                 LEFT JOIN analysis_results ON analysis_results.page_id = page_records.page_id
                 GROUP BY documents.document_id
                 ORDER BY documents.created_at DESC"
            )
            .fetch_all(&mut *conn)
            .await
            .map_err(|err| super::db::database_error("query", "document_list_failed", err))?;

            Ok(rows.into_iter().map(|r| r.to_dto()).collect())
        })
    }

    pub fn list_pages_by_document(
        conn: &mut SqliteConnection,
        document_id: &str,
    ) -> AppResult<Vec<PageRecordDto>> {
        block_on_db(async {
            let rows = sqlx::query_as::<_, PageRow>(
                "SELECT page_id, document_id, page_number, image_hash, status, error_summary, created_at, updated_at
                 FROM page_records WHERE document_id = ?1 ORDER BY page_number"
            )
            .bind(document_id)
            .fetch_all(&mut *conn)
            .await
            .map_err(|err| super::db::database_error("query", "page_list_failed", err))?;

            Ok(rows.into_iter().map(|r| r.to_dto()).collect())
        })
    }

    pub fn list_failed_pages_by_document(
        conn: &mut SqliteConnection,
        document_id: &str,
    ) -> AppResult<Vec<PageRecordDto>> {
        block_on_db(async {
            let rows = sqlx::query_as::<_, PageRow>(
                "SELECT page_records.page_id, page_records.document_id, page_records.page_number,
                        page_records.image_hash, page_records.status, page_records.error_summary,
                        page_records.created_at, page_records.updated_at
                 FROM page_records
                 LEFT JOIN analysis_results ON analysis_results.page_id = page_records.page_id
                 WHERE page_records.document_id = ?1
                   AND (page_records.status = 'failed' OR analysis_results.status = 'failed')
                 ORDER BY page_records.page_number",
            )
            .bind(document_id)
            .fetch_all(&mut *conn)
            .await
            .map_err(|err| super::db::database_error("query", "failed_page_list_failed", err))?;

            Ok(rows.into_iter().map(|r| r.to_dto()).collect())
        })
    }

    pub fn delete_document_records(
        conn: &mut SqliteConnection,
        document_id: &str,
    ) -> AppResult<Option<DeletedDocumentArtifacts>> {
        block_on_db(async {
            let original_path: Option<String> =
                sqlx::query_scalar("SELECT original_path FROM documents WHERE document_id = ?1")
                    .bind(document_id)
                    .fetch_optional(&mut *conn)
                    .await
                    .map_err(|err| {
                        super::db::database_error("document", "document_lookup_failed", err)
                    })?;

            let Some(original_path) = original_path else {
                return Ok(None);
            };

            let asset_rows = sqlx::query_as::<_, ImageAssetPathRow>(
                "SELECT DISTINCT page_records.image_hash, image_assets.file_path
                 FROM page_records
                 LEFT JOIN image_assets ON image_assets.image_hash = page_records.image_hash
                 WHERE page_records.document_id = ?1",
            )
            .bind(document_id)
            .fetch_all(&mut *conn)
            .await
            .map_err(|err| {
                super::db::database_error("document", "document_asset_list_failed", err)
            })?;

            sqlx::query("BEGIN IMMEDIATE")
                .execute(&mut *conn)
                .await
                .map_err(|err| {
                    super::db::database_error("document", "document_delete_begin_failed", err)
                })?;

            let result: AppResult<Vec<String>> = async {
                sqlx::query(
                    "DELETE FROM analysis_results
                     WHERE page_id IN (
                       SELECT page_id FROM page_records WHERE document_id = ?1
                     )",
                )
                .bind(document_id)
                .execute(&mut *conn)
                .await
                .map_err(|err| {
                    super::db::database_error("document", "document_analysis_delete_failed", err)
                })?;

                sqlx::query("DELETE FROM page_records WHERE document_id = ?1")
                    .bind(document_id)
                    .execute(&mut *conn)
                    .await
                    .map_err(|err| {
                        super::db::database_error("document", "document_pages_delete_failed", err)
                    })?;

                sqlx::query("DELETE FROM documents WHERE document_id = ?1")
                    .bind(document_id)
                    .execute(&mut *conn)
                    .await
                    .map_err(|err| {
                        super::db::database_error("document", "document_delete_failed", err)
                    })?;

                let mut asset_paths = HashMap::new();
                for row in asset_rows {
                    if let Some(file_path) = row.file_path {
                        asset_paths.entry(row.image_hash).or_insert(file_path);
                    }
                }

                let mut removable_paths = Vec::new();
                for (image_hash, file_path) in asset_paths {
                    let remaining_refs: i64 = sqlx::query_scalar(
                        "SELECT COUNT(*) FROM page_records WHERE image_hash = ?1",
                    )
                    .bind(&image_hash)
                    .fetch_one(&mut *conn)
                    .await
                    .map_err(|err| {
                        super::db::database_error(
                            "document",
                            "document_asset_ref_count_failed",
                            err,
                        )
                    })?;

                    if remaining_refs == 0 {
                        sqlx::query("DELETE FROM image_assets WHERE image_hash = ?1")
                            .bind(&image_hash)
                            .execute(&mut *conn)
                            .await
                            .map_err(|err| {
                                super::db::database_error(
                                    "document",
                                    "document_asset_delete_failed",
                                    err,
                                )
                            })?;
                        removable_paths.push(file_path);
                    }
                }

                Ok(removable_paths)
            }
            .await;

            match result {
                Ok(removable_image_paths) => {
                    sqlx::query("COMMIT")
                        .execute(&mut *conn)
                        .await
                        .map_err(|err| {
                            super::db::database_error(
                                "document",
                                "document_delete_commit_failed",
                                err,
                            )
                        })?;
                    Ok(Some(DeletedDocumentArtifacts {
                        original_path,
                        removable_image_paths,
                    }))
                }
                Err(err) => {
                    let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                    Err(err)
                }
            }
        })
    }

    pub fn list_pages_needing_analysis(
        conn: &mut SqliteConnection,
    ) -> AppResult<Vec<PageRecordDto>> {
        block_on_db(async {
            let rows = sqlx::query_as::<_, PageRow>(
                "SELECT page_records.page_id, page_records.document_id, page_records.page_number, page_records.image_hash,
                        page_records.status, page_records.error_summary, page_records.created_at, page_records.updated_at
                 FROM page_records
                 LEFT JOIN analysis_results
                   ON analysis_results.page_id = page_records.page_id
                  AND analysis_results.status = 'succeeded'
                 WHERE page_records.status IN ('rendered', 'failed')
                   AND analysis_results.page_id IS NULL
                 ORDER BY page_records.document_id, page_records.page_number",
            )
            .fetch_all(&mut *conn)
            .await
            .map_err(|err| {
                super::db::database_error("query", "analysis_eligible_pages_list_failed", err)
            })?;

            Ok(rows.into_iter().map(|r| r.to_dto()).collect())
        })
    }

    pub fn try_mark_page_analysis_pending(
        conn: &mut SqliteConnection,
        page_id: &str,
        force_reanalysis: bool,
    ) -> AppResult<bool> {
        let now = Utc::now().to_rfc3339();
        block_on_db(async {
            let affected = if force_reanalysis {
                sqlx::query(
                    "UPDATE page_records
                     SET status = 'analysis_pending', error_summary = NULL, updated_at = ?1
                     WHERE page_id = ?2
                       AND status != 'analysis_pending'",
                )
                .bind(&now)
                .bind(page_id)
                .execute(&mut *conn)
                .await
            } else {
                sqlx::query(
                    "UPDATE page_records
                     SET status = 'analysis_pending', error_summary = NULL, updated_at = ?1
                     WHERE page_id = ?2
                       AND status IN ('rendered', 'failed')
                       AND NOT EXISTS (
                         SELECT 1 FROM analysis_results
                         WHERE analysis_results.page_id = page_records.page_id
                           AND analysis_results.status = 'succeeded'
                       )",
                )
                .bind(&now)
                .bind(page_id)
                .execute(&mut *conn)
                .await
            }
            .map_err(|err| {
                super::db::database_error("analysis", "page_analysis_lease_failed", err)
            })?
            .rows_affected();

            Ok(affected > 0)
        })
    }

    pub fn recover_analysis_pending_pages(
        conn: &mut SqliteConnection,
        error_summary: &str,
    ) -> AppResult<u64> {
        let now = Utc::now().to_rfc3339();
        block_on_db(async {
            let affected = sqlx::query(
                "UPDATE page_records
                 SET status = 'failed', error_summary = ?1, updated_at = ?2
                 WHERE status = 'analysis_pending'",
            )
            .bind(error_summary)
            .bind(&now)
            .execute(&mut *conn)
            .await
            .map_err(|err| {
                super::db::database_error("analysis", "analysis_pending_recovery_failed", err)
            })?
            .rows_affected();

            Ok(affected)
        })
    }

    pub fn list_analysis_pending_pages(
        conn: &mut SqliteConnection,
    ) -> AppResult<Vec<PageRecordDto>> {
        block_on_db(async {
            let rows = sqlx::query_as::<_, PageRow>(
                "SELECT page_id, document_id, page_number, image_hash, status, error_summary, created_at, updated_at
                 FROM page_records
                 WHERE status = 'analysis_pending'
                 ORDER BY document_id, page_number",
            )
            .fetch_all(&mut *conn)
            .await
            .map_err(|err| {
                super::db::database_error("analysis", "analysis_pending_pages_list_failed", err)
            })?;

            Ok(rows.into_iter().map(|r| r.to_dto()).collect())
        })
    }

    pub fn list_all_pages(conn: &mut SqliteConnection) -> AppResult<Vec<PageRecordDto>> {
        block_on_db(async {
            let rows = sqlx::query_as::<_, PageRow>(
                "SELECT page_id, document_id, page_number, image_hash, status, error_summary, created_at, updated_at
                 FROM page_records ORDER BY document_id, page_number"
            )
            .fetch_all(&mut *conn)
            .await
            .map_err(|err| super::db::database_error("query", "all_pages_list_failed", err))?;

            Ok(rows.into_iter().map(|r| r.to_dto()).collect())
        })
    }
}

#[derive(sqlx::FromRow)]
struct DocumentRow {
    document_id: String,
    original_filename: String,
    file_type: String,
    file_hash: String,
    original_path: String,
    page_count: Option<i64>,
    status: String,
    error_summary: Option<String>,
    job_id: Option<String>,
    analysis_succeeded_pages: i64,
    analysis_failed_pages: i64,
    last_analyzed_at: Option<String>,
    created_at: String,
    updated_at: String,
}

impl DocumentRow {
    fn to_dto(self) -> DocumentDto {
        DocumentDto {
            document_id: self.document_id,
            original_filename: self.original_filename,
            file_type: self.file_type,
            file_hash: self.file_hash,
            original_path: self.original_path,
            page_count: self.page_count,
            status: self.status,
            error_summary: self.error_summary,
            job_id: self.job_id,
            analysis_succeeded_pages: self.analysis_succeeded_pages,
            analysis_failed_pages: self.analysis_failed_pages,
            last_analyzed_at: self.last_analyzed_at,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct PageRow {
    page_id: String,
    document_id: String,
    page_number: i64,
    image_hash: String,
    status: String,
    error_summary: Option<String>,
    created_at: String,
    updated_at: String,
}

impl PageRow {
    fn to_dto(self) -> PageRecordDto {
        PageRecordDto {
            page_id: self.page_id,
            document_id: self.document_id,
            page_number: self.page_number,
            image_hash: self.image_hash,
            status: self.status,
            error_summary: self.error_summary,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct ImageAssetRow {
    image_hash: String,
    file_path: String,
    file_size: i64,
    created_at: String,
}

#[derive(sqlx::FromRow)]
struct ImageAssetPathRow {
    image_hash: String,
    file_path: Option<String>,
}

impl ImageAssetRow {
    fn to_dto(self) -> ImageAssetDto {
        ImageAssetDto {
            image_hash: self.image_hash,
            file_path: self.file_path,
            file_size: self.file_size,
            created_at: self.created_at,
        }
    }
}
