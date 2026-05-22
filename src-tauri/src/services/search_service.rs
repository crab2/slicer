use crate::artifacts::index_store::{read_active_pointer, write_active_pointer};
use crate::artifacts::workspace_layout::WorkspaceLayout;
use crate::domain::analysis::PageAnalysisV1;
use crate::domain::index::{
    ActiveIndexPointer, IndexRebuildResultDto, IndexRebuildStartDto, IndexStatusDto,
    SearchIndexDocument, SearchResponseDto, SearchResultItemDto, DEFAULT_SEARCH_PROVIDER_ID,
    TANTIVY_ANALYZER_VERSION,
};
use crate::errors::{AppError, AppResult};
use crate::jobs::job_orchestrator::JobOrchestrator;
use crate::providers::search::search_provider::SearchProvider;
use crate::providers::search::tantivy_bm25_provider::TantivyBm25SearchProvider;
use crate::repositories::analysis_repository::AnalysisRepository;
use crate::repositories::db::block_on_db;
use crate::repositories::document_repository::DocumentRepository;
use crate::repositories::index_repository::IndexRepository;
use crate::services::workspace_service::WorkspaceService;
use base64::{engine::general_purpose, Engine as _};
use serde_json::to_string_pretty;
use sqlx::SqliteConnection;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::thread;

pub struct SearchService;

impl SearchService {
    pub fn get_index_status(workspace: &WorkspaceService) -> AppResult<IndexStatusDto> {
        let layout = workspace.workspace_layout()?;
        let mut conn = workspace.get_db_connection()?;
        IndexRepository::recover_stale_building_versions(
            &mut conn,
            IndexRepository::default_provider(),
        )?;
        Self::build_index_status(workspace, &layout, &mut conn)
    }

