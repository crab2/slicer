use crate::artifacts::page_json_exporter::PageJsonExporter;
use crate::domain::analysis::{
    AnalysisResultDto, PageAnalysisSummaryDto, PageAnalysisV1, PageWorkbenchDto,
};
use crate::domain::page::PageRecordDto;
use crate::errors::{AppError, AppResult};
use crate::repositories::db::block_on_db;
use crate::repositories::document_repository::DocumentRepository;
use chrono::Utc;
use sqlx::SqliteConnection;
use uuid::Uuid;

pub struct AnalysisRepository;

impl AnalysisRepository {
    pub fn save_success_result(
        conn: &mut SqliteConnection,
        page_id: &str,
        schema_version: &str,
        provider: &str,
        model_name: &str,
        result_json: &str,
    ) -> AppResult<AnalysisResultDto> {
        let analysis_id = Uuid::new_v4().to_string();
        let created_at = Utc::now().to_rfc3339();
        let updated_at = Utc::now().to_rfc3339();

        block_on_db(async {
            let row = sqlx::query_as::<_, AnalysisResultRow>(
                "INSERT INTO analysis_results
                 (analysis_id, page_id, schema_version, provider, model_name, status, result_json, error_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'succeeded', ?6, NULL, ?7, ?8)
                 ON CONFLICT(page_id) DO UPDATE SET
                   analysis_id = excluded.analysis_id,
                   schema_version = excluded.schema_version,
                   provider = excluded.provider,
                   model_name = excluded.model_name,
                   status = 'succeeded',
                   result_json = excluded.result_json,
                   error_id = NULL,
                   updated_at = excluded.updated_at
                 RETURNING analysis_id, page_id, schema_version, provider, model_name, status, result_json, error_id, created_at, updated_at",
            )
            .bind(&analysis_id)
            .bind(page_id)
            .bind(schema_version)
            .bind(provider)
            .bind(model_name)
            .bind(result_json)
            .bind(&created_at)
            .bind(&updated_at)
            .fetch_one(&mut *conn)
            .await
            .map_err(|err| super::db::database_error("analysis", "analysis_result_save_failed", err))?;
            Ok(row.to_dto())
        })
    }

    pub fn save_failure_result(
        conn: &mut SqliteConnection,
        page_id: &str,
        schema_version: &str,
        provider: &str,
        model_name: &str,
        error_id: &str,
    ) -> AppResult<AnalysisResultDto> {
        let analysis_id = Uuid::new_v4().to_string();
        let created_at = Utc::now().to_rfc3339();
        let updated_at = Utc::now().to_rfc3339();

        block_on_db(async {
            let row = sqlx::query_as::<_, AnalysisResultRow>(
                "INSERT INTO analysis_results
                 (analysis_id, page_id, schema_version, provider, model_name, status, result_json, error_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'failed', NULL, ?6, ?7, ?8)
                 ON CONFLICT(page_id) DO UPDATE SET
                   schema_version = excluded.schema_version,
                   provider = excluded.provider,
                   model_name = excluded.model_name,
                   status = 'failed',
                   result_json = NULL,
                   error_id = excluded.error_id,
                   updated_at = excluded.updated_at
                 RETURNING analysis_id, page_id, schema_version, provider, model_name, status, result_json, error_id, created_at, updated_at",
            )
            .bind(&analysis_id)
            .bind(page_id)
            .bind(schema_version)
            .bind(provider)
            .bind(model_name)
            .bind(error_id)
            .bind(&created_at)
            .bind(&updated_at)
            .fetch_one(&mut *conn)
            .await
            .map_err(|err| super::db::database_error("analysis", "analysis_failure_save_failed", err))?;
            Ok(row.to_dto())
        })
    }

    pub fn find_summary_for_page(
        conn: &mut SqliteConnection,
        page_id: &str,
    ) -> AppResult<Option<PageAnalysisSummaryDto>> {
        let Some(analysis) = Self::find_succeeded_page_analysis(conn, page_id)? else {
            return Ok(None);
        };
        Ok(Some(PageAnalysisSummaryDto::from_analysis(&analysis)))
    }

    pub fn list_workbench_pages(
        conn: &mut SqliteConnection,
        document_id: &str,
    ) -> AppResult<Vec<PageWorkbenchDto>> {
        let pages = DocumentRepository::list_pages_by_document(conn, document_id)?;
        let mut items = Vec::with_capacity(pages.len());
        for page in pages {
            items.push(Self::page_to_workbench(conn, page)?);
        }
        Ok(items)
    }

