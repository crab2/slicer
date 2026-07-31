use crate::artifacts::workspace_layout::{is_link_or_reparse_point, WorkspaceLayout};
use crate::domain::document_viewer::{
    DocumentViewerAssetDto, DocumentViewerContentDto, DocumentViewerFormatAvailabilityDto,
    DocumentViewerManifestDto, DocumentViewerPagePreviewDto, DOCUMENT_VIEWER_FORMATS,
};
use crate::domain::pdf_structure::VisualModuleAnalysisV1;
use crate::errors::{AppError, AppResult};
use crate::providers::model::schema_validator::{
    validate_visual_module_analysis_v1, ExpectedVisualModuleContext,
};
use crate::providers::pdf_renderer::render_pdf_page_to_png_with_geometry;
use crate::providers::pdf_structure::opendataloader_block_id;
use crate::repositories::document_viewer_repository::{
    DocumentViewerArtifactRecord, DocumentViewerRepository, DocumentViewerVisualEnrichmentRecord,
};
use crate::services::workspace_service::WorkspaceService;
use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

const MAX_TEXT_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PDF_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PREVIEW_ASSET_BYTES: u64 = 16 * 1024 * 1024;
const MAX_PREVIEW_ASSETS_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
const SLICER_VISUAL_ANALYSIS_FIELD: &str = "slicer_visual_analysis";
static DOCUMENT_VIEWER_RENDER_QUEUE: Mutex<()> = Mutex::new(());
static DOCUMENT_VIEWER_REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static DOCUMENT_VIEWER_LATEST_REQUESTS: LazyLock<Mutex<HashMap<String, u64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub struct DocumentViewerService;

impl DocumentViewerService {
    pub fn get_manifest(
        workspace: &WorkspaceService,
        document_id: &str,
    ) -> AppResult<DocumentViewerManifestDto> {
        let layout = workspace.workspace_layout()?;
        layout.validate_storage_id(document_id, "document")?;
        let mut conn = workspace.get_db_connection()?;
        let document = DocumentViewerRepository::find_document(&mut conn, document_id)?
            .ok_or_else(document_not_found)?;
        let registered_artifacts =
            DocumentViewerRepository::list_artifacts(&mut conn, document_id)?;
        let formats = DOCUMENT_VIEWER_FORMATS
            .into_iter()
            .map(|format| DocumentViewerFormatAvailabilityDto {
                format: format.to_string(),
                available: registered_artifacts.iter().any(|artifact| {
                    artifact.kind == artifact_kind(format)
                        && registered_artifact_is_present(&layout, artifact)
                }),
            })
            .collect();

        Ok(DocumentViewerManifestDto {
            document_id: document.document_id,
            original_filename: document.original_filename,
            page_count: document.page_count,
            formats,
        })
    }

    pub fn get_content(
        workspace: &WorkspaceService,
        document_id: &str,
        format: &str,
    ) -> AppResult<DocumentViewerContentDto> {
        let layout = workspace.workspace_layout()?;
        layout.validate_storage_id(document_id, "document")?;
        let format = validate_format(format)?;
        let kind = artifact_kind(format);
        let mut conn = workspace.get_db_connection()?;
        if DocumentViewerRepository::find_document(&mut conn, document_id)?.is_none() {
            return Err(document_not_found());
        }
        let artifact = DocumentViewerRepository::find_artifact(&mut conn, document_id, kind)?
            .ok_or_else(|| format_unavailable(format))?;
        let max_bytes = if matches!(format, "pdf" | "annot") {
            MAX_PDF_ARTIFACT_BYTES
        } else {
            MAX_TEXT_ARTIFACT_BYTES
        };
        let (artifact_path, bytes) = read_registered_artifact(&layout, &artifact, max_bytes)?;
        let (encoding, mut content) = if matches!(format, "pdf" | "annot") {
            ("base64", general_purpose::STANDARD.encode(bytes))
        } else {
            let text = String::from_utf8(bytes).map_err(|err| {
                AppError::new(
                    "document_viewer_text_encoding_invalid",
                    "文档查看制品不是有效的 UTF-8 文本。",
                    "document_viewer",
                    false,
                )
                .with_details(err.to_string())
            })?;
            ("utf8", text)
        };
        if format == "json" {
            let enrichments = DocumentViewerRepository::list_visual_enrichments(
                &mut conn,
                document_id,
                &artifact.relative_path,
            )?;
            content = project_visual_analysis_json(document_id, &content, &enrichments);
        }
        let assets = if format == "preview" {
            Self::load_preview_assets(&layout, &mut conn, document_id, &artifact_path)?
        } else {
            Vec::new()
        };

        Ok(DocumentViewerContentDto {
            format: format.to_string(),
            mime_type: format_mime_type(format).to_string(),
            encoding: encoding.to_string(),
            content,
            assets,
        })
    }

