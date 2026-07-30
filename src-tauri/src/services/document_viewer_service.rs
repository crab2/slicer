use crate::artifacts::workspace_layout::{is_link_or_reparse_point, WorkspaceLayout};
use crate::domain::document_viewer::{
    DocumentViewerAssetDto, DocumentViewerContentDto, DocumentViewerFormatAvailabilityDto,
    DocumentViewerManifestDto, DOCUMENT_VIEWER_FORMATS,
};
use crate::errors::{AppError, AppResult};
use crate::repositories::document_viewer_repository::{
    DocumentViewerArtifactRecord, DocumentViewerRepository,
};
use crate::services::workspace_service::WorkspaceService;
use base64::{engine::general_purpose, Engine as _};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

const MAX_TEXT_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PDF_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PREVIEW_ASSET_BYTES: u64 = 16 * 1024 * 1024;
const MAX_PREVIEW_ASSETS_TOTAL_BYTES: u64 = 64 * 1024 * 1024;

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
        let (encoding, content) = if matches!(format, "pdf" | "annot") {
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
    use super::{hex_digest, DocumentViewerService};
    use crate::api::state::ApiAppState;
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
        (base, workspace, document_id)
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