    pub fn search(
        workspace: &WorkspaceService,
        query: &str,
        limit: usize,
    ) -> AppResult<SearchResponseDto> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(SearchResponseDto {
                items: Vec::new(),
                query: String::new(),
                limit: limit.clamp(1, 100),
            });
        }
        let limit = limit.clamp(1, 100);
        let layout = workspace.workspace_layout()?;
        let mut conn = workspace.get_db_connection()?;
        let active =
            IndexRepository::find_active_version(&mut conn, IndexRepository::default_provider())?;
        let Some(active) = active else {
            return Err(AppError::new(
                "index_not_ready",
                "索引尚未建立，请先构建索引。",
                "search",
                true,
            ));
        };
        let index_path = layout.root().join(&active.index_directory);
        let provider = TantivyBm25SearchProvider;
        provider.health_check(&index_path)?;
        let hits = provider.search(&index_path, trimmed, limit)?;
        let mut items = Vec::with_capacity(hits.len());
        for hit in hits {
            items.push(Self::assemble_result_item(
                workspace,
                &layout,
                &mut conn,
                &hit.page_id,
                hit.score,
            )?);
        }
        Ok(SearchResponseDto {
            items,
            query: trimmed.to_string(),
            limit,
        })
    }

    pub fn start_index_rebuild(workspace: &WorkspaceService) -> AppResult<IndexRebuildStartDto> {
        let layout = workspace.workspace_layout()?;
        let orchestrator = JobOrchestrator::new(layout.clone());
        let job = orchestrator.create_job("index_rebuild")?;
        let mut conn = workspace.get_db_connection()?;
        IndexRepository::recover_stale_building_versions(
            &mut conn,
            IndexRepository::default_provider(),
        )?;
        let documents = Self::collect_index_documents(&mut conn)?;
        if documents.is_empty() {
            return Err(AppError::new(
                "index_no_documents",
                "没有可索引的页面。请先在「模型分析」中完成页面分析，再构建索引。",
                "index",
                false,
            ));
        }
        let version_id = uuid::Uuid::new_v4().to_string();
        let build_dir = layout.bm25_build_dir(&version_id);
        let relative_dir = build_dir
            .strip_prefix(layout.root())
            .map_err(|_| AppError::new("index_path_invalid", "索引目录路径无效。", "index", false))?
            .to_string_lossy()
            .replace('\\', "/");
        let version = IndexRepository::create_build_version(
            &mut conn,
            &version_id,
            IndexRepository::default_provider(),
            TANTIVY_ANALYZER_VERSION,
            &relative_dir,
        )?;
        orchestrator.update_progress(&job.job_id, 5, Some("索引重建已开始"))?;
        let workspace = workspace.clone();
        let job_id = job.job_id.clone();
        let rebuild_version_id = version.version_id.clone();
        let spawn_job_id = job_id.clone();
        let spawn_version_id = rebuild_version_id.clone();
        thread::spawn(move || {
            let _ = Self::run_index_rebuild(&workspace, &spawn_job_id, &spawn_version_id);
        });
        Ok(IndexRebuildStartDto {
            job_id,
            version_id: rebuild_version_id,
        })
    }

    pub fn get_page_image_preview(
        workspace: &WorkspaceService,
        page_id: &str,
    ) -> AppResult<Option<String>> {
        let layout = workspace.workspace_layout()?;
        let mut conn = workspace.get_db_connection()?;
        let analysis = AnalysisRepository::find_succeeded_page_analysis(&mut conn, page_id)?;
        let Some(analysis) = analysis else {
            return Ok(None);
        };
        let image_path = workspace_image_path(&layout, &analysis.image_path)?;
        let Some(image_path) = image_path else {
            return Ok(None);
        };
        let bytes = fs::read(&image_path).map_err(|err| {
            AppError::io("search", "search_preview_image_read_failed", err)
                .with_details(image_path.display().to_string())
        })?;
        let mime = image_mime_type(&image_path);
        let encoded = general_purpose::STANDARD.encode(bytes);
        Ok(Some(format!("data:{mime};base64,{encoded}")))
    }

    fn run_index_rebuild(
        workspace: &WorkspaceService,
        job_id: &str,
        version_id: &str,
    ) -> AppResult<IndexRebuildResultDto> {
        let layout = workspace.workspace_layout()?;
        let orchestrator = JobOrchestrator::new(layout.clone());
        let result = (|| {
            orchestrator.update_progress(job_id, 15, Some("正在收集可索引页面"))?;
            let mut conn = workspace.get_db_connection()?;
            let documents = Self::collect_index_documents(&mut conn)?;
            if documents.is_empty() {
                return Err(AppError::new(
                    "index_no_documents",
                    "没有可索引的页面。请先在「模型分析」中完成页面分析，再构建索引。",
                    "index",
                    false,
                ));
            }
            let skipped = Self::count_analyzed_pages(&mut conn)? - documents.len() as i64;
            let version =
                IndexRepository::find_version(&mut conn, version_id)?.ok_or_else(|| {
                    AppError::new(
                        "index_version_missing",
                        "索引版本记录不存在。",
                        "index",
                        false,
                    )
                })?;
            let build_path = layout.root().join(&version.index_directory);
            orchestrator.update_progress(job_id, 40, Some("正在写入 BM25 索引"))?;
            let provider = TantivyBm25SearchProvider;
            let stats = provider.build_index(&build_path, &documents)?;
            orchestrator.update_progress(job_id, 80, Some("正在验证新索引"))?;
            provider.health_check(&build_path)?;
            let sample_query = documents
                .first()
                .and_then(|doc| doc.title.clone())
                .unwrap_or_else(|| "test".to_string());
            let _ = provider.search(&build_path, &sample_query, 1)?;
            IndexRepository::mark_version_ready(
                &mut conn,
                version_id,
                stats.document_count as i64,
            )?;
            let pointer = ActiveIndexPointer {
                version_id: version_id.to_string(),
                provider: DEFAULT_SEARCH_PROVIDER_ID.to_string(),
                analyzer_version: TANTIVY_ANALYZER_VERSION.to_string(),
            };
            write_active_pointer(&layout.bm25_active_pointer_path(), &pointer)?;
            IndexRepository::set_active_version(
                &mut conn,
                IndexRepository::default_provider(),
                version_id,
            )?;
            orchestrator.update_progress(job_id, 100, Some("索引重建完成"))?;
            Ok(IndexRebuildResultDto {
                job_id: job_id.to_string(),
                version_id: version_id.to_string(),
                status: "succeeded".to_string(),
                indexed_pages: stats.document_count as i64,
                skipped_pages: skipped.max(0),
                failed_pages: 0,
                error_summary: None,
                updated_at: chrono::Utc::now().to_rfc3339(),
            })
        })();

        match result {
            Ok(ok) => Ok(ok),
            Err(err) => {
                let mut conn = workspace.get_db_connection().ok();
                if let Some(conn) = conn.as_mut() {
                    let error_id = orchestrator.record_error(&err).ok();
                    let _ =
                        IndexRepository::mark_version_failed(conn, version_id, error_id.as_deref());
                }
                let _ = orchestrator.mark_failed(job_id, &err, &err.message);
                Err(err)
            }
        }
    }

    fn build_index_status(
        _workspace: &WorkspaceService,
        layout: &WorkspaceLayout,
        conn: &mut SqliteConnection,
    ) -> AppResult<IndexStatusDto> {
        let provider = IndexRepository::default_provider();
        let active = IndexRepository::find_active_version(conn, provider)?;
        let building = IndexRepository::list_building_versions(conn, provider)?;
        let pointer = read_active_pointer(&layout.bm25_active_pointer_path())?;
        let analyzable = Self::count_indexable_pages(conn)?;
        let indexed = active.as_ref().map(|v| v.document_count).unwrap_or(0);
        let pending = (analyzable - indexed).max(0);
        let stale = pending > 0;
        let building_version_id = building.first().map(|v| v.version_id.clone());
        let latest_failed = IndexRepository::find_latest_failed_version(conn, provider)?;
        let mut status = if building_version_id.is_some() {
            "building".to_string()
        } else if active.is_some() {
            "ready".to_string()
        } else if pointer.is_some() {
            "needs_rebuild".to_string()
        } else if latest_failed.is_some() {
            "failed".to_string()
        } else {
            "not_built".to_string()
        };
        let mut error_summary = latest_failed
            .as_ref()
            .map(|version| Self::error_message_for_version(conn, version))
            .transpose()?
            .flatten();
        let correlation_id = latest_failed
            .as_ref()
            .map(|version| Self::correlation_id_for_version(conn, version))
            .transpose()?
            .flatten();
        if let Some(active_version) = &active {
            let index_path = layout.root().join(&active_version.index_directory);
            if TantivyBm25SearchProvider.health_check(&index_path).is_err() {
                status = "failed".to_string();
                error_summary = Some("活动索引无法打开，请重建索引。".to_string());
            }
        }
        let can_search = status == "ready" || (active.is_some() && status == "building");
        Ok(IndexStatusDto {
            status,
            provider: provider.to_string(),
            active_version_id: active.as_ref().map(|v| v.version_id.clone()),
            indexed_page_count: indexed,
            analyzable_page_count: analyzable,
            pending_index_page_count: pending,
            building_version_id,
            building_job_id: None,
            error_summary,
            correlation_id,
            can_search,
            can_rebuild: building.is_empty() && analyzable > 0,
            stale,
            stale_reason: if analyzable == 0 {
                Some("尚无已分析页面，请先完成模型分析".to_string())
            } else if stale {
                Some(format!("有 {pending} 个已分析页面尚未纳入当前索引"))
            } else {
                None
            },
            search_uses_stale_index: can_search && stale,
        })
    }

    fn collect_index_documents(conn: &mut SqliteConnection) -> AppResult<Vec<SearchIndexDocument>> {
        let analyses = AnalysisRepository::list_current_succeeded_analyses(conn)?;
        let mut documents = Vec::with_capacity(analyses.len());
        for analysis in analyses {
            let document = SearchIndexDocument::from_analysis(&analysis);
            if document.combined_index_text().trim().is_empty() {
                continue;
            }
            documents.push(document);
        }
        Ok(documents)
    }

    fn count_analyzed_pages(conn: &mut SqliteConnection) -> AppResult<i64> {
        let pages = DocumentRepository::list_all_pages(conn)?;
        Ok(pages
            .into_iter()
            .filter(|page| page.status == "analyzed")
            .count() as i64)
    }

    fn count_indexable_pages(conn: &mut SqliteConnection) -> AppResult<i64> {
        Ok(Self::collect_index_documents(conn)?.len() as i64)
    }

    fn error_message_for_version(
        conn: &mut SqliteConnection,
        version: &crate::domain::index::IndexVersionDto,
    ) -> AppResult<Option<String>> {
        let Some(error_id) = version.error_id.as_deref() else {
            return Ok(None);
        };
        block_on_db(async {
            sqlx::query_scalar::<_, String>("SELECT message FROM errors WHERE error_id = ?1")
                .bind(error_id)
                .fetch_optional(conn)
                .await
                .map_err(|err| {
                    crate::errors::AppError::new(
                        "index_error_lookup_failed",
                        "读取索引错误信息失败。",
                        "index",
                        false,
                    )
                    .with_details(err.to_string())
                })
        })
    }

    fn correlation_id_for_version(
        conn: &mut SqliteConnection,
        version: &crate::domain::index::IndexVersionDto,
    ) -> AppResult<Option<String>> {
        let Some(error_id) = version.error_id.as_deref() else {
            return Ok(None);
        };
        block_on_db(async {
            sqlx::query_scalar::<_, String>("SELECT correlation_id FROM errors WHERE error_id = ?1")
                .bind(error_id)
                .fetch_optional(conn)
                .await
                .map_err(|err| {
                    crate::errors::AppError::new(
                        "index_error_lookup_failed",
                        "读取索引错误信息失败。",
                        "index",
                        false,
                    )
                    .with_details(err.to_string())
                })
        })
    }

    fn assemble_result_item(
        _workspace: &WorkspaceService,
        layout: &WorkspaceLayout,
        conn: &mut SqliteConnection,
        page_id: &str,
        score: f32,
    ) -> AppResult<SearchResultItemDto> {
        let analysis = AnalysisRepository::find_succeeded_page_analysis(conn, page_id)?;
        let Some(analysis) = analysis else {
            return Err(AppError::new(
                "search_result_missing_analysis",
                "搜索结果对应的页面分析不存在。",
                "search",
                false,
            ));
        };
        let image_abs = layout.root().join(&analysis.image_path);
        let image_available = image_abs.is_file();
        let image_path = if image_available {
            Some(path_to_string(&image_abs))
        } else {
            None
        };
        let page_json = to_string_pretty(&analysis).map_err(|err| {
            AppError::new(
                "search_result_json_failed",
                "搜索结果 JSON 序列化失败。",
                "search",
                false,
            )
            .with_details(err.to_string())
        })?;
        Ok(SearchResultItemDto {
            page_id: analysis.page_id.clone(),
            document_id: analysis.source.document_id.clone(),
            page_number: analysis.source.page_number,
            original_filename: analysis.source.original_filename.clone(),
            score,
            title: analysis.analysis.title.clone(),
            summary: analysis.analysis.summary.clone(),
            image_path,
            image_available,
            page_json,
        })
    }
}