    pub fn get_page_preview(
        workspace: &WorkspaceService,
        document_id: &str,
        format: &str,
        page_number: i64,
        request_key: &str,
    ) -> AppResult<DocumentViewerPagePreviewDto> {
        if format != "annot" {
            return Err(AppError::new(
                "document_viewer_page_preview_format_invalid",
                "仅 Annot 格式支持交互式按页查看。",
                "document_viewer",
                false,
            )
            .with_details(format.to_string()));
        }
        if page_number < 1 {
            return Err(page_out_of_range(page_number, None));
        }
        let request_key = validate_request_key(request_key)?;

        let layout = workspace.workspace_layout()?;
        layout.validate_storage_id(document_id, "document")?;
        let mut conn = workspace.get_db_connection()?;
        let document = DocumentViewerRepository::find_document(&mut conn, document_id)?
            .ok_or_else(document_not_found)?;
        if document
            .page_count
            .is_some_and(|page_count| page_number > page_count)
        {
            return Err(page_out_of_range(page_number, document.page_count));
        }
        let artifact =
            DocumentViewerRepository::find_artifact(&mut conn, document_id, artifact_kind(format))?
                .ok_or_else(|| format_unavailable(format))?;
        drop(conn);

        let request_id = register_latest_page_request(request_key)?;
        let result = (|| {
            let _queue_guard = DOCUMENT_VIEWER_RENDER_QUEUE.lock().map_err(|_| {
                AppError::new(
                    "document_viewer_page_render_queue_failed",
                    "Annot 页面渲染队列暂时不可用。",
                    "document_viewer",
                    true,
                )
            })?;
            if !page_request_is_latest(request_key, request_id)? {
                return Err(AppError::new(
                    "document_viewer_page_request_superseded",
                    "Annot 页面请求已被更新页码替代。",
                    "document_viewer",
                    true,
                ));
            }

            let (_, bytes) = read_registered_artifact(&layout, &artifact, MAX_PDF_ARTIFACT_BYTES)?;
            let rendered =
                render_pdf_page_to_png_with_geometry(&bytes, page_number).map_err(|error| {
                    let retryable = error.retryable;
                    AppError::new(
                        "document_viewer_page_render_failed",
                        "Annot 页面渲染失败。",
                        "document_viewer",
                        retryable,
                    )
                    .with_details(error.to_string())
                })?;

            Ok(DocumentViewerPagePreviewDto {
                format: format.to_string(),
                page_number,
                mime_type: "image/png".to_string(),
                data_url: format!(
                    "data:image/png;base64,{}",
                    general_purpose::STANDARD.encode(rendered.png_bytes)
                ),
                geometry: rendered.geometry,
            })
        })();
        finish_page_request(request_key, request_id);
        result
    }

    fn load_preview_assets(
        layout: &WorkspaceLayout,
        conn: &mut sqlx::SqliteConnection,
        document_id: &str,
        html_path: &Path,
    ) -> AppResult<Vec<DocumentViewerAssetDto>> {
        let html_parent = html_path.parent().ok_or_else(|| {
            AppError::new(
                "document_viewer_html_parent_missing",
                "Preview 制品路径无效。",
                "document_viewer",
                false,
            )
        })?;
        let mut total_bytes = 0_u64;
        let mut assets = Vec::new();
        for artifact in DocumentViewerRepository::list_artifacts(conn, document_id)?
            .into_iter()
            .filter(|artifact| artifact.kind == "pdf_structure_image")
        {
            let (path, bytes) =
                read_registered_artifact(layout, &artifact, MAX_PREVIEW_ASSET_BYTES)?;
            let relative = path.strip_prefix(html_parent).map_err(|_| {
                AppError::new(
                    "document_viewer_preview_asset_outside_output",
                    "Preview 图片不属于当前 HTML 制品目录。",
                    "document_viewer",
                    false,
                )
                .with_details(artifact.relative_path.clone())
            })?;
            if relative.as_os_str().is_empty()
                || relative
                    .components()
                    .any(|component| !matches!(component, Component::Normal(_)))
            {
                return Err(AppError::new(
                    "document_viewer_preview_asset_path_invalid",
                    "Preview 图片相对路径无效。",
                    "document_viewer",
                    false,
                ));
            }
            total_bytes = total_bytes
                .checked_add(bytes.len() as u64)
                .ok_or_else(|| preview_assets_too_large(MAX_PREVIEW_ASSETS_TOTAL_BYTES + 1))?;
            if total_bytes > MAX_PREVIEW_ASSETS_TOTAL_BYTES {
                return Err(preview_assets_too_large(total_bytes));
            }
            let mime_type = image_mime_type(&path);
            assets.push(DocumentViewerAssetDto {
                source: relative.to_string_lossy().replace('\\', "/"),
                mime_type: mime_type.to_string(),
                data_url: format!(
                    "data:{mime_type};base64,{}",
                    general_purpose::STANDARD.encode(bytes)
                ),
            });
        }
        Ok(assets)
    }
}