    fn page_to_workbench(
        conn: &mut SqliteConnection,
        page: PageRecordDto,
    ) -> AppResult<PageWorkbenchDto> {
        let image_path = DocumentRepository::find_image_asset_by_hash(conn, &page.image_hash)?
            .map(|asset| asset.file_path);
        let analysis_summary = if page.status == "analyzed" {
            Self::find_summary_for_page(conn, &page.page_id)?
        } else {
            None
        };
        Ok(PageWorkbenchDto {
            page_id: page.page_id,
            document_id: page.document_id,
            page_number: page.page_number,
            image_hash: page.image_hash,
            image_path,
            status: page.status,
            error_summary: page.error_summary,
            created_at: page.created_at,
            updated_at: page.updated_at,
            analysis_summary,
        })
    }

    pub fn find_succeeded_page_analysis(
        conn: &mut SqliteConnection,
        page_id: &str,
    ) -> AppResult<Option<PageAnalysisV1>> {
        let Some(result) = Self::find_current_by_page_id(conn, page_id)? else {
            return Ok(None);
        };
        if result.status != "succeeded" {
            return Ok(None);
        }
        let Some(result_json) = result.result_json else {
            return Ok(None);
        };
        let mut analysis = serde_json::from_str(&result_json).map_err(|err| {
            AppError::new(
                "analysis_result_json_parse_failed",
                "账本中的页面分析 JSON 无法解析。",
                "analysis",
                false,
            )
            .with_details(err.to_string())
        })?;
        PageJsonExporter::sync_analysis_image_path(conn, &mut analysis)?;
        Ok(Some(analysis))
    }

    pub fn list_current_succeeded_analyses(
        conn: &mut SqliteConnection,
    ) -> AppResult<Vec<PageAnalysisV1>> {
        block_on_db(async {
            let rows = sqlx::query_as::<_, AnalysisResultRow>(
                "SELECT analysis_id, page_id, schema_version, provider, model_name, status, result_json, error_id, created_at, updated_at
                 FROM analysis_results
                 WHERE status = 'succeeded' AND result_json IS NOT NULL
                 ORDER BY page_id",
            )
            .fetch_all(&mut *conn)
            .await
            .map_err(|err| {
                super::db::database_error("analysis", "analysis_succeeded_list_failed", err)
            })?;

            let mut analyses = Vec::with_capacity(rows.len());
            for row in rows {
                let Some(result_json) = row.result_json else {
                    continue;
                };
                let mut analysis: PageAnalysisV1 =
                    serde_json::from_str(&result_json).map_err(|err| {
                        AppError::new(
                            "analysis_result_json_parse_failed",
                            "账本中的页面分析 JSON 无法解析。",
                            "analysis",
                            false,
                        )
                        .with_details(err.to_string())
                    })?;
                if let Some(image_path) =
                    Self::lookup_image_path_for_page(&mut *conn, &analysis.page_id).await?
                {
                    analysis.image_path = image_path;
                }
                analyses.push(analysis);
            }
            Ok(analyses)
        })
    }