impl SearchIndexDocument {
    fn from_analysis(analysis: &PageAnalysisV1) -> Self {
        Self {
            page_id: analysis.page_id.clone(),
            document_id: analysis.source.document_id.clone(),
            page_number: analysis.source.page_number,
            image_path: analysis.image_path.clone(),
            original_filename: analysis.source.original_filename.clone(),
            title: analysis.analysis.title.clone(),
            summary: analysis.analysis.summary.clone(),
            visible_text: analysis.analysis.visible_text.clone(),
            topics: analysis.analysis.topics.clone(),
            keywords: analysis.analysis.keywords.clone(),
            bm25_text: analysis.retrieval.bm25_text.clone(),
        }
    }
}

fn path_to_string(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn workspace_image_path(
    layout: &WorkspaceLayout,
    relative_path: &str,
) -> AppResult<Option<PathBuf>> {
    let relative = Path::new(relative_path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Ok(None);
    }

    let image_path = layout.root().join(relative);
    if !image_path.is_file() {
        return Ok(None);
    }
    let root = fs::canonicalize(layout.root()).map_err(|err| {
        AppError::io("search", "search_workspace_path_invalid", err)
            .with_details(layout.root().display().to_string())
    })?;
    let image_path = fs::canonicalize(&image_path).map_err(|err| {
        AppError::io("search", "search_preview_image_path_invalid", err)
            .with_details(image_path.display().to_string())
    })?;
    if !image_path.starts_with(root) {
        return Err(AppError::new(
            "search_preview_image_outside_workspace",
            "页面图片路径不在当前工作区内。",
            "search",
            false,
        ));
    }
    Ok(Some(image_path))
}

fn image_mime_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => "image/png",
    }
}

#[cfg(test)]
mod tests {
    use crate::providers::search::mock_search_provider::MockSearchProvider;
    use crate::providers::search::search_provider::SearchProvider;
    use crate::repositories::db::{block_on_db, connect_workspace_db, run_migrations};
    use crate::repositories::index_repository::IndexRepository;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn mock_provider_allows_search_service_level_tests() {
        let provider = MockSearchProvider::new();
        provider.set_hits("合同", vec![("page-1".to_string(), 2.5)]);
        let hits = provider
            .search(PathBuf::from("/tmp/mock").as_path(), "合同", 5)
            .expect("search");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].page_id, "page-1");
    }

    #[test]
    fn index_status_defaults_to_not_built() {
        let root = std::env::temp_dir().join(format!("slicer-index-status-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("dir");
        let db_path = root.join("app.db");
        let mut conn = block_on_db(async {
            run_migrations(db_path.clone()).await?;
            connect_workspace_db(db_path).await
        })
        .expect("connect");
        let active =
            IndexRepository::find_active_version(&mut conn, IndexRepository::default_provider())
                .expect("lookup");
        assert!(active.is_none());
        let _ = fs::remove_dir_all(root);
    }
}