fn project_visual_analysis_json(
    document_id: &str,
    source: &str,
    enrichments: &[DocumentViewerVisualEnrichmentRecord],
) -> String {
    if enrichments.is_empty() {
        return source.to_string();
    }
    let Ok(mut root) = serde_json::from_str::<Value>(source) else {
        return source.to_string();
    };
    let mut by_block_id = HashMap::new();
    for record in enrichments {
        let Ok(untrusted) = serde_json::from_str::<VisualModuleAnalysisV1>(&record.enrichment_json)
        else {
            continue;
        };
        let expected = ExpectedVisualModuleContext {
            block_id: record.block_id.clone(),
            provider: untrusted.model.provider,
            model_name: untrusted.model.model_name,
        };
        let Ok(analysis) = validate_visual_module_analysis_v1(&record.enrichment_json, &expected)
        else {
            continue;
        };
        let Ok(value) = serde_json::to_value(analysis) else {
            continue;
        };
        by_block_id.insert(record.block_id.clone(), value);
    }
    if by_block_id.is_empty() {
        return source.to_string();
    }

    let Some(kids) = root.get_mut("kids").and_then(Value::as_array_mut) else {
        return source.to_string();
    };
    let mut projected = false;
    for (index, kid) in kids.iter_mut().enumerate() {
        projected |= inject_visual_analysis(document_id, kid, &index.to_string(), &by_block_id);
    }
    if !projected {
        return source.to_string();
    }
    let Ok(projected) = serde_json::to_string_pretty(&root) else {
        return source.to_string();
    };
    if projected.len() as u64 > MAX_TEXT_ARTIFACT_BYTES {
        return source.to_string();
    }
    projected
}

fn inject_visual_analysis(
    document_id: &str,
    node: &mut Value,
    path: &str,
    by_block_id: &HashMap<String, Value>,
) -> bool {
    let block_id = opendataloader_block_id(document_id, node, path);
    let analysis = by_block_id.get(&block_id).cloned();
    let Some(object) = node.as_object_mut() else {
        return false;
    };
    let mut projected = false;
    if let Some(analysis) = analysis {
        object.insert(SLICER_VISUAL_ANALYSIS_FIELD.to_string(), analysis);
        projected = true;
    }
    if let Some(children) = object.get_mut("kids").and_then(Value::as_array_mut) {
        for (index, child) in children.iter_mut().enumerate() {
            projected |=
                inject_visual_analysis(document_id, child, &format!("{path}.{index}"), by_block_id);
        }
    }
    projected
}

fn registered_artifact_is_present(
    layout: &WorkspaceLayout,
    artifact: &DocumentViewerArtifactRecord,
) -> bool {
    let relative = Path::new(&artifact.relative_path);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        // Invalid registrations remain selectable so the content command can reject them explicitly.
        return true;
    }
    match fs::symlink_metadata(layout.root().join(relative)) {
        Ok(_) => true,
        Err(error) => error.kind() != std::io::ErrorKind::NotFound,
    }
}

fn validate_format(format: &str) -> AppResult<&str> {
    DOCUMENT_VIEWER_FORMATS
        .into_iter()
        .find(|supported| *supported == format)
        .ok_or_else(|| {
            AppError::new(
                "document_viewer_format_invalid",
                "不支持的文档查看格式。",
                "document_viewer",
                false,
            )
            .with_details(format.to_string())
        })
}

fn artifact_kind(format: &str) -> &'static str {
    match format {
        "pdf" => "canonical_pdf",
        "annot" => "pdf_structure_annotated_pdf",
        "preview" | "html" => "pdf_structure_html",
        "md" => "pdf_structure_markdown",
        "json" => "pdf_structure_json",
        _ => "",
    }
}

fn format_mime_type(format: &str) -> &'static str {
    match format {
        "pdf" | "annot" => "application/pdf",
        "preview" | "html" => "text/html; charset=utf-8",
        "md" => "text/markdown; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn image_mime_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "image/png",
    }
}

