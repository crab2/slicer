use crate::artifacts::index_store::{read_active_pointer, write_active_pointer};
use crate::artifacts::workspace_layout::WorkspaceLayout;
use crate::domain::analysis::PageAnalysisV1;
use crate::domain::index::{
    legacy_page_hit_id, module_hit_id, ActiveIndexPointer, IndexRebuildResultDto,
    IndexRebuildStartDto, IndexStatusDto, SearchHitDto, SearchIndexDocument, SearchResponseDto,
    SearchResultItemDto, SearchResultPageDto, DEFAULT_SEARCH_PROVIDER_ID,
    MODULE_INDEX_SCHEMA_VERSION, TANTIVY_ANALYZER_VERSION,
};
use crate::domain::pdf_structure::PdfContentBlockDto;
use crate::errors::{AppError, AppResult};
use crate::jobs::job_orchestrator::JobOrchestrator;
use crate::providers::search::search_provider::SearchProvider;
use crate::providers::search::tantivy_bm25_provider::TantivyBm25SearchProvider;
use crate::repositories::analysis_repository::AnalysisRepository;
use crate::repositories::db::block_on_db;
use crate::repositories::document_repository::DocumentRepository;
use crate::repositories::index_repository::IndexRepository;
use crate::repositories::pdf_structure_repository::PdfStructureRepository;
use crate::services::workspace_service::WorkspaceService;
use base64::{engine::general_purpose, Engine as _};
use serde_json::{to_string_pretty, Map, Value};
use sha2::{Digest, Sha256};
use sqlx::SqliteConnection;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::thread;

pub struct SearchService;

static INDEX_REBUILD_STATE_LOCK: Mutex<()> = Mutex::new(());

const MODULE_SNAPSHOT_TEXT_CHARS: usize = 4_000;
const MODULE_SNAPSHOT_ENRICHMENT_CHARS: usize = 2_000;
const MODULE_SNAPSHOT_KEYWORD_CHARS: usize = 120;
const MODULE_SNAPSHOT_KEYWORDS: usize = 32;

impl SearchService {
    pub fn get_index_status(workspace: &WorkspaceService) -> AppResult<IndexStatusDto> {
        let _state_guard = INDEX_REBUILD_STATE_LOCK.lock().map_err(|_| {
            AppError::new(
                "index_rebuild_lock_poisoned",
                "索引重建状态锁不可用，请重启应用后重试。",
                "index",
                true,
            )
        })?;
        let layout = workspace.workspace_layout()?;
        let orchestrator = JobOrchestrator::new(layout.clone());
        let building_job_id = Self::live_index_rebuild_job_id(&orchestrator)?;
        let mut conn = workspace.get_db_connection()?;
        if building_job_id.is_none() {
            IndexRepository::recover_stale_building_versions(
                &mut conn,
                IndexRepository::default_provider(),
            )?;
        }
        Self::build_index_status(workspace, &layout, &mut conn, building_job_id)
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
        let index_path = Self::validated_index_path(&layout, &active.index_directory)?;
        let provider = TantivyBm25SearchProvider;
        provider.health_check(&index_path)?;
        let hits = provider.search(&index_path, trimmed, limit)?;
        let mut items = Vec::with_capacity(hits.len());
        for hit in hits {
            if let Some(item) = Self::assemble_result_item(workspace, &layout, &mut conn, &hit)? {
                items.push(item);
            }
        }
        Ok(SearchResponseDto {
            items,
            query: trimmed.to_string(),
            limit,
        })
    }