    async fn lookup_image_path_for_page(
        conn: &mut SqliteConnection,
        page_id: &str,
    ) -> AppResult<Option<String>> {
        let image_hash: Option<String> =
            sqlx::query_scalar("SELECT image_hash FROM page_records WHERE page_id = ?1")
                .bind(page_id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(|err| {
                    super::db::database_error("analysis", "page_image_hash_lookup_failed", err)
                })?;

        let Some(image_hash) = image_hash else {
            return Ok(None);
        };

        let file_path: Option<String> =
            sqlx::query_scalar("SELECT file_path FROM image_assets WHERE image_hash = ?1")
                .bind(image_hash)
                .fetch_optional(&mut *conn)
                .await
                .map_err(|err| {
                    super::db::database_error("analysis", "image_asset_path_lookup_failed", err)
                })?;

        Ok(file_path)
    }

    pub fn find_current_by_page_id(
        conn: &mut SqliteConnection,
        page_id: &str,
    ) -> AppResult<Option<AnalysisResultDto>> {
        block_on_db(async {
            let row = sqlx::query_as::<_, AnalysisResultRow>(
                "SELECT analysis_id, page_id, schema_version, provider, model_name, status, result_json, error_id, created_at, updated_at
                 FROM analysis_results WHERE page_id = ?1",
            )
            .bind(page_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|err| super::db::database_error("analysis", "analysis_result_lookup_failed", err))?;

            Ok(row.map(|r| r.to_dto()))
        })
    }
}

#[derive(sqlx::FromRow)]
struct AnalysisResultRow {
    analysis_id: String,
    page_id: String,
    schema_version: String,
    provider: String,
    model_name: String,
    status: String,
    result_json: Option<String>,
    error_id: Option<String>,
    created_at: String,
    updated_at: String,
}

impl AnalysisResultRow {
    fn to_dto(self) -> AnalysisResultDto {
        AnalysisResultDto {
            analysis_id: self.analysis_id,
            page_id: self.page_id,
            schema_version: self.schema_version,
            provider: self.provider,
            model_name: self.model_name,
            status: self.status,
            result_json: self.result_json,
            error_id: self.error_id,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AnalysisRepository;
    use crate::domain::analysis::PAGE_ANALYSIS_SCHEMA_VERSION;
    use crate::repositories::db::{block_on_db, connect_workspace_db, run_migrations};
    use crate::repositories::document_repository::DocumentRepository;
    use std::fs;

    fn test_connection() -> (sqlx::SqliteConnection, std::path::PathBuf) {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "slicer-analysis-repo-{}-{nonce}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("temp root");
        let db_path = root.join("app.db");
        block_on_db(run_migrations(db_path.clone())).expect("migrations");
        let conn = block_on_db(connect_workspace_db(db_path)).expect("connection");
        (conn, root)
    }

    fn seed_page(conn: &mut sqlx::SqliteConnection) {
        DocumentRepository::create_document(
            conn,
            "sample.pdf",
            "pdf",
            "file-hash",
            "originals/sample.pdf",
            None,
        )
        .expect("document");
        DocumentRepository::create_image_asset(conn, "image-hash", "pages/doc/image.png", 42)
            .expect("image asset");
        let doc = DocumentRepository::find_document_by_hash(conn, "file-hash")
            .expect("lookup")
            .expect("doc");
        DocumentRepository::create_page_record(conn, &doc.document_id, 1, "image-hash")
            .expect("page");
    }

    fn seed_error(conn: &mut sqlx::SqliteConnection, error_id: &str) {
        block_on_db(async {
            sqlx::query(
                "INSERT INTO errors (error_id, code, message, stage, retryable, details, correlation_id, created_at)
                 VALUES (?1, 'analysis_failed', '分析失败', 'analysis', 1, NULL, ?2, ?3)",
            )
            .bind(error_id)
            .bind(format!("correlation-{error_id}"))
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(&mut *conn)
            .await
            .map_err(|err| crate::repositories::db::database_error("test", "error_seed_failed", err))?;
            Ok(())
        })
        .expect("seed error");
    }

    #[test]
    fn saves_and_reads_current_success_result() {
        let (mut conn, root) = test_connection();
        seed_page(&mut conn);
        let page = DocumentRepository::list_all_pages(&mut conn)
            .expect("pages")
            .remove(0);

        let result = AnalysisRepository::save_success_result(
            &mut conn,
            &page.page_id,
            PAGE_ANALYSIS_SCHEMA_VERSION,
            "local_mock",
            "mock",
            r#"{"ok":true}"#,
        )
        .expect("save");

        let loaded = AnalysisRepository::find_current_by_page_id(&mut conn, &page.page_id)
            .expect("lookup")
            .expect("result");
        assert_eq!(loaded.analysis_id, result.analysis_id);
        assert_eq!(loaded.status, "succeeded");
        assert_eq!(loaded.result_json.as_deref(), Some(r#"{"ok":true}"#));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failure_result_does_not_create_success_payload() {
        let (mut conn, root) = test_connection();
        seed_page(&mut conn);
        seed_error(&mut conn, "error-1");
        let page = DocumentRepository::list_all_pages(&mut conn)
            .expect("pages")
            .remove(0);

        let result = AnalysisRepository::save_failure_result(
            &mut conn,
            &page.page_id,
            PAGE_ANALYSIS_SCHEMA_VERSION,
            "local_mock",
            "mock",
            "error-1",
        )
        .expect("save failure");

        assert_eq!(result.status, "failed");
        assert!(result.result_json.is_none());
        assert_eq!(result.error_id.as_deref(), Some("error-1"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reanalysis_success_updates_analysis_id() {
        let (mut conn, root) = test_connection();
        seed_page(&mut conn);
        let page = DocumentRepository::list_all_pages(&mut conn)
            .expect("pages")
            .remove(0);

        let first = AnalysisRepository::save_success_result(
            &mut conn,
            &page.page_id,
            PAGE_ANALYSIS_SCHEMA_VERSION,
            "local_mock",
            "mock",
            r#"{"ok":true}"#,
        )
        .expect("first save");

        let second = AnalysisRepository::save_success_result(
            &mut conn,
            &page.page_id,
            PAGE_ANALYSIS_SCHEMA_VERSION,
            "local_mock",
            "mock",
            r#"{"ok":true,"attempt":2}"#,
        )
        .expect("second save");

        assert_ne!(first.analysis_id, second.analysis_id);
        assert_eq!(
            second.result_json.as_deref(),
            Some(r#"{"ok":true,"attempt":2}"#)
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn list_current_succeeded_analyses_skips_failed() {
        let (mut conn, root) = test_connection();
        seed_page(&mut conn);
        seed_error(&mut conn, "error-1");
        let page = DocumentRepository::list_all_pages(&mut conn)
            .expect("pages")
            .remove(0);

        AnalysisRepository::save_failure_result(
            &mut conn,
            &page.page_id,
            PAGE_ANALYSIS_SCHEMA_VERSION,
            "local_mock",
            "mock",
            "error-1",
        )
        .expect("failure");

        let analyses =
            AnalysisRepository::list_current_succeeded_analyses(&mut conn).expect("list");
        assert!(analyses.is_empty());

        let payload = format!(
            r#"{{
  "schema_version": "page_analysis_v1",
  "page_id": "{page_id}",
  "image_hash": "image-hash",
  "image_path": "pages/doc/image.png",
  "source": {{"document_id": "doc", "page_number": 1, "original_filename": null}},
  "analysis": {{"title": null, "summary": null, "visible_text": null, "topics": [], "keywords": []}},
  "retrieval": {{"bm25_text": "hello"}},
  "model": {{"provider": "local_mock", "model_name": "mock"}}
}}"#,
            page_id = page.page_id
        );
        AnalysisRepository::save_success_result(
            &mut conn,
            &page.page_id,
            PAGE_ANALYSIS_SCHEMA_VERSION,
            "local_mock",
            "mock",
            &payload,
        )
        .expect("success");

        let analyses =
            AnalysisRepository::list_current_succeeded_analyses(&mut conn).expect("list");
        assert_eq!(analyses.len(), 1);
        assert_eq!(analyses[0].retrieval.bm25_text, "hello");
        assert_eq!(analyses[0].image_path, "pages/doc/image.png");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn list_workbench_pages_includes_summary_for_succeeded_analysis() {
        let (mut conn, root) = test_connection();
        seed_page(&mut conn);
        let page = DocumentRepository::list_all_pages(&mut conn)
            .expect("pages")
            .remove(0);
        let payload = format!(
            r#"{{
  "schema_version": "page_analysis_v1",
  "page_id": "{page_id}",
  "image_hash": "image-hash",
  "image_path": "pages/doc/image.png",
  "source": {{"document_id": "{doc_id}", "page_number": 1, "original_filename": "sample.pdf"}},
  "analysis": {{"title": "标题", "summary": "摘要", "visible_text": "正文", "topics": ["A"], "keywords": ["K"]}},
  "retrieval": {{"bm25_text": "正文"}},
  "model": {{"provider": "local_mock", "model_name": "mock"}}
}}"#,
            page_id = page.page_id,
            doc_id = page.document_id
        );
        AnalysisRepository::save_success_result(
            &mut conn,
            &page.page_id,
            PAGE_ANALYSIS_SCHEMA_VERSION,
            "local_mock",
            "mock",
            &payload,
        )
        .expect("save");
        DocumentRepository::update_page_status(&mut conn, &page.page_id, "analyzed", None)
            .expect("mark analyzed");

        let items = AnalysisRepository::list_workbench_pages(&mut conn, &page.document_id)
            .expect("workbench pages");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].image_path.as_deref(), Some("pages/doc/image.png"));
        let summary = items[0].analysis_summary.as_ref().expect("summary");
        assert_eq!(summary.title.as_deref(), Some("标题"));
        assert_eq!(summary.visible_text_char_count, "正文".len());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn list_workbench_pages_omits_summary_while_reanalysis_pending() {
        let (mut conn, root) = test_connection();
        seed_page(&mut conn);
        let page = DocumentRepository::list_all_pages(&mut conn)
            .expect("pages")
            .remove(0);
        let payload = format!(
            r#"{{
  "schema_version": "page_analysis_v1",
  "page_id": "{page_id}",
  "image_hash": "image-hash",
  "image_path": "pages/doc/image.png",
  "source": {{"document_id": "{doc_id}", "page_number": 1, "original_filename": "sample.pdf"}},
  "analysis": {{"title": "旧标题", "summary": "旧摘要", "visible_text": "正文", "topics": [], "keywords": []}},
  "retrieval": {{"bm25_text": "正文"}},
  "model": {{"provider": "local_mock", "model_name": "mock"}}
}}"#,
            page_id = page.page_id,
            doc_id = page.document_id
        );
        AnalysisRepository::save_success_result(
            &mut conn,
            &page.page_id,
            PAGE_ANALYSIS_SCHEMA_VERSION,
            "local_mock",
            "mock",
            &payload,
        )
        .expect("save");
        DocumentRepository::update_page_status(&mut conn, &page.page_id, "analyzed", None)
            .expect("mark analyzed");
        DocumentRepository::update_page_status(&mut conn, &page.page_id, "analysis_pending", None)
            .expect("mark pending");

        let items = AnalysisRepository::list_workbench_pages(&mut conn, &page.document_id)
            .expect("workbench pages");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].status, "analysis_pending");
        assert!(items[0].analysis_summary.is_none());

        let _ = fs::remove_dir_all(root);
    }
}