fn read_registered_artifact(
    layout: &WorkspaceLayout,
    artifact: &DocumentViewerArtifactRecord,
    max_bytes: u64,
) -> AppResult<(PathBuf, Vec<u8>)> {
    let path = resolve_registered_artifact_path(layout, &artifact.relative_path)?;
    let mut file = File::open(&path).map_err(|err| {
        AppError::io(
            "document_viewer",
            "document_viewer_artifact_open_failed",
            err,
        )
        .with_details(artifact.relative_path.clone())
    })?;
    let metadata = file.metadata().map_err(|err| {
        AppError::io(
            "document_viewer",
            "document_viewer_artifact_metadata_failed",
            err,
        )
    })?;
    if is_link_or_reparse_point(&metadata) || !metadata.is_file() {
        return Err(AppError::new(
            "document_viewer_artifact_not_regular_file",
            "文档查看制品不是普通文件。",
            "document_viewer",
            false,
        ));
    }
    if metadata.len() > max_bytes {
        return Err(artifact_too_large(metadata.len(), max_bytes));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.by_ref()
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|err| {
            AppError::io(
                "document_viewer",
                "document_viewer_artifact_read_failed",
                err,
            )
        })?;
    if bytes.len() as u64 > max_bytes {
        return Err(artifact_too_large(bytes.len() as u64, max_bytes));
    }
    let actual_hash = hex_digest(&Sha256::digest(&bytes));
    if !actual_hash.eq_ignore_ascii_case(artifact.content_hash.trim()) {
        return Err(AppError::new(
            "document_viewer_artifact_hash_mismatch",
            "文档查看制品完整性校验失败。",
            "document_viewer",
            false,
        )
        .with_details(format!("kind={}", artifact.kind)));
    }
    Ok((path, bytes))
}

fn resolve_registered_artifact_path(
    layout: &WorkspaceLayout,
    relative_path: &str,
) -> AppResult<PathBuf> {
    let relative = Path::new(relative_path);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(AppError::new(
            "document_viewer_artifact_path_invalid",
            "文档查看制品登记路径无效。",
            "document_viewer",
            false,
        )
        .with_details(relative_path.to_string()));
    }
    let root = fs::canonicalize(layout.root())
        .map_err(|err| AppError::io("document_viewer", "document_viewer_workspace_invalid", err))?;
    let mut candidate = root.clone();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            unreachable!("relative path components were validated")
        };
        candidate.push(component);
        let metadata = fs::symlink_metadata(&candidate).map_err(|err| {
            AppError::io("document_viewer", "document_viewer_artifact_missing", err)
                .with_details(relative_path.to_string())
        })?;
        if is_link_or_reparse_point(&metadata) {
            return Err(AppError::new(
                "document_viewer_artifact_link_rejected",
                "文档查看制品路径不能包含符号链接或 junction。",
                "document_viewer",
                false,
            )
            .with_details(relative_path.to_string()));
        }
    }
    let resolved = fs::canonicalize(&candidate).map_err(|err| {
        AppError::io(
            "document_viewer",
            "document_viewer_artifact_path_unavailable",
            err,
        )
    })?;
    if !resolved.starts_with(&root) || !resolved.is_file() {
        return Err(AppError::new(
            "document_viewer_artifact_outside_workspace",
            "文档查看制品越过工作区边界。",
            "document_viewer",
            false,
        )
        .with_details(relative_path.to_string()));
    }
    Ok(resolved)
}

fn validate_request_key(value: &str) -> AppResult<&str> {
    let value = value.trim();
    if value.is_empty() || value.len() > 128 {
        return Err(AppError::new(
            "document_viewer_request_key_invalid",
            "Annot 页面请求标识无效。",
            "document_viewer",
            false,
        ));
    }
    Ok(value)
}

fn register_latest_page_request(request_key: &str) -> AppResult<u64> {
    let request_id = DOCUMENT_VIEWER_REQUEST_SEQUENCE
        .fetch_add(1, Ordering::Relaxed)
        .wrapping_add(1);
    DOCUMENT_VIEWER_LATEST_REQUESTS
        .lock()
        .map_err(|_| request_state_error())?
        .insert(request_key.to_string(), request_id);
    Ok(request_id)
}

fn page_request_is_latest(request_key: &str, request_id: u64) -> AppResult<bool> {
    Ok(DOCUMENT_VIEWER_LATEST_REQUESTS
        .lock()
        .map_err(|_| request_state_error())?
        .get(request_key)
        .is_some_and(|latest| *latest == request_id))
}

fn finish_page_request(request_key: &str, request_id: u64) {
    let Ok(mut requests) = DOCUMENT_VIEWER_LATEST_REQUESTS.lock() else {
        return;
    };
    if requests
        .get(request_key)
        .is_some_and(|latest| *latest == request_id)
    {
        requests.remove(request_key);
    }
}