    pub fn start_index_rebuild(workspace: &WorkspaceService) -> AppResult<IndexRebuildStartDto> {
        let _start_guard = INDEX_REBUILD_STATE_LOCK.lock().map_err(|_| {
            AppError::new(
                "index_rebuild_lock_poisoned",
                "索引重建准入锁不可用，请重启应用后重试。",
                "index",
                true,
            )
        })?;
        let layout = workspace.workspace_layout()?;
        let orchestrator = JobOrchestrator::new(layout.clone());
        let mut conn = workspace.get_db_connection()?;
        if Self::live_index_rebuild_job_id(&orchestrator)?.is_some() {
            return Err(AppError::new(
                "index_rebuild_in_progress",
                "已有索引重建任务正在运行。",
                "index",
                true,
            ));
        }
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
        Self::validated_index_path(&layout, &relative_dir)?;
        let version = IndexRepository::create_build_version_with_schema(
            &mut conn,
            &version_id,
            IndexRepository::default_provider(),
            TANTIVY_ANALYZER_VERSION,
            MODULE_INDEX_SCHEMA_VERSION,
            &relative_dir,
        )?;
        let job = orchestrator.create_job("index_rebuild")?;
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
        let image_path = match AnalysisRepository::find_succeeded_page_analysis(&mut conn, page_id)?
        {
            Some(analysis) => analysis.image_path,
            None => {
                let Some(page) = DocumentRepository::find_page_by_id(&mut conn, page_id)? else {
                    return Ok(None);
                };
                let Some(image_hash) = page.image_hash.as_deref() else {
                    return Ok(None);
                };
                let Some(asset) =
                    DocumentRepository::find_image_asset_by_hash(&mut conn, image_hash)?
                else {
                    return Ok(None);
                };
                asset.file_path
            }
        };
        let image_path = workspace_image_path(&layout, &image_path)?;
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
            let skipped = 0;
            let content_fingerprint = Self::index_content_fingerprint(&documents)?;
            let version =
                IndexRepository::find_version(&mut conn, version_id)?.ok_or_else(|| {
                    AppError::new(
                        "index_version_missing",
                        "索引版本记录不存在。",
                        "index",
                        false,
                    )
                })?;
            let build_path = Self::validated_index_path(&layout, &version.index_directory)?;
            orchestrator.update_progress(job_id, 40, Some("正在写入 BM25 索引"))?;
            let provider = TantivyBm25SearchProvider;
            let stats = provider.build_index(&build_path, &documents)?;
            orchestrator.update_progress(job_id, 80, Some("正在验证新索引"))?;
            provider.validate_build(&build_path, stats.document_count)?;
            IndexRepository::activate_version_and_complete_job(
                &mut conn,
                IndexRepository::default_provider(),
                version_id,
                stats.document_count as i64,
                &content_fingerprint,
                job_id,
                "索引重建完成",
            )?;
            let pointer = ActiveIndexPointer {
                version_id: version_id.to_string(),
                provider: DEFAULT_SEARCH_PROVIDER_ID.to_string(),
                analyzer_version: TANTIVY_ANALYZER_VERSION.to_string(),
            };
            if let Err(error) = write_active_pointer(&layout.bm25_active_pointer_path(), &pointer) {
                tracing::warn!(
                    target: "index",
                    version_id,
                    error = %error,
                    "active index pointer write failed after SQLite activation"
                );
            }
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

    fn live_index_rebuild_job_id(orchestrator: &JobOrchestrator) -> AppResult<Option<String>> {
        Ok(orchestrator
            .list_jobs()?
            .into_iter()
            .find(|job| {
                job.job_type == "index_rebuild"
                    && matches!(job.status.as_str(), "queued" | "running")
            })
            .map(|job| job.job_id))
    }

    fn validated_index_path(layout: &WorkspaceLayout, index_directory: &str) -> AppResult<PathBuf> {
        let relative = Path::new(index_directory);
        let invalid = relative.as_os_str().is_empty()
            || relative.is_absolute()
            || relative.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            });
        let candidate = layout.root().join(relative);
        if invalid || !candidate.starts_with(layout.bm25_index_dir()) {
            return Err(AppError::new(
                "index_directory_invalid",
                "索引版本包含不安全的目录路径。",
                "index",
                false,
            )
            .with_details(index_directory.to_string()));
        }
        Ok(candidate)
    }

    fn failed_version_is_newer_than_active(
        failed: &crate::domain::index::IndexVersionDto,
        active: &crate::domain::index::IndexVersionDto,
    ) -> bool {
        let Some(active_at) = active.activated_at.as_deref() else {
            return true;
        };
        let failed_at = failed
            .build_finished_at
            .as_deref()
            .unwrap_or(&failed.updated_at);
        match (
            chrono::DateTime::parse_from_rfc3339(failed_at),
            chrono::DateTime::parse_from_rfc3339(active_at),
        ) {
            (Ok(failed_at), Ok(active_at)) => failed_at > active_at,
            _ => true,
        }
    }

    fn analyzer_version_is_stale(active: Option<&crate::domain::index::IndexVersionDto>) -> bool {
        active.is_some_and(|version| version.analyzer_version != TANTIVY_ANALYZER_VERSION)
    }

    fn build_index_status(
        _workspace: &WorkspaceService,
        layout: &WorkspaceLayout,
        conn: &mut SqliteConnection,
        building_job_id: Option<String>,
    ) -> AppResult<IndexStatusDto> {
        let provider = IndexRepository::default_provider();
        let active = IndexRepository::find_active_version(conn, provider)?;
        let building = IndexRepository::list_building_versions(conn, provider)?;
        let pointer = match read_active_pointer(&layout.bm25_active_pointer_path()) {
            Ok(pointer) => pointer,
            Err(error) => {
                tracing::warn!(
                    target: "index",
                    error = %error,
                    "ignoring corrupt non-authoritative active index pointer"
                );
                None
            }
        };
        let indexable = Self::collect_index_document_set(conn)?;
        let analyzable = indexable.documents.len() as i64;
        let content_fingerprint = Self::index_content_fingerprint(&indexable.documents)?;
        let indexed = active.as_ref().map(|v| v.document_count).unwrap_or(0);
        let pending = (analyzable - indexed).max(0);
        let count_stale = active.is_some() && analyzable != indexed;
        let schema_stale = indexable.module_count > 0
            && active.as_ref().is_some_and(|version| {
                version.content_schema_version != MODULE_INDEX_SCHEMA_VERSION
            });
        let analyzer_stale = Self::analyzer_version_is_stale(active.as_ref());
        let content_stale = active
            .as_ref()
            .is_some_and(|version| version.content_fingerprint != content_fingerprint);
        let stale = count_stale || schema_stale || analyzer_stale || content_stale;
        let building_version_id = building.first().map(|v| v.version_id.clone());
        let latest_failed =
            IndexRepository::find_latest_failed_version(conn, provider)?.filter(|failed| {
                active.as_ref().is_none_or(|active_version| {
                    Self::failed_version_is_newer_than_active(failed, active_version)
                })
            });
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
            let index_health = Self::validated_index_path(layout, &active_version.index_directory)
                .and_then(|index_path| TantivyBm25SearchProvider.health_check(&index_path));
            if index_health.is_err() {
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
            building_job_id,
            error_summary,
            correlation_id,
            can_search,
            can_rebuild: building.is_empty() && analyzable > 0,
            stale,
            stale_reason: if analyzable == 0 {
                Some("尚无已分析页面，请先完成模型分析".to_string())
            } else if schema_stale {
                Some("当前活动索引仍是页级版本，结构化模块尚未激活。".to_string())
            } else if analyzer_stale {
                Some("当前活动索引的分词器版本已过期，请重建索引。".to_string())
            } else if count_stale && pending == 0 {
                Some("当前活动索引仍包含已替换或已移除的检索单元。".to_string())
            } else if content_stale {
                Some("结构化模块或视觉描述已更新，请重建索引。".to_string())
            } else if stale {
                Some(format!("有 {pending} 个检索单元尚未纳入当前索引"))
            } else {
                None
            },
            search_uses_stale_index: can_search && stale,
        })
    }

    fn collect_index_documents(conn: &mut SqliteConnection) -> AppResult<Vec<SearchIndexDocument>> {
        Ok(Self::collect_index_document_set(conn)?.documents)
    }

    fn collect_index_document_set(
        conn: &mut SqliteConnection,
    ) -> AppResult<IndexDocumentCollection> {
        let blocks = PdfStructureRepository::list_indexable_blocks(conn)?;
        let analyses = AnalysisRepository::list_current_succeeded_analyses(conn)?;
        let mut documents = Vec::with_capacity(blocks.len() + analyses.len());
        let mut covered_pages = HashSet::new();
        let mut page_contexts: HashMap<String, PageIndexContext> = HashMap::new();
        let mut module_count = 0;

        for block in blocks {
            if !block.is_indexable {
                continue;
            }
            let index_text = block.index_text();
            if index_text.trim().is_empty() {
                continue;
            }
            let context = if let Some(context) = page_contexts.get(&block.page_id) {
                context.clone()
            } else {
                let context = Self::load_page_index_context(conn, &block.page_id)?;
                page_contexts.insert(block.page_id.clone(), context.clone());
                context
            };
            if context.document_id != block.document_id || context.page_number != block.page_number
            {
                return Err(AppError::new(
                    "index_block_provenance_mismatch",
                    "PDF 模块与持久化页面来源不一致。",
                    "index",
                    false,
                )
                .with_details(block.block_id.clone()));
            }
            let bbox = block.bbox.filter(|bbox| bbox.is_valid());
            let snippet = compact_snippet(&block.source_text, &index_text, 240);
            let title = module_title(&block);
            let summary = module_summary(&block);
            let module_json =
                module_snapshot_json(&block, bbox, title.as_deref(), summary.as_deref())?;
            documents.push(SearchIndexDocument {
                hit_id: module_hit_id(&block.block_id),
                page_id: block.page_id.clone(),
                module_id: Some(block.block_id.clone()),
                module_type: block.block_type.clone(),
                snippet,
                bbox,
                module_json: Some(module_json),
                document_id: block.document_id.clone(),
                page_number: block.page_number,
                image_path: context.image_path,
                original_filename: Some(context.original_filename),
                title,
                summary,
                visible_text: None,
                topics: Vec::new(),
                keywords: Vec::new(),
                bm25_text: index_text,
            });
            covered_pages.insert(block.page_id);
            module_count += 1;
        }

        for analysis in analyses {
            if covered_pages.contains(&analysis.page_id) {
                continue;
            }
            let document = SearchIndexDocument::from_analysis(&analysis);
            if document.combined_index_text().trim().is_empty() {
                continue;
            }
            documents.push(document);
        }
        Ok(IndexDocumentCollection {
            documents,
            module_count,
        })
    }

    fn index_content_fingerprint(documents: &[SearchIndexDocument]) -> AppResult<String> {
        let mut ordered: Vec<&SearchIndexDocument> = documents.iter().collect();
        ordered.sort_by(|left, right| left.hit_id.cmp(&right.hit_id));
        let mut hasher = Sha256::new();
        hasher.update(b"slicer-index-content-v2\0");
        for document in ordered {
            serde_json::to_writer(&mut DigestWriter(&mut hasher), document).map_err(|err| {
                AppError::new(
                    "index_content_fingerprint_failed",
                    "无法计算索引内容版本。",
                    "index",
                    false,
                )
                .with_details(err.to_string())
            })?;
            hasher.update([0]);
        }
        let digest = hasher.finalize();
        Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
    }

    fn load_page_index_context(
        conn: &mut SqliteConnection,
        page_id: &str,
    ) -> AppResult<PageIndexContext> {
        let page = DocumentRepository::find_page_by_id(conn, page_id)?.ok_or_else(|| {
            AppError::new(
                "index_block_page_missing",
                "PDF 模块引用的页面已不存在。",
                "index",
                false,
            )
            .with_details(page_id.to_string())
        })?;
        let document = DocumentRepository::find_document_by_id(conn, &page.document_id)?
            .ok_or_else(|| {
                AppError::new(
                    "index_block_document_missing",
                    "PDF 模块引用的文档已不存在。",
                    "index",
                    false,
                )
                .with_details(page.document_id.clone())
            })?;
        let image_path = match page.image_hash.as_deref() {
            Some(image_hash) => DocumentRepository::find_image_asset_by_hash(conn, image_hash)?
                .map(|asset| asset.file_path)
                .unwrap_or_default(),
            None => String::new(),
        };
        Ok(PageIndexContext {
            document_id: page.document_id,
            page_number: page.page_number,
            image_path,
            original_filename: document.original_filename,
        })
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
        hit: &SearchHitDto,
    ) -> AppResult<Option<SearchResultItemDto>> {
        if let Some(document_id) = hit
            .document_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            let Some(document) = DocumentRepository::find_document_by_id(conn, document_id)? else {
                return Ok(None);
            };
            if document.status != "ready" {
                return Ok(None);
            }
        }
        if let Some(item) = Self::assemble_stored_result_item(layout, hit)? {
            return Ok(Some(item));
        }
        if let Some(module_id) = hit.module_id.as_deref() {
            return Self::assemble_module_result(layout, conn, module_id, hit);
        }
        Self::assemble_legacy_result_item(layout, conn, &hit.page_id, hit.score)
    }

    fn assemble_stored_result_item(
        layout: &WorkspaceLayout,
        hit: &SearchHitDto,
    ) -> AppResult<Option<SearchResultItemDto>> {
        let Some(document_id) = hit
            .document_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        else {
            return Ok(None);
        };
        let Some(page_number) = hit.page_number.filter(|value| *value > 0) else {
            return Ok(None);
        };
        let image_path = match hit.image_path.as_deref() {
            Some(path) => workspace_image_path(layout, path)?.map(|path| path_to_string(&path)),
            None => None,
        };
        let image_available = image_path.is_some();

        let (title, summary, page_json) = if hit.module_id.is_some() {
            let (title, summary) = hit
                .module_json
                .as_deref()
                .map(stored_module_metadata)
                .unwrap_or((None, None));
            let page_json = hit.module_json.clone().unwrap_or_else(|| {
                serde_json::json!({
                    "hit_id": hit.hit_id,
                    "module_id": hit.module_id,
                    "page_id": hit.page_id,
                    "document_id": document_id,
                    "page_number": page_number
                })
                .to_string()
            });
            (title, summary, page_json)
        } else {
            let analysis = hit
                .module_json
                .as_deref()
                .and_then(|json| serde_json::from_str::<PageAnalysisV1>(json).ok());
            let title = analysis
                .as_ref()
                .and_then(|value| value.analysis.title.clone());
            let summary = analysis
                .as_ref()
                .and_then(|value| value.analysis.summary.clone());
            let page_json = hit.module_json.clone().unwrap_or_else(|| {
                serde_json::json!({
                    "schema_version": "legacy_index_snapshot_v1",
                    "hit_id": hit.hit_id,
                    "page_id": hit.page_id,
                    "document_id": document_id,
                    "page_number": page_number,
                    "original_filename": hit.original_filename
                })
                .to_string()
            });
            (title, summary, page_json)
        };

        Ok(Some(SearchResultItemDto {
            hit_id: hit.hit_id.clone(),
            module_id: hit.module_id.clone(),
            module_type: hit.module_type.clone(),
            snippet: hit.snippet.clone(),
            page: SearchResultPageDto {
                page_id: hit.page_id.clone(),
                document_id: document_id.to_string(),
                page_number,
            },
            bbox: hit.bbox.filter(|bbox| bbox.is_valid()),
            module_json: hit.module_id.as_ref().and(hit.module_json.clone()),
            page_id: hit.page_id.clone(),
            document_id: document_id.to_string(),
            page_number,
            original_filename: hit.original_filename.clone(),
            score: hit.score,
            title,
            summary,
            image_path,
            image_available,
            page_json,
        }))
    }

    fn assemble_module_result(
        layout: &WorkspaceLayout,
        conn: &mut SqliteConnection,
        module_id: &str,
        hit: &SearchHitDto,
    ) -> AppResult<Option<SearchResultItemDto>> {
        let Some(block) = PdfStructureRepository::find_block_by_id(conn, module_id)? else {
            return Ok(None);
        };
        if !block.is_indexable || block.page_id != hit.page_id {
            return Ok(None);
        }
        let Some(page) = DocumentRepository::find_page_by_id(conn, &block.page_id)? else {
            return Ok(None);
        };
        let Some(document) = DocumentRepository::find_document_by_id(conn, &page.document_id)?
        else {
            return Ok(None);
        };
        if document.status != "ready" {
            return Ok(None);
        }
        let image_path = match page.image_hash.as_deref() {
            Some(image_hash) => {
                match DocumentRepository::find_image_asset_by_hash(conn, image_hash)? {
                    Some(asset) => workspace_image_path(layout, &asset.file_path)?
                        .map(|path| path_to_string(&path)),
                    None => None,
                }
            }
            None => None,
        };
        let bbox = block.bbox.filter(|bbox| bbox.is_valid());
        let index_text = block.index_text();
        let snippet = compact_snippet(&block.source_text, &index_text, 240);
        let title = module_title(&block);
        let summary = module_summary(&block);
        let module_json = module_snapshot_json(&block, bbox, title.as_deref(), summary.as_deref())?;
        let page_location = SearchResultPageDto {
            page_id: page.page_id.clone(),
            document_id: page.document_id.clone(),
            page_number: page.page_number,
        };
        Ok(Some(SearchResultItemDto {
            hit_id: module_hit_id(&block.block_id),
            module_id: Some(block.block_id.clone()),
            module_type: block.block_type.clone(),
            snippet,
            page: page_location,
            bbox,
            module_json: Some(module_json.clone()),
            page_id: page.page_id,
            document_id: page.document_id,
            page_number: page.page_number,
            original_filename: Some(document.original_filename),
            score: hit.score,
            title,
            summary,
            image_available: image_path.is_some(),
            image_path,
            page_json: module_json,
        }))
    }

    fn assemble_legacy_result_item(
        layout: &WorkspaceLayout,
        conn: &mut SqliteConnection,
        page_id: &str,
        score: f32,
    ) -> AppResult<Option<SearchResultItemDto>> {
        let analysis = AnalysisRepository::find_succeeded_page_analysis(conn, page_id)?;
        let Some(analysis) = analysis else {
            return Ok(None);
        };
        let Some(document) =
            DocumentRepository::find_document_by_id(conn, &analysis.source.document_id)?
        else {
            return Ok(None);
        };
        if document.status != "ready" {
            return Ok(None);
        }
        let image_path =
            workspace_image_path(layout, &analysis.image_path)?.map(|path| path_to_string(&path));
        let image_available = image_path.is_some();
        let page_json = to_string_pretty(&analysis).map_err(|err| {
            AppError::new(
                "search_result_json_failed",
                "搜索结果 JSON 序列化失败。",
                "search",
                false,
            )
            .with_details(err.to_string())
        })?;
        Ok(Some(SearchResultItemDto {
            hit_id: legacy_page_hit_id(&analysis.page_id),
            module_id: None,
            module_type: "page".to_string(),
            snippet: compact_snippet(
                analysis
                    .analysis
                    .visible_text
                    .as_deref()
                    .unwrap_or_default(),
                analysis
                    .analysis
                    .summary
                    .as_deref()
                    .unwrap_or(&analysis.retrieval.bm25_text),
                240,
            ),
            page: SearchResultPageDto {
                page_id: analysis.page_id.clone(),
                document_id: analysis.source.document_id.clone(),
                page_number: analysis.source.page_number,
            },
            bbox: None,
            module_json: None,
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
        }))
    }
}

