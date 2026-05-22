use crate::domain::analysis::PageAnalysisV1;
use crate::domain::page::PageRecordDto;
use crate::errors::{AppError, AppResult};
use crate::repositories::analysis_repository::AnalysisRepository;
use crate::repositories::document_repository::DocumentRepository;
use serde::Serialize;
use sqlx::SqliteConnection;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PageBaselineJsonl {
    pub page_id: String,
    pub document_id: String,
    pub page_number: i64,
    pub image_hash: String,
    pub image_path: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_summary: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum PageJsonlLine {
    Analysis(PageAnalysisV1),
    Baseline(PageBaselineJsonl),
}

pub struct PageJsonExporter;

impl PageJsonExporter {
    pub fn build_lines(conn: &mut SqliteConnection) -> AppResult<Vec<PageJsonlLine>> {
        let pages = DocumentRepository::list_all_pages(conn)?;
        let mut lines = Vec::with_capacity(pages.len());

        for page in pages {
            let image_path = DocumentRepository::find_image_asset_by_hash(conn, &page.image_hash)?
                .map(|asset| asset.file_path)
                .unwrap_or_default();

            if let Some(analysis_line) = Self::analysis_line_for_page(conn, &page, &image_path)? {
                lines.push(analysis_line);
            } else {
                lines.push(PageJsonlLine::Baseline(PageBaselineJsonl {
                    page_id: page.page_id,
                    document_id: page.document_id,
                    page_number: page.page_number,
                    image_hash: page.image_hash,
                    image_path,
                    status: page.status,
                    error_summary: page.error_summary,
                    created_at: page.created_at,
                    updated_at: page.updated_at,
                }));
            }
        }

        Ok(lines)
    }

    pub fn serialize_line(line: &PageJsonlLine) -> AppResult<String> {
        // Structured PageAnalysisV1 / PageBaselineJsonl only — no log-style redact_secrets
        // (that helper truncates safe content to 800 chars and is for diagnostics).
        serde_json::to_string(line).map_err(|err| {
            AppError::new(
                "page_jsonl_serialize_failed",
                "页面 JSONL 序列化失败。",
                "export",
                false,
            )
            .with_details(err.to_string())
        })
    }

    pub fn sync_analysis_image_path(
        conn: &mut SqliteConnection,
        analysis: &mut PageAnalysisV1,
    ) -> AppResult<()> {
        let page =
            DocumentRepository::find_page_by_id(conn, &analysis.page_id)?.ok_or_else(|| {
                AppError::new(
                    "page_not_found",
                    "找不到页面记录以同步图片路径。",
                    "export",
                    false,
                )
            })?;
        if let Some(asset) = DocumentRepository::find_image_asset_by_hash(conn, &page.image_hash)? {
            analysis.image_path = asset.file_path;
        }
        Ok(())
    }

    fn analysis_line_for_page(
        conn: &mut SqliteConnection,
        page: &PageRecordDto,
        image_path: &str,
    ) -> AppResult<Option<PageJsonlLine>> {
        let Some(mut analysis) =
            AnalysisRepository::find_succeeded_page_analysis(conn, &page.page_id)?
        else {
            return Ok(None);
        };

        if analysis.page_id != page.page_id {
            return Err(AppError::new(
                "stored_analysis_page_id_mismatch",
                "账本中的页面分析 page_id 不一致。",
                "export",
                false,
            )
            .with_details(format!(
                "expected={}; actual={}",
                page.page_id, analysis.page_id
            )));
        }

        if !image_path.is_empty() {
            analysis.image_path = image_path.to_string();
        } else {
            Self::sync_analysis_image_path(conn, &mut analysis)?;
        }

        Ok(Some(PageJsonlLine::Analysis(analysis)))
    }
}

#[cfg(test)]
mod tests {
    use super::{PageJsonExporter, PageJsonlLine};
    use crate::domain::analysis::{
        PageAnalysisContent, PageAnalysisModelInfo, PageAnalysisSource, PageAnalysisV1,
        PageRetrievalFields, PAGE_ANALYSIS_SCHEMA_VERSION,
    };
    use crate::repositories::analysis_repository::AnalysisRepository;
    use crate::repositories::db::{block_on_db, connect_workspace_db, run_migrations};
    use crate::repositories::document_repository::DocumentRepository;
    use std::fs;

    fn test_connection() -> (sqlx::SqliteConnection, std::path::PathBuf) {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "slicer-page-json-export-{}-{nonce}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("temp root");
        let db_path = root.join("app.db");
        block_on_db(run_migrations(db_path.clone())).expect("migrations");
        let conn = block_on_db(connect_workspace_db(db_path)).expect("connection");
        (conn, root)
    }

    fn seed_page(conn: &mut sqlx::SqliteConnection) -> String {
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
        let page = DocumentRepository::create_page_record(conn, &doc.document_id, 1, "image-hash")
            .expect("page");
        page.page_id
    }

    fn valid_analysis_json(page_id: &str) -> String {
        format!(
            r#"{{
  "schema_version": "{PAGE_ANALYSIS_SCHEMA_VERSION}",
  "page_id": "{page_id}",
  "image_hash": "image-hash",
  "image_path": "pages/doc/image.png",
  "source": {{
    "document_id": "doc",
    "page_number": 1,
    "original_filename": "sample.pdf"
  }},
  "analysis": {{
    "title": "标题",
    "summary": "摘要",
    "visible_text": "正文",
    "topics": ["主题"],
    "keywords": ["关键词"]
  }},
  "retrieval": {{
    "bm25_text": "标题 摘要 正文"
  }},
  "model": {{
    "provider": "local_mock",
    "model_name": "mock"
  }}
}}"#
        )
    }

    #[test]
    fn exports_baseline_when_page_not_analyzed() {
        let (mut conn, root) = test_connection();
        seed_page(&mut conn);

        let lines = PageJsonExporter::build_lines(&mut conn).expect("lines");
        assert_eq!(lines.len(), 1);
        match &lines[0] {
            PageJsonlLine::Baseline(baseline) => {
                assert_eq!(baseline.image_path, "pages/doc/image.png");
                assert_eq!(baseline.status, "rendered");
            }
            PageJsonlLine::Analysis(_) => panic!("expected baseline"),
        }

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn exports_page_analysis_v1_when_succeeded() {
        let (mut conn, root) = test_connection();
        let page_id = seed_page(&mut conn);

        AnalysisRepository::save_success_result(
            &mut conn,
            &page_id,
            PAGE_ANALYSIS_SCHEMA_VERSION,
            "local_mock",
            "mock",
            &valid_analysis_json(&page_id),
        )
        .expect("save");

        let lines = PageJsonExporter::build_lines(&mut conn).expect("lines");
        match &lines[0] {
            PageJsonlLine::Analysis(analysis) => {
                assert_eq!(analysis.schema_version, PAGE_ANALYSIS_SCHEMA_VERSION);
                assert_eq!(analysis.retrieval.bm25_text, "标题 摘要 正文");
            }
            PageJsonlLine::Baseline(_) => panic!("expected analysis"),
        }

        let serialized = PageJsonExporter::serialize_line(&lines[0]).expect("serialize");
        assert!(!serialized.contains("api_key"));
        assert!(!serialized.contains("Authorization"));

        let _ = fs::remove_dir_all(root);
    }

    fn seed_error(conn: &mut sqlx::SqliteConnection, error_id: &str) {
        block_on_db(async {
            sqlx::query(
                "INSERT INTO errors (error_id, code, message, stage, retryable, details, correlation_id, created_at)
                 VALUES (?1, 'analysis_failed', '失败', 'analysis', 1, NULL, ?2, ?3)",
            )
            .bind(error_id)
            .bind(format!("correlation-{error_id}"))
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(&mut *conn)
            .await
            .map_err(|err| crate::repositories::db::database_error("test", "seed_error", err))?;
            Ok(())
        })
        .expect("seed error");
    }

    #[test]
    fn failed_analysis_exports_baseline_not_raw_result() {
        let (mut conn, root) = test_connection();
        let page_id = seed_page(&mut conn);

        seed_error(&mut conn, "err-1");

        AnalysisRepository::save_failure_result(
            &mut conn,
            &page_id,
            PAGE_ANALYSIS_SCHEMA_VERSION,
            "local_mock",
            "mock",
            "err-1",
        )
        .expect("failure");

        let lines = PageJsonExporter::build_lines(&mut conn).expect("lines");
        assert!(matches!(lines[0], PageJsonlLine::Baseline(_)));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn serialize_line_does_not_truncate_long_safe_content() {
        let long_text = "长".repeat(900);
        let line = PageJsonlLine::Analysis(PageAnalysisV1 {
            schema_version: PAGE_ANALYSIS_SCHEMA_VERSION.to_string(),
            page_id: "p1".to_string(),
            image_hash: "h1".to_string(),
            image_path: "pages/d/h1.png".to_string(),
            source: PageAnalysisSource {
                document_id: "d1".to_string(),
                page_number: 1,
                original_filename: None,
            },
            analysis: PageAnalysisContent {
                title: None,
                summary: None,
                visible_text: Some(long_text.clone()),
                topics: vec![],
                keywords: vec![],
            },
            retrieval: PageRetrievalFields {
                bm25_text: long_text,
            },
            model: PageAnalysisModelInfo {
                provider: "local_mock".to_string(),
                model_name: "mock".to_string(),
            },
            provider_response: None,
        });

        let serialized = PageJsonExporter::serialize_line(&line).expect("serialize");
        assert!(serialized.len() > 800);
        assert!(serialized.contains("page_analysis_v1"));
    }
}