fn request_state_error() -> AppError {
    AppError::new(
        "document_viewer_request_state_failed",
        "Annot 页面请求状态暂时不可用。",
        "document_viewer",
        true,
    )
}

fn document_not_found() -> AppError {
    AppError::new(
        "document_viewer_document_not_found",
        "未找到要查看的文档。",
        "document_viewer",
        false,
    )
}

fn format_unavailable(format: &str) -> AppError {
    AppError::new(
        "document_viewer_format_unavailable",
        "该文档没有登记此格式的查看制品。",
        "document_viewer",
        false,
    )
    .with_details(format.to_string())
}

fn page_out_of_range(page_number: i64, page_count: Option<i64>) -> AppError {
    AppError::new(
        "document_viewer_page_out_of_range",
        "Annot 页码超出文档范围。",
        "document_viewer",
        false,
    )
    .with_details(format!(
        "page_number={page_number}; page_count={}",
        page_count
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    ))
}

fn artifact_too_large(actual: u64, limit: u64) -> AppError {
    AppError::new(
        "document_viewer_artifact_too_large",
        "文档查看制品超过安全大小限制。",
        "document_viewer",
        false,
    )
    .with_details(format!("bytes={actual}; limit={limit}"))
}

fn preview_assets_too_large(actual: u64) -> AppError {
    AppError::new(
        "document_viewer_preview_assets_too_large",
        "Preview 图片总大小超过安全限制。",
        "document_viewer",
        false,
    )
    .with_details(format!(
        "bytes={actual}; limit={MAX_PREVIEW_ASSETS_TOTAL_BYTES}"
    ))
}