struct DigestWriter<'a>(&'a mut Sha256);

impl Write for DigestWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0.update(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct IndexDocumentCollection {
    documents: Vec<SearchIndexDocument>,
    module_count: usize,
}

#[derive(Clone)]
struct PageIndexContext {
    document_id: String,
    page_number: i64,
    image_path: String,
    original_filename: String,
}

impl SearchIndexDocument {
    fn from_analysis(analysis: &PageAnalysisV1) -> Self {
        Self {
            hit_id: legacy_page_hit_id(&analysis.page_id),
            page_id: analysis.page_id.clone(),
            module_id: None,
            module_type: "page".to_string(),
            snippet: compact_snippet(
                analysis
                    .analysis
                    .visible_text
                    .as_deref()
                    .unwrap_or_default(),
                analysis
                    .analysis
                    .summary
                    .as_deref()
                    .unwrap_or(&analysis.retrieval.bm25_text),
                240,
            ),
            bbox: None,
            module_json: serde_json::to_string(analysis).ok(),
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

fn module_title(block: &PdfContentBlockDto) -> Option<String> {
    let block_type = block.block_type.to_ascii_lowercase();
    if block_type.contains("title") || block_type.contains("heading") {
        let title = compact_snippet(&block.source_text, "", 120);
        (!title.is_empty()).then_some(title)
    } else {
        None
    }
}

fn module_summary(block: &PdfContentBlockDto) -> Option<String> {
    let enrichment = block.enrichment_json.as_deref()?;
    let value = serde_json::from_str::<Value>(enrichment).ok()?;
    ["summary", "description", "caption"]
        .into_iter()
        .find_map(|key| value.get(key).and_then(Value::as_str))
        .map(|text| compact_snippet(text, "", 240))
        .filter(|text| !text.is_empty())
}

fn module_snapshot_json(
    block: &PdfContentBlockDto,
    bbox: Option<crate::domain::pdf_structure::NormalizedBbox>,
    title: Option<&str>,
    summary: Option<&str>,
) -> AppResult<String> {
    let snapshot = serde_json::json!({
        "schema_version": "pdf_module_search_snapshot_v1",
        "block_id": block.block_id,
        "document_id": block.document_id,
        "page_id": block.page_id,
        "page_number": block.page_number,
        "parent_block_id": block.parent_block_id,
        "ordinal": block.ordinal,
        "type": block.block_type,
        "source_text": compact_snippet(&block.source_text, "", MODULE_SNAPSHOT_TEXT_CHARS),
        "title": title,
        "summary": summary,
        "enrichment": compact_module_enrichment(block.enrichment_json.as_deref()),
        "is_visual": block.is_visual,
        "bbox": bbox,
    });
    serde_json::to_string(&snapshot).map_err(|err| {
        AppError::new(
            "index_module_json_failed",
            "无法为搜索索引序列化 PDF 模块。",
            "index",
            false,
        )
        .with_details(err.to_string())
    })
}

fn compact_module_enrichment(enrichment_json: Option<&str>) -> Option<Value> {
    let source = serde_json::from_str::<Value>(enrichment_json?).ok()?;
    let object = source.as_object()?;
    let mut compact = Map::new();
    for key in ["schema_version", "block_id"] {
        if let Some(value) = object.get(key).and_then(Value::as_str) {
            compact.insert(
                key.to_string(),
                Value::String(compact_snippet(value, "", MODULE_SNAPSHOT_KEYWORD_CHARS)),
            );
        }
    }
    for key in ["description", "summary", "visible_text", "caption"] {
        if let Some(value) = object.get(key).and_then(Value::as_str) {
            compact.insert(
                key.to_string(),
                Value::String(compact_snippet(value, "", MODULE_SNAPSHOT_ENRICHMENT_CHARS)),
            );
        }
    }
    if let Some(keywords) = object.get("keywords").and_then(Value::as_array) {
        let keywords = keywords
            .iter()
            .filter_map(Value::as_str)
            .map(|keyword| {
                Value::String(compact_snippet(keyword, "", MODULE_SNAPSHOT_KEYWORD_CHARS))
            })
            .filter(|keyword| keyword.as_str().is_some_and(|value| !value.is_empty()))
            .take(MODULE_SNAPSHOT_KEYWORDS)
            .collect();
        compact.insert("keywords".to_string(), Value::Array(keywords));
    }
    if let Some(model) = object.get("model").and_then(Value::as_object) {
        let mut compact_model = Map::new();
        for key in ["provider", "model_name"] {
            if let Some(value) = model.get(key).and_then(Value::as_str) {
                compact_model.insert(
                    key.to_string(),
                    Value::String(compact_snippet(value, "", MODULE_SNAPSHOT_KEYWORD_CHARS)),
                );
            }
        }
        if !compact_model.is_empty() {
            compact.insert("model".to_string(), Value::Object(compact_model));
        }
    }
    (!compact.is_empty()).then_some(Value::Object(compact))
}

fn stored_module_metadata(module_json: &str) -> (Option<String>, Option<String>) {
    let value = serde_json::from_str::<Value>(module_json).ok();
    let title = value
        .as_ref()
        .and_then(|value| value.get("title"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .filter(|value| !value.is_empty());
    let summary = value
        .as_ref()
        .and_then(|value| value.get("summary"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .filter(|value| !value.is_empty());
    if title.is_some() || summary.is_some() {
        return (title, summary);
    }
    serde_json::from_str::<PdfContentBlockDto>(module_json)
        .ok()
        .map(|block| (module_title(&block), module_summary(&block)))
        .unwrap_or((None, None))
}

fn compact_snippet(primary: &str, fallback: &str, max_chars: usize) -> String {
    let source = if primary.trim().is_empty() {
        fallback
    } else {
        primary
    };
    let mut normalized = String::with_capacity(max_chars.min(source.len()));
    let mut normalized_chars = 0_usize;
    let mut pending_space = false;
    let mut exceeded = false;
    for ch in source.chars() {
        if ch.is_whitespace() {
            pending_space = !normalized.is_empty();
            continue;
        }
        if pending_space {
            if normalized_chars >= max_chars {
                exceeded = true;
                break;
            }
            normalized.push(' ');
            normalized_chars += 1;
            pending_space = false;
        }
        if normalized_chars >= max_chars {
            exceeded = true;
            break;
        }
        normalized.push(ch);
        normalized_chars += 1;
    }
    if !exceeded || max_chars <= 3 {
        return normalized;
    }
    let mut truncated: String = normalized.chars().take(max_chars - 3).collect();
    truncated.push_str("...");
    truncated
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
    use super::{
        compact_snippet, module_snapshot_json, SearchService, INDEX_REBUILD_STATE_LOCK,
        MODULE_SNAPSHOT_ENRICHMENT_CHARS, MODULE_SNAPSHOT_TEXT_CHARS,
    };
    use crate::api::state::ApiAppState;
    use crate::artifacts::workspace_layout::WorkspaceLayout;
    use crate::domain::index::{
        module_hit_id, IndexVersionDto, SearchHitDto, SearchIndexDocument, TANTIVY_ANALYZER_VERSION,
    };
    use crate::domain::pdf_structure::{NormalizedBbox, PdfContentBlockDto};
    use crate::providers::search::mock_search_provider::MockSearchProvider;
    use crate::providers::search::search_provider::SearchProvider;
    use crate::repositories::db::{block_on_db, connect_workspace_db, run_migrations};
    use crate::repositories::document_repository::DocumentRepository;
    use crate::repositories::index_repository::IndexRepository;
    use crate::services::api_server_service::ApiServerService;
    use crate::services::workspace_service::WorkspaceService;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{mpsc, Arc};
    use std::time::Duration;

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
    fn stored_hits_require_ready_document_but_not_page_or_module_rows() {
        let root =
            std::env::temp_dir().join(format!("slicer-index-snapshot-{}", uuid::Uuid::new_v4()));
        let service = WorkspaceService::new(root.join("config"));
        let api = ApiServerService::new(ApiAppState::new(Arc::new(service.clone())));
        let selected =
            service.select_workspace(root.join("workspace").to_string_lossy().into_owned(), &api);
        assert_eq!(selected.status, "ready");
        let layout = service.workspace_layout().expect("layout");
        let mut conn = service.get_db_connection().expect("connection");
        let document = DocumentRepository::create_document(
            &mut conn,
            "old.pdf",
            "pdf",
            "old-hash",
            "originals/old.pdf",
            None,
        )
        .expect("document");
        DocumentRepository::update_document_status(
            &mut conn,
            &document.document_id,
            "ready",
            Some(0),
            None,
        )
        .expect("ready document");
        let image_relative = format!("pages/{}/page.png", document.document_id);
        let image_path = layout.root().join(&image_relative);
        fs::create_dir_all(image_path.parent().expect("image parent")).expect("image parent");
        fs::write(&image_path, b"snapshot").expect("image");
        let hits = [
            SearchHitDto {
                hit_id: "module:old-block".to_string(),
                page_id: "old-page".to_string(),
                module_id: Some("old-block".to_string()),
                module_type: "paragraph".to_string(),
                snippet: "stored module text".to_string(),
                bbox: Some(NormalizedBbox {
                    x: 0.1,
                    y: 0.2,
                    width: 0.3,
                    height: 0.4,
                }),
                module_json: Some(r#"{"block_id":"old-block"}"#.to_string()),
                document_id: Some(document.document_id.clone()),
                page_number: Some(7),
                image_path: Some(image_relative.clone()),
                original_filename: Some("old.pdf".to_string()),
                score: 2.0,
            },
            SearchHitDto {
                hit_id: "page:legacy-page".to_string(),
                page_id: "legacy-page".to_string(),
                module_id: None,
                module_type: "page".to_string(),
                snippet: String::new(),
                bbox: None,
                module_json: None,
                document_id: Some(document.document_id.clone()),
                page_number: Some(8),
                image_path: Some(image_relative),
                original_filename: Some("legacy.pdf".to_string()),
                score: 1.0,
            },
        ];

        for hit in &hits {
            let item = SearchService::assemble_result_item(&service, &layout, &mut conn, hit)
                .expect("assemble snapshot")
                .expect("stored hit");
            assert_eq!(item.hit_id, hit.hit_id);
            assert_eq!(item.document_id, document.document_id);
            assert!(item.image_available);
        }

        DocumentRepository::update_document_status(
            &mut conn,
            &document.document_id,
            "failed",
            Some(0),
            Some("failed"),
        )
        .expect("fail document");
        assert!(
            SearchService::assemble_result_item(&service, &layout, &mut conn, &hits[0],)
                .expect("failed document lookup")
                .is_none()
        );

        DocumentRepository::update_document_status(
            &mut conn,
            &document.document_id,
            "ready",
            Some(0),
            None,
        )
        .expect("restore document");
        DocumentRepository::delete_document_records(&mut conn, &document.document_id)
            .expect("delete document")
            .expect("deleted document");
        assert!(
            SearchService::assemble_result_item(&service, &layout, &mut conn, &hits[0],)
                .expect("deleted document lookup")
                .is_none()
        );

        let _ = fs::remove_dir_all(root);
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

    #[test]
    fn index_status_serializes_with_rebuild_state_changes() {
        let root =
            std::env::temp_dir().join(format!("slicer-index-status-lock-{}", uuid::Uuid::new_v4()));
        let service = WorkspaceService::new(root.join("config"));
        let api = ApiServerService::new(ApiAppState::new(Arc::new(service.clone())));
        let selected =
            service.select_workspace(root.join("workspace").to_string_lossy().into_owned(), &api);
        assert_eq!(selected.status, "ready");

        let state_guard = INDEX_REBUILD_STATE_LOCK.lock().expect("state lock");
        let (sender, receiver) = mpsc::channel();
        let status_service = service.clone();
        let worker = std::thread::spawn(move || {
            sender
                .send(SearchService::get_index_status(&status_service))
                .expect("send status");
        });

        assert!(matches!(
            receiver.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        drop(state_guard);
        let status = receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("status unblocked")
            .expect("status result");
        assert_eq!(status.status, "not_built");
        worker.join().expect("status worker");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn index_status_ignores_corrupt_non_authoritative_pointer() {
        let root = std::env::temp_dir().join(format!(
            "slicer-index-corrupt-pointer-{}",
            uuid::Uuid::new_v4()
        ));
        let service = WorkspaceService::new(root.join("config"));
        let api = ApiServerService::new(ApiAppState::new(Arc::new(service.clone())));
        let selected =
            service.select_workspace(root.join("workspace").to_string_lossy().into_owned(), &api);
        assert_eq!(selected.status, "ready");
        let layout = service.workspace_layout().expect("layout");
        fs::write(layout.bm25_active_pointer_path(), b"{not-json").expect("corrupt pointer");

        let status = SearchService::get_index_status(&service).expect("status");
        assert_eq!(status.status, "not_built");
        assert!(!status.can_search);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn persisted_index_directory_must_stay_inside_bm25_root() {
        let root = std::env::temp_dir().join(format!(
            "slicer-index-directory-validation-{}",
            uuid::Uuid::new_v4()
        ));
        let layout = WorkspaceLayout::from_root(root.clone());
        assert_eq!(
            SearchService::validated_index_path(&layout, "indexes/bm25/build-safe-version")
                .expect("valid path"),
            layout.bm25_index_dir().join("build-safe-version")
        );
        for invalid in [
            "../outside",
            "indexes/bm25/../outside",
            "indexes/other/build-version",
        ] {
            assert_eq!(
                SearchService::validated_index_path(&layout, invalid)
                    .expect_err("unsafe path")
                    .code,
                "index_directory_invalid"
            );
        }
        assert_eq!(
            SearchService::validated_index_path(&layout, &root.join("outside").to_string_lossy())
                .expect_err("absolute path")
                .code,
            "index_directory_invalid"
        );
    }

    #[test]
    fn analyzer_and_failure_age_are_compared_to_current_active_version() {
        let version = |version_id: &str,
                       analyzer_version: &str,
                       status: &str,
                       updated_at: &str,
                       build_finished_at: Option<&str>,
                       activated_at: Option<&str>| IndexVersionDto {
            version_id: version_id.to_string(),
            provider: IndexRepository::default_provider().to_string(),
            analyzer_version: analyzer_version.to_string(),
            content_schema_version: "pdf_modules_v2".to_string(),
            content_fingerprint: "fingerprint".to_string(),
            status: status.to_string(),
            index_directory: format!("indexes/bm25/build-{version_id}"),
            document_count: 1,
            build_started_at: None,
            build_finished_at: build_finished_at.map(ToString::to_string),
            activated_at: activated_at.map(ToString::to_string),
            error_id: None,
            created_at: "2026-07-30T00:00:00+00:00".to_string(),
            updated_at: updated_at.to_string(),
        };
        let active = version(
            "active",
            TANTIVY_ANALYZER_VERSION,
            "ready",
            "2026-07-30T02:00:00+00:00",
            Some("2026-07-30T02:00:00+00:00"),
            Some("2026-07-30T02:00:00+00:00"),
        );
        let old_analyzer = version(
            "old-analyzer",
            "cjk_bigram_v1",
            "ready",
            "2026-07-30T02:00:00+00:00",
            None,
            Some("2026-07-30T02:00:00+00:00"),
        );
        assert!(!SearchService::analyzer_version_is_stale(Some(&active)));
        assert!(SearchService::analyzer_version_is_stale(Some(
            &old_analyzer
        )));

        let historical_failure = version(
            "failed-before-active",
            TANTIVY_ANALYZER_VERSION,
            "failed",
            "2026-07-30T01:00:00+00:00",
            Some("2026-07-30T01:00:00+00:00"),
            None,
        );
        let recent_failure = version(
            "failed-after-active",
            TANTIVY_ANALYZER_VERSION,
            "failed",
            "2026-07-30T03:00:00+00:00",
            Some("2026-07-30T03:00:00+00:00"),
            None,
        );
        assert!(!SearchService::failed_version_is_newer_than_active(
            &historical_failure,
            &active,
        ));
        assert!(SearchService::failed_version_is_newer_than_active(
            &recent_failure,
            &active,
        ));
    }

    #[test]
    fn module_snapshot_omits_raw_artifacts_and_bounds_text() {
        let block = PdfContentBlockDto {
            block_id: "block-1".to_string(),
            parse_id: "parse-1".to_string(),
            document_id: "doc-1".to_string(),
            page_id: "page-1".to_string(),
            page_number: 1,
            parent_block_id: None,
            source_element_id: Some("element-1".to_string()),
            ordinal: 0,
            block_type: "figure".to_string(),
            source_text: "source ".repeat(MODULE_SNAPSHOT_TEXT_CHARS),
            enrichment_json: Some(
                serde_json::json!({
                    "description": "description ".repeat(MODULE_SNAPSHOT_ENRICHMENT_CHARS),
                    "visible_text": "visible text",
                    "keywords": ["keyword"]
                })
                .to_string(),
            ),
            raw_json: "raw-secret".repeat(10_000),
            source_image_path: Some("structure/doc/images/source.png".to_string()),
            is_indexable: true,
            is_visual: true,
            is_decorative: false,
            bbox: Some(NormalizedBbox {
                x: 0.1,
                y: 0.2,
                width: 0.3,
                height: 0.4,
            }),
        };

        let encoded =
            module_snapshot_json(&block, block.bbox, None, Some("summary")).expect("snapshot");
        let snapshot: serde_json::Value = serde_json::from_str(&encoded).expect("snapshot json");
        assert!(snapshot.get("raw_json").is_none());
        assert!(snapshot.get("source_image_path").is_none());
        assert!(
            snapshot["source_text"]
                .as_str()
                .expect("source text")
                .chars()
                .count()
                <= MODULE_SNAPSHOT_TEXT_CHARS
        );
        assert!(
            snapshot["enrichment"]["description"]
                .as_str()
                .expect("description")
                .chars()
                .count()
                <= MODULE_SNAPSHOT_ENRICHMENT_CHARS
        );
        assert!(!encoded.contains("raw-secret"));
        assert!(!encoded.contains("source.png"));
    }

    #[test]
    fn compact_snippet_normalizes_and_truncates_without_expanding_output() {
        assert_eq!(compact_snippet(" alpha\n beta ", "", 20), "alpha beta");
        let input = format!("alpha {} omega", "x".repeat(100_000));
        let snippet = compact_snippet(&input, "", 24);
        assert_eq!(snippet.chars().count(), 24);
        assert!(snippet.ends_with("..."));
    }

    #[test]
    fn index_content_fingerprint_is_order_independent_and_tracks_content_changes() {
        let document = |module_id: &str, text: &str| SearchIndexDocument {
            hit_id: module_hit_id(module_id),
            page_id: "page-1".to_string(),
            module_id: Some(module_id.to_string()),
            module_type: "paragraph".to_string(),
            snippet: text.to_string(),
            bbox: Some(NormalizedBbox {
                x: 0.1,
                y: 0.2,
                width: 0.3,
                height: 0.4,
            }),
            module_json: Some(format!(r#"{{"block_id":"{module_id}"}}"#)),
            document_id: "doc-1".to_string(),
            page_number: 1,
            image_path: "pages/doc-1/page.png".to_string(),
            original_filename: Some("document.pdf".to_string()),
            title: None,
            summary: None,
            visible_text: None,
            topics: Vec::new(),
            keywords: Vec::new(),
            bm25_text: text.to_string(),
        };
        let first = vec![document("block-a", "alpha"), document("block-b", "beta")];
        let reversed = vec![document("block-b", "beta"), document("block-a", "alpha")];
        let changed = vec![
            document("block-a", "alpha with enrichment"),
            document("block-b", "beta"),
        ];

        let first_hash = SearchService::index_content_fingerprint(&first).expect("fingerprint");
        assert_eq!(
            first_hash,
            SearchService::index_content_fingerprint(&reversed).expect("reversed fingerprint")
        );
        assert_ne!(
            first_hash,
            SearchService::index_content_fingerprint(&changed).expect("changed fingerprint")
        );
    }
}