fn hex_digest(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{
        finish_page_request, hex_digest, page_request_is_latest, register_latest_page_request,
        DocumentViewerService,
    };
    use crate::api::state::ApiAppState;
    use crate::domain::pdf_structure::VISUAL_MODULE_ANALYSIS_SCHEMA_VERSION;
    use crate::providers::pdf_structure::opendataloader_block_id;
    use crate::repositories::db::block_on_db;
    use crate::services::api_server_service::ApiServerService;
    use crate::services::workspace_service::WorkspaceService;
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use uuid::Uuid;

    #[test]
    fn manifest_maps_registered_artifacts_to_six_formats() {
        let (base, workspace, document_id) = test_workspace();
        seed_artifact(
            &workspace,
            &document_id,
            "canonical_pdf",
            "pdfs/document.pdf",
            b"pdf",
        );
        seed_artifact(
            &workspace,
            &document_id,
            "pdf_structure_html",
            "structure/document.html",
            b"<html></html>",
        );
        seed_artifact(
            &workspace,
            &document_id,
            "pdf_structure_json",
            "structure/document.json",
            br#"{"kids":[]}"#,
        );

        let manifest =
            DocumentViewerService::get_manifest(&workspace, &document_id).expect("manifest");
        let availability: Vec<(&str, bool)> = manifest
            .formats
            .iter()
            .map(|item| (item.format.as_str(), item.available))
            .collect();
        assert_eq!(
            availability,
            vec![
                ("pdf", true),
                ("annot", false),
                ("preview", true),
                ("html", true),
                ("md", false),
                ("json", true),
            ]
        );

        let content = DocumentViewerService::get_content(&workspace, &document_id, "json")
            .expect("json content");
        assert_eq!(content.encoding, "utf8");
        assert_eq!(content.content, r#"{"kids":[]}"#);
        assert!(content.assets.is_empty());

        let missing = DocumentViewerService::get_content(&workspace, &document_id, "annot")
            .expect_err("historical artifact should stay unavailable");
        assert_eq!(missing.code, "document_viewer_format_unavailable");
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn json_content_projects_visual_enrichment_without_mutating_artifact() {
        let (base, workspace, document_id) = test_workspace();
        let raw_json = br#"{"number of pages":3,"kids":[{"type":"section","id":"shared","page number":2,"kids":[{"type":"image","id":"shared","page number":2,"source":"images/imageFile3.png","alt_source":"missing","slicer_visual_analysis":{"description":"untrusted source value"}}]},{"type":"image","id":"bad","page number":2,"source":"images/bad.png"},{"type":"image","id":"blob","page number":2,"source":"images/blob.png"}]}"#;
        let raw_json_path = "structure/document.json";
        seed_artifact(
            &workspace,
            &document_id,
            "pdf_structure_json",
            raw_json_path,
            raw_json,
        );
        let source: serde_json::Value =
            serde_json::from_slice(raw_json).expect("source JSON should parse");
        let nested_block_id =
            opendataloader_block_id(&document_id, &source["kids"][0]["kids"][0], "0.0");
        let bad_block_id = opendataloader_block_id(&document_id, &source["kids"][1], "1");
        let blob_block_id = opendataloader_block_id(&document_id, &source["kids"][2], "2");
        let (parse_id, page_id) = seed_structure_run(&workspace, &document_id, raw_json_path);
        seed_visual_enrichment(
            &workspace,
            &document_id,
            &parse_id,
            &page_id,
            &nested_block_id,
            0,
            &valid_visual_enrichment(&nested_block_id, "A model-generated description"),
        );
        seed_visual_enrichment(
            &workspace,
            &document_id,
            &parse_id,
            &page_id,
            &bad_block_id,
            1,
            "not-json",
        );
        seed_visual_enrichment_blob(
            &workspace,
            &document_id,
            &parse_id,
            &page_id,
            &blob_block_id,
            2,
        );

        let other_document_id = Uuid::new_v4().to_string();
        seed_document(&workspace, &other_document_id);
        let (other_parse_id, other_page_id) =
            seed_structure_run(&workspace, &other_document_id, raw_json_path);
        let other_block_id =
            opendataloader_block_id(&other_document_id, &source["kids"][0]["kids"][0], "0.0");
        seed_visual_enrichment(
            &workspace,
            &other_document_id,
            &other_parse_id,
            &other_page_id,
            &other_block_id,
            0,
            &valid_visual_enrichment(&other_block_id, "Other document description"),
        );

        let artifact_path = workspace
            .workspace_layout()
            .expect("layout")
            .root()
            .join(raw_json_path);
        let original_bytes = fs::read(&artifact_path).expect("original artifact");
        let content = DocumentViewerService::get_content(&workspace, &document_id, "json")
            .expect("projected JSON content");
        let projected: serde_json::Value =
            serde_json::from_str(&content.content).expect("projected JSON should parse");
        let analysis = &projected["kids"][0]["kids"][0]["slicer_visual_analysis"];
        assert_eq!(analysis["block_id"], nested_block_id);
        assert_eq!(analysis["description"], "A model-generated description");
        assert_eq!(analysis["visible_text"], "Visible model text");
        assert_eq!(analysis["keywords"], serde_json::json!(["model", "image"]));
        assert_eq!(analysis["model"]["provider"], "local_mock");
        assert!(projected["kids"][0].get("slicer_visual_analysis").is_none());
        assert!(projected["kids"][1].get("slicer_visual_analysis").is_none());
        assert!(projected["kids"][2].get("slicer_visual_analysis").is_none());
        assert_eq!(
            fs::read(&artifact_path).expect("artifact after projection"),
            original_bytes
        );
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn preview_only_returns_registered_assets_relative_to_html() {
        let (base, workspace, document_id) = test_workspace();
        seed_artifact(
            &workspace,
            &document_id,
            "pdf_structure_html",
            "structure/run/document.html",
            br#"<html><img src="images/page.png"></html>"#,
        );
        seed_artifact(
            &workspace,
            &document_id,
            "pdf_structure_image",
            "structure/run/images/page.png",
            b"png-bytes",
        );

        let content = DocumentViewerService::get_content(&workspace, &document_id, "preview")
            .expect("preview content");
        assert_eq!(content.assets.len(), 1);
        assert_eq!(content.assets[0].source, "images/page.png");
        assert!(content.assets[0]
            .data_url
            .starts_with("data:image/png;base64,"));
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn interactive_page_preview_only_accepts_annot_and_valid_page_numbers() {
        let (base, workspace, document_id) = test_workspace();
        let invalid_format =
            DocumentViewerService::get_page_preview(&workspace, &document_id, "pdf", 1, "test")
                .expect_err("only annot supports interactive page previews");
        assert_eq!(
            invalid_format.code,
            "document_viewer_page_preview_format_invalid"
        );

        let out_of_range =
            DocumentViewerService::get_page_preview(&workspace, &document_id, "annot", 4, "test")
                .expect_err("page count is checked before rendering");
        assert_eq!(out_of_range.code, "document_viewer_page_out_of_range");
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn newer_page_request_supersedes_queued_work_for_the_same_pane() {
        let request_key = format!("test-pane-{}", Uuid::new_v4());
        let first = register_latest_page_request(&request_key).expect("first request");
        let second = register_latest_page_request(&request_key).expect("second request");
        assert!(!page_request_is_latest(&request_key, first).expect("first status"));
        assert!(page_request_is_latest(&request_key, second).expect("second status"));

        finish_page_request(&request_key, first);
        assert!(page_request_is_latest(&request_key, second).expect("second remains latest"));
        finish_page_request(&request_key, second);
        assert!(!page_request_is_latest(&request_key, second).expect("request removed"));
    }

    #[test]
    fn renders_registered_annot_page_with_geometry() {
        let (base, workspace, document_id) = test_workspace();
        seed_artifact(
            &workspace,
            &document_id,
            "pdf_structure_annotated_pdf",
            "structure/document_annotated.pdf",
            include_bytes!("../../../tmp/pdfs/structured-retrieval-fixture.pdf"),
        );

        let preview =
            DocumentViewerService::get_page_preview(&workspace, &document_id, "annot", 2, "test")
                .expect("render registered Annot page");
        assert_eq!(preview.format, "annot");
        assert_eq!(preview.page_number, 2);
        assert!(preview.data_url.starts_with("data:image/png;base64,"));
        assert!(preview.geometry.is_valid());
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn registered_parent_traversal_is_rejected() {
        let (base, workspace, document_id) = test_workspace();
        insert_artifact_record(
            &workspace,
            &document_id,
            "pdf_structure_json",
            "../outside.json",
            "unused",
        );

        let error = DocumentViewerService::get_content(&workspace, &document_id, "json")
            .expect_err("traversal must be rejected");
        assert_eq!(error.code, "document_viewer_artifact_path_invalid");
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn tampered_registered_artifact_is_rejected() {
        let (base, workspace, document_id) = test_workspace();
        seed_artifact(
            &workspace,
            &document_id,
            "pdf_structure_json",
            "structure/document.json",
            br#"{"before":true}"#,
        );
        let layout = workspace.workspace_layout().expect("layout");
        fs::write(
            layout.root().join("structure/document.json"),
            br#"{"after":true}"#,
        )
        .expect("tamper");

        let error = DocumentViewerService::get_content(&workspace, &document_id, "json")
            .expect_err("tampering must be rejected");
        assert_eq!(error.code, "document_viewer_artifact_hash_mismatch");
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn manifest_marks_a_registered_but_missing_artifact_unavailable() {
        let (base, workspace, document_id) = test_workspace();
        insert_artifact_record(
            &workspace,
            &document_id,
            "pdf_structure_markdown",
            "structure/missing.md",
            "unused",
        );

        let manifest =
            DocumentViewerService::get_manifest(&workspace, &document_id).expect("manifest");
        let markdown = manifest
            .formats
            .iter()
            .find(|item| item.format == "md")
            .expect("markdown format");
        assert!(!markdown.available);
        let _ = fs::remove_dir_all(base);
    }

    fn test_workspace() -> (PathBuf, WorkspaceService, String) {
        let base = std::env::temp_dir().join(format!(
            "slicer-document-viewer-{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ));
        let config = base.join("config");
        let root = base.join("workspace");
        fs::create_dir_all(&base).expect("base");
        let workspace = WorkspaceService::new(config);
        let api_state = ApiAppState::new(Arc::new(workspace.clone()));
        let api = ApiServerService::new(api_state);
        let selected = workspace.select_workspace(root.to_string_lossy().into_owned(), &api);
        assert_eq!(selected.status, "ready");
        let document_id = Uuid::new_v4().to_string();
        seed_document(&workspace, &document_id);
        (base, workspace, document_id)
    }

    fn seed_document(workspace: &WorkspaceService, document_id: &str) {
        let mut conn = workspace.get_db_connection().expect("db");
        block_on_db(async {
            let now = chrono::Utc::now().to_rfc3339();
            sqlx::query(
                "INSERT INTO documents
                 (document_id, original_filename, file_type, file_hash, original_path,
                  page_count, status, created_at, updated_at)
                 VALUES (?1, 'document.pdf', 'pdf', ?1, 'originals/document.pdf',
                         3, 'ready', ?2, ?2)",
            )
            .bind(&document_id)
            .bind(now)
            .execute(&mut conn)
            .await
            .expect("document");
            Ok(())
        })
        .expect("seed document");
    }

    fn seed_structure_run(
        workspace: &WorkspaceService,
        document_id: &str,
        raw_json_path: &str,
    ) -> (String, String) {
        let parse_id = Uuid::new_v4().to_string();
        let page_id = Uuid::new_v4().to_string();
        let mut conn = workspace.get_db_connection().expect("db");
        block_on_db(async {
            let now = chrono::Utc::now().to_rfc3339();
            sqlx::query(
                "INSERT INTO page_records
                 (page_id, document_id, page_number, status, created_at, updated_at)
                 VALUES (?1, ?2, 2, 'structured', ?3, ?3)",
            )
            .bind(&page_id)
            .bind(document_id)
            .bind(&now)
            .execute(&mut conn)
            .await
            .expect("page record");
            sqlx::query(
                "INSERT INTO pdf_parse_runs
                 (parse_id, document_id, parser_name, parser_version, schema_version,
                  parser_options_json, status, raw_json_path, created_at, updated_at)
                 VALUES (?1, ?2, 'opendataloader-pdf', 'test',
                         'opendataloader_pdf_json_v2', '{}', 'succeeded', ?3, ?4, ?4)",
            )
            .bind(&parse_id)
            .bind(document_id)
            .bind(raw_json_path)
            .bind(now)
            .execute(&mut conn)
            .await
            .expect("parse run");
            Ok(())
        })
        .expect("seed structure run");
        (parse_id, page_id)
    }

    #[allow(clippy::too_many_arguments)]
    fn seed_visual_enrichment(
        workspace: &WorkspaceService,
        document_id: &str,
        parse_id: &str,
        page_id: &str,
        block_id: &str,
        ordinal: i64,
        enrichment_json: &str,
    ) {
        let mut conn = workspace.get_db_connection().expect("db");
        block_on_db(async {
            let now = chrono::Utc::now().to_rfc3339();
            sqlx::query(
                "INSERT INTO content_blocks
                 (block_id, parse_id, document_id, page_id, page_number, ordinal,
                  block_type, source_text, enrichment_json, raw_json, is_indexable,
                  is_visual, is_decorative, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 2, ?5, 'image', '', ?6, '{}', 1, 1, 0, ?7, ?7)",
            )
            .bind(block_id)
            .bind(parse_id)
            .bind(document_id)
            .bind(page_id)
            .bind(ordinal)
            .bind(enrichment_json)
            .bind(now)
            .execute(&mut conn)
            .await
            .expect("visual block");
            Ok(())
        })
        .expect("seed visual enrichment");
    }

    fn seed_visual_enrichment_blob(
        workspace: &WorkspaceService,
        document_id: &str,
        parse_id: &str,
        page_id: &str,
        block_id: &str,
        ordinal: i64,
    ) {
        let mut conn = workspace.get_db_connection().expect("db");
        block_on_db(async {
            let now = chrono::Utc::now().to_rfc3339();
            sqlx::query(
                "INSERT INTO content_blocks
                 (block_id, parse_id, document_id, page_id, page_number, ordinal,
                  block_type, source_text, enrichment_json, raw_json, is_indexable,
                  is_visual, is_decorative, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 2, ?5, 'image', '', ?6, '{}', 1, 1, 0, ?7, ?7)",
            )
            .bind(block_id)
            .bind(parse_id)
            .bind(document_id)
            .bind(page_id)
            .bind(ordinal)
            .bind(vec![0xff_u8, 0xfe_u8])
            .bind(now)
            .execute(&mut conn)
            .await
            .expect("visual block with blob enrichment");
            Ok(())
        })
        .expect("seed blob visual enrichment");
    }

    fn valid_visual_enrichment(block_id: &str, description: &str) -> String {
        serde_json::json!({
            "schema_version": VISUAL_MODULE_ANALYSIS_SCHEMA_VERSION,
            "block_id": block_id,
            "description": description,
            "visible_text": "Visible model text",
            "keywords": ["model", "image"],
            "model": {
                "provider": "local_mock",
                "model_name": "mock-model"
            }
        })
        .to_string()
    }

    fn seed_artifact(
        workspace: &WorkspaceService,
        document_id: &str,
        kind: &str,
        relative_path: &str,
        bytes: &[u8],
    ) {
        let layout = workspace.workspace_layout().expect("layout");
        let path = layout.root().join(relative_path);
        fs::create_dir_all(path.parent().unwrap_or(Path::new("."))).expect("artifact parent");
        fs::write(&path, bytes).expect("artifact");
        insert_artifact_record(
            workspace,
            document_id,
            kind,
            relative_path,
            &hex_digest(&Sha256::digest(bytes)),
        );
    }

    fn insert_artifact_record(
        workspace: &WorkspaceService,
        document_id: &str,
        kind: &str,
        relative_path: &str,
        content_hash: &str,
    ) {
        let mut conn = workspace.get_db_connection().expect("db");
        block_on_db(async {
            let now = chrono::Utc::now().to_rfc3339();
            sqlx::query(
                "INSERT INTO document_artifacts
                 (artifact_id, document_id, kind, relative_path, content_hash,
                  created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(document_id)
            .bind(kind)
            .bind(relative_path)
            .bind(content_hash)
            .bind(now)
            .execute(&mut conn)
            .await
            .expect("artifact record");
            Ok(())
        })
        .expect("seed artifact");
    }
}
