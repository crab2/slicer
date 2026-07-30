use crate::artifacts::jsonl_exporter::ArtifactExporter;
use crate::artifacts::workspace_layout::{is_link_or_reparse_point, WorkspaceLayout};
use crate::domain::document::DocumentDto;
use crate::domain::pdf_structure::{
    DocumentArtifactInput, PdfParseRun, PdfStructurePage, PDF_STRUCTURE_OPTIONS_JSON,
    PDF_STRUCTURE_PARSER_NAME, PDF_STRUCTURE_PARSER_VERSION, PDF_STRUCTURE_SCHEMA_VERSION,
};
use crate::errors::{AppError, AppResult};
use crate::jobs::job_orchestrator::JobOrchestrator;
use crate::providers::converter::{detect_file_type, is_office_extension, DocumentConverter};
use crate::providers::pdf_renderer::{
    compute_file_hash, compute_image_hash, sanitize_filename, PdfPageMetadata, PdfRenderer,
};
use crate::providers::pdf_structure::OpenDataLoaderPdfProvider;
use crate::repositories::document_repository::DocumentRepository;
use crate::repositories::pdf_structure_repository::PdfStructureRepository;
use crate::services::workspace_service::WorkspaceService;
use image::ImageFormat;
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{Cursor, ErrorKind, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct ImportService;

struct ImportSourceSnapshot {
    directory: PathBuf,
    path: PathBuf,
}

impl ImportSourceSnapshot {
    fn create(layout: &WorkspaceLayout, source: &Path, original_filename: &str) -> AppResult<Self> {
        let directory_id = Uuid::new_v4().to_string();
        let directory = layout.ensure_managed_document_dir(&layout.tmp_dir(), &directory_id)?;
        let mut filename = sanitize_filename(original_filename);
        if filename.is_empty() {
            filename = "source".to_string();
        }
        let path = directory.join(filename);
        if let Err(error) = ImportService::copy_original_file(source, &path) {
            let _ = fs::remove_dir_all(&directory);
            return Err(AppError::io(
                "import",
                "import_source_snapshot_failed",
                error,
            ));
        }
        Ok(Self { directory, path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ImportSourceSnapshot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

struct ArtifactDirectoryCleanup {
    path: Option<PathBuf>,
}

impl ArtifactDirectoryCleanup {
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    fn retarget(&mut self, path: PathBuf) {
        self.path = Some(path);
    }

    fn disarm(&mut self) {
        self.path = None;
    }
}

impl Drop for ArtifactDirectoryCleanup {
    fn drop(&mut self) {
        if let Some(path) = self.path.as_deref() {
            let _ = fs::remove_dir_all(path);
        }
    }
}

struct ImportStatusGuard {
    workspace: WorkspaceService,
    document_id: String,
    job_id: String,
    completed: bool,
}

impl ImportStatusGuard {
    fn new(workspace: &WorkspaceService, document_id: &str, job_id: &str) -> Self {
        Self {
            workspace: workspace.clone(),
            document_id: document_id.to_string(),
            job_id: job_id.to_string(),
            completed: false,
        }
    }

    fn complete(&mut self) {
        self.completed = true;
    }
}

impl Drop for ImportStatusGuard {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        let error = AppError::new(
            "import_aborted",
            "导入流程意外中止，文档已保留并可重试。",
            "import",
            true,
        );
        if let Ok(layout) = self.workspace.workspace_layout() {
            let orchestrator = JobOrchestrator::new(layout);
            let _ = orchestrator.fail_import_if_active(
                &self.document_id,
                &self.job_id,
                &error,
                &error.message,
            );
        }
    }
}

impl ImportService {
    pub fn is_image_extension(ext: &str) -> bool {
        matches!(ext.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg")
    }

    pub fn import_image(
        workspace: &WorkspaceService,
        image_path: &PathBuf,
    ) -> AppResult<DocumentDto> {
        let status = workspace.get_workspace_status();
        if status.status != "ready" {
            return Err(AppError::new(
                "workspace_not_ready",
                "工作区未就绪，请先选择工作区。",
                "import",
                true,
            ));
        }

        if !image_path.exists() {
            return Err(AppError::new(
                "file_not_found",
                "找不到指定的图片文件。",
                "import",
                false,
            ));
        }

        if !image_path.is_file() {
            return Err(AppError::new(
                "file_not_found",
                "选择的图片路径不是文件。",
                "import",
                false,
            ));
        }

        let ext = image_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !Self::is_image_extension(&ext) {
            return Err(AppError::new(
                "unsupported_file_type",
                format!("不支持的图片类型: .{ext}，当前支持 PNG、JPG、JPEG。"),
                "import",
                false,
            ));
        }

        let layout = workspace.workspace_layout()?;
        let originals_dir = layout.managed_parent(&layout.originals_dir())?;
        let pages_dir = layout.managed_parent(&layout.pages_dir())?;

        let original_name = image_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown-image")
            .to_string();

        let source_snapshot = ImportSourceSnapshot::create(&layout, image_path, &original_name)?;
        let source_path = source_snapshot.path();

        let file_hash = compute_file_hash(source_path)?;
        let sanitized = sanitize_filename(&original_name);

        let mut conn = workspace.get_db_connection()?;

        if let Some(existing) = DocumentRepository::find_document_by_hash(&mut conn, &file_hash)? {
            return Ok(existing);
        }

        let orchestrator = JobOrchestrator::new(layout.clone());
        let job = orchestrator.create_job("image_import")?;
        let job_id = &job.job_id;

        let dest_filename = format!("{}_{}", &file_hash[..16], sanitized);
        let dest_path = originals_dir.join(&dest_filename);
        let document = DocumentRepository::create_document(
            &mut conn,
            &original_name,
            &ext,
            &file_hash,
            &dest_path.to_string_lossy(),
            Some(job_id),
        )?;
        let mut import_guard = ImportStatusGuard::new(workspace, &document.document_id, job_id);

        Self::copy_original_file(source_path, &dest_path).map_err(|e| {
            Self::fail_with_cleanup(
                workspace,
                &document.document_id,
                job_id,
                "file_copy_failed",
                "无法复制原图片到工作区。",
                &e.to_string(),
            )
        })?;

        orchestrator.update_progress(job_id, 40, Some("正在处理图片"))?;

        let png_bytes = Self::decode_image_to_png(source_path).map_err(|e| {
            Self::fail_with_cleanup(
                workspace,
                &document.document_id,
                job_id,
                &e.code,
                &e.message,
                e.details.as_deref().unwrap_or(""),
            )
        })?;

        let image_hash = compute_image_hash(&png_bytes);
        let doc_pages_dir = layout
            .ensure_managed_document_dir(&pages_dir, &document.document_id)
            .map_err(|e| {
                Self::fail_with_cleanup(
                    workspace,
                    &document.document_id,
                    job_id,
                    "pages_dir_create_failed",
                    "无法创建页面目录。",
                    &e.to_string(),
                )
            })?;

        let mut new_files = HashSet::new();
        let existing = DocumentRepository::find_image_asset_by_hash(&mut conn, &image_hash)?;
        if existing.is_some() {
            DocumentRepository::create_page_record(
                &mut conn,
                &document.document_id,
                1,
                &image_hash,
            )?;
        } else {
            let png_filename = format!("{image_hash}.png");
            let final_path = doc_pages_dir.join(&png_filename);

            Self::write_file_atomically_new(&final_path, &png_bytes).map_err(|e| {
                Self::fail_with_cleanup_safe(
                    workspace,
                    &document.document_id,
                    job_id,
                    "page_write_failed",
                    "页面图片写入失败。",
                    &e.to_string(),
                    &new_files,
                )
            })?;

            new_files.insert(final_path);

            let file_size = png_bytes.len() as i64;
            let rel_path = format!("pages/{}/{}.png", document.document_id, image_hash);
            DocumentRepository::create_image_asset(&mut conn, &image_hash, &rel_path, file_size)?;
            DocumentRepository::create_page_record(
                &mut conn,
                &document.document_id,
                1,
                &image_hash,
            )?;
        }

        orchestrator.complete_import(&mut conn, &document.document_id, job_id, 1, "导入完成")?;
        import_guard.complete();

        if let Err(e) = ArtifactExporter::export_all(workspace) {
            eprintln!("[WARN] JSONL 导出失败，不影响导入结果: {}", e);
        }

        let updated = DocumentRepository::list_documents(&mut conn)?
            .into_iter()
            .find(|d| d.document_id == document.document_id)
            .unwrap_or(document);

        Ok(updated)
    }

    pub fn import_pdf(
        workspace: &WorkspaceService,
        pdf_path: &PathBuf,
        renderer: &dyn PdfRenderer,
    ) -> AppResult<DocumentDto> {
        let status = workspace.get_workspace_status();
        if status.status != "ready" {
            return Err(AppError::new(
                "workspace_not_ready",
                "工作区未就绪，请先选择工作区。",
                "import",
                true,
            ));
        }

        if !pdf_path.exists() {
            return Err(AppError::new(
                "file_not_found",
                "找不到指定的 PDF 文件。",
                "import",
                false,
            ));
        }

        let layout = workspace.workspace_layout()?;
        let originals_dir = layout.managed_parent(&layout.originals_dir())?;

        let original_name = pdf_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown.pdf")
            .to_string();

        let source_snapshot = ImportSourceSnapshot::create(&layout, pdf_path, &original_name)?;
        let source_path = source_snapshot.path();

        let file_hash = compute_file_hash(source_path)?;
        let sanitized = sanitize_filename(&original_name);

        let mut conn = workspace.get_db_connection()?;

        if let Some(existing) = DocumentRepository::find_document_by_hash(&mut conn, &file_hash)? {
            return Ok(existing);
        }

        let orchestrator = JobOrchestrator::new(layout.clone());
        let job = orchestrator.create_job("pdf_import")?;
        let job_id = &job.job_id;

        let dest_filename = format!("{}_{}", &file_hash[..16], sanitized);
        let dest_path = originals_dir.join(&dest_filename);
        let document = DocumentRepository::create_document(
            &mut conn,
            &original_name,
            "pdf",
            &file_hash,
            &dest_path.to_string_lossy(),
            Some(job_id),
        )?;
        let mut import_guard = ImportStatusGuard::new(workspace, &document.document_id, job_id);

        Self::copy_original_file(source_path, &dest_path).map_err(|e| {
            Self::fail_with_cleanup(
                workspace,
                &document.document_id,
                job_id,
                "file_copy_failed",
                "无法复制原文件到工作区。",
                &e.to_string(),
            )
        })?;

        let canonical_pdf =
            Self::persist_canonical_pdf(&layout, &document.document_id, source_path).map_err(
                |error| {
                    Self::fail_with_cleanup(
                        workspace,
                        &document.document_id,
                        job_id,
                        &error.code,
                        &error.message,
                        error.details.as_deref().unwrap_or_default(),
                    )
                },
            )?;
        Self::record_canonical_pdf(&mut conn, &layout, &document.document_id, &canonical_pdf)
            .map_err(|error| {
                Self::fail_with_cleanup(
                    workspace,
                    &document.document_id,
                    job_id,
                    &error.code,
                    &error.message,
                    error.details.as_deref().unwrap_or_default(),
                )
            })?;

        orchestrator.update_progress(job_id, 10, Some("正在读取 PDF 页面元数据"))?;

        let pages = renderer.inspect_pdf(&canonical_pdf).map_err(|e| {
            Self::fail_with_cleanup(
                workspace,
                &document.document_id,
                job_id,
                &e.code,
                &e.message,
                e.details.as_deref().unwrap_or_default(),
            )
        })?;

        let page_count = pages.len() as i64;
        for (i, page) in pages.iter().enumerate() {
            let progress = (10 + ((i + 1) * 20 / pages.len())).min(30) as u8;
            orchestrator.update_progress(
                job_id,
                progress,
                Some(&format!("正在登记第 {} 页结构", page.page_number)),
            )?;
            let page_record = DocumentRepository::create_structured_page_record(
                &mut conn,
                &document.document_id,
                page.page_number,
            )?;
            DocumentRepository::update_page_pdf_geometry(
                &mut conn,
                &page_record.page_id,
                page.geometry,
            )?;
        }

        orchestrator.update_progress(job_id, 35, Some("正在提取 PDF 结构"))?;
        Self::extract_and_persist_pdf_structure(
            workspace,
            &mut conn,
            &document.document_id,
            &canonical_pdf,
            &pages,
        )
        .map_err(|error| {
            Self::fail_preserving_pdf_artifacts(workspace, &document.document_id, job_id, error)
        })?;

        orchestrator.complete_import(
            &mut conn,
            &document.document_id,
            job_id,
            page_count,
            "导入完成",
        )?;
        import_guard.complete();

        if let Err(e) = ArtifactExporter::export_all(workspace) {
            eprintln!("[WARN] JSONL 导出失败，不影响导入结果: {}", e);
        }

        let updated = DocumentRepository::list_documents(&mut conn)?
            .into_iter()
            .find(|d| d.document_id == document.document_id)
            .unwrap_or(document);

        Ok(updated)
    }

    pub fn import_document(
        workspace: &WorkspaceService,
        file_path: &PathBuf,
        renderer: &dyn PdfRenderer,
        converter: &dyn DocumentConverter,
    ) -> AppResult<DocumentDto> {
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let file_type = detect_file_type(file_path);

        if ext == "pdf" {
            return Self::import_pdf(workspace, file_path, renderer);
        }

        if Self::is_image_extension(&ext) {
            return Self::import_image(workspace, file_path);
        }

        if !is_office_extension(&ext) {
            return Err(AppError::new(
                "unsupported_file_type",
                format!("不支持的文件类型: .{ext}，当前支持 PDF、DOC、DOCX、PPT、PPTX、PNG、JPG、JPEG。"),
                "import",
                false,
            ));
        }

        let status = workspace.get_workspace_status();
        if status.status != "ready" {
            return Err(AppError::new(
                "workspace_not_ready",
                "工作区未就绪，请先选择工作区。",
                "import",
                true,
            ));
        }

        if !file_path.exists() {
            return Err(AppError::new(
                "file_not_found",
                "找不到指定的文件。",
                "import",
                false,
            ));
        }

        let layout = workspace.workspace_layout()?;
        let originals_dir = layout.managed_parent(&layout.originals_dir())?;

        let original_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let source_snapshot = ImportSourceSnapshot::create(&layout, file_path, &original_name)?;
        let source_path = source_snapshot.path();

        let file_hash = compute_file_hash(source_path)?;
        let sanitized = sanitize_filename(&original_name);

        let mut conn = workspace.get_db_connection()?;

        if let Some(existing) = DocumentRepository::find_document_by_hash(&mut conn, &file_hash)? {
            return Ok(existing);
        }

        let orchestrator = JobOrchestrator::new(layout.clone());
        let job = orchestrator.create_job("document_import")?;
        let job_id = &job.job_id;

        let dest_filename = format!("{}_{}", &file_hash[..16], sanitized);
        let dest_path = originals_dir.join(&dest_filename);
        let document = DocumentRepository::create_document(
            &mut conn,
            &original_name,
            file_type,
            &file_hash,
            &dest_path.to_string_lossy(),
            Some(job_id),
        )?;
        let mut import_guard = ImportStatusGuard::new(workspace, &document.document_id, job_id);

        Self::copy_original_file(source_path, &dest_path).map_err(|e| {
            Self::fail_with_cleanup(
                workspace,
                &document.document_id,
                job_id,
                "file_copy_failed",
                "无法复制原文件到工作区。",
                &e.to_string(),
            )
        })?;

        orchestrator.update_progress(job_id, 10, Some("正在转换为 PDF"))?;

        let conversion_dir_id = format!("office-conversion-{job_id}");
        let conversion_dir =
            layout.ensure_managed_document_dir(&layout.tmp_dir(), &conversion_dir_id)?;
        let _conversion_cleanup = ArtifactDirectoryCleanup::new(conversion_dir.clone());

        let converted_pdf = converter
            .convert_to_pdf(source_path, &conversion_dir)
            .map_err(|e| {
                Self::fail_with_cleanup(
                    workspace,
                    &document.document_id,
                    job_id,
                    "conversion_failed",
                    "Office 文档转换为 PDF 失败。",
                    &e.to_string(),
                )
            })?;

        let canonical_pdf =
            Self::persist_canonical_pdf(&layout, &document.document_id, &converted_pdf).map_err(
                |error| {
                    let _ = fs::remove_file(&converted_pdf);
                    Self::fail_with_cleanup(
                        workspace,
                        &document.document_id,
                        job_id,
                        &error.code,
                        &error.message,
                        error.details.as_deref().unwrap_or_default(),
                    )
                },
            )?;
        Self::record_canonical_pdf(&mut conn, &layout, &document.document_id, &canonical_pdf)
            .map_err(|error| {
                let _ = fs::remove_file(&converted_pdf);
                Self::fail_with_cleanup(
                    workspace,
                    &document.document_id,
                    job_id,
                    &error.code,
                    &error.message,
                    error.details.as_deref().unwrap_or_default(),
                )
            })?;

        orchestrator.update_progress(job_id, 30, Some("正在读取 PDF 页面元数据"))?;

        let pages = renderer.inspect_pdf(&canonical_pdf).map_err(|e| {
            let _ = fs::remove_file(&converted_pdf);
            Self::fail_with_cleanup(
                workspace,
                &document.document_id,
                job_id,
                &e.code,
                &e.message,
                e.details.as_deref().unwrap_or_default(),
            )
        })?;

        let page_count = pages.len() as i64;
        for (i, page) in pages.iter().enumerate() {
            let progress = (30 + ((i + 1) * 20 / pages.len())).min(50) as u8;
            orchestrator.update_progress(
                job_id,
                progress,
                Some(&format!("正在登记第 {} 页结构", page.page_number)),
            )?;
            let page_record = DocumentRepository::create_structured_page_record(
                &mut conn,
                &document.document_id,
                page.page_number,
            )?;
            DocumentRepository::update_page_pdf_geometry(
                &mut conn,
                &page_record.page_id,
                page.geometry,
            )?;
        }

        orchestrator.update_progress(job_id, 55, Some("正在提取 PDF 结构"))?;
        Self::extract_and_persist_pdf_structure(
            workspace,
            &mut conn,
            &document.document_id,
            &canonical_pdf,
            &pages,
        )
        .map_err(|error| {
            let _ = fs::remove_file(&converted_pdf);
            Self::fail_preserving_pdf_artifacts(workspace, &document.document_id, job_id, error)
        })?;

        let _ = fs::remove_file(&converted_pdf);

        orchestrator.complete_import(
            &mut conn,
            &document.document_id,
            job_id,
            page_count,
            "导入完成",
        )?;
        import_guard.complete();

        if let Err(e) = ArtifactExporter::export_all(workspace) {
            eprintln!("[WARN] JSONL 导出失败，不影响导入结果: {}", e);
        }

        let updated = DocumentRepository::list_documents(&mut conn)?
            .into_iter()
            .find(|d| d.document_id == document.document_id)
            .unwrap_or(document);

        Ok(updated)
    }

    pub fn retry_import(workspace: &WorkspaceService, document_id: &str) -> AppResult<DocumentDto> {
        let layout = workspace.workspace_layout()?;
        layout.validate_storage_id(document_id, "document")?;
        let doc = {
            let mut conn = workspace.get_db_connection()?;
            let doc = DocumentRepository::find_document_by_id(&mut conn, document_id)?.ok_or_else(
                || AppError::new("document_not_found", "找不到指定的文档。", "import", false),
            )?;

            if doc.status != "failed" {
                return Err(AppError::new(
                    "document_not_failed",
                    "只能重试失败状态的文档。",
                    "import",
                    false,
                ));
            }
            doc
        };

        let original_path = Self::resolve_workspace_file(&layout, Path::new(&doc.original_path))?
            .ok_or_else(|| {
            AppError::new(
                "original_file_missing",
                "原文件不存在，无法重试。请重新选择文件导入。",
                "import",
                false,
            )
        })?;
        if !original_path.is_file() {
            return Err(AppError::new(
                "original_file_missing",
                "原文件不存在，无法重试。请重新选择文件导入。",
                "import",
                false,
            ));
        }

        let ext = doc.file_type.to_ascii_lowercase();
        if ext != "pdf" && !Self::is_image_extension(&ext) && !is_office_extension(&ext) {
            return Err(AppError::new(
                "unsupported_file_type",
                format!("不支持的文件类型: .{ext}"),
                "import",
                false,
            ));
        }

        let retry_dir_id = Uuid::new_v4().to_string();
        let retry_dir = layout.ensure_managed_document_dir(&layout.tmp_dir(), &retry_dir_id)?;
        let retry_path = retry_dir.join(Self::retry_snapshot_filename(
            &doc.original_filename,
            &ext,
            &doc.file_hash,
        ));
        Self::copy_original_file(&original_path, &retry_path).map_err(|err| {
            AppError::io("import", "retry_snapshot_copy_failed", err)
                .with_details(retry_path.to_string_lossy().to_string())
        })?;

        let result = (|| -> AppResult<DocumentDto> {
            let renderer = crate::providers::pdf_renderer::PdfiumRenderer;
            let office_converter = if is_office_extension(&ext) {
                let lo_path =
                    crate::services::settings_service::SettingsService::get_libreoffice_path(
                        workspace,
                    )?;
                let converter =
                    crate::providers::libreoffice_converter::LibreOfficeConverter::new(lo_path);
                converter.validate_configuration()?;
                Some(converter)
            } else {
                None
            };
            if ext == "pdf" {
                renderer.inspect_pdf(&retry_path)?;
                OpenDataLoaderPdfProvider::discover(layout.root())?.health_check()?;
            } else if Self::is_image_extension(&ext) {
                Self::decode_image_to_png(&retry_path)?;
            } else {
                OpenDataLoaderPdfProvider::discover(layout.root())?.health_check()?;
            }

            for parent in [
                layout.pages_dir(),
                layout.canonical_pdfs_dir(),
                layout.pdf_structure_dir(),
            ] {
                let _ = layout.resolve_existing_managed_document_dir(&parent, document_id)?;
            }

            let deleted = {
                let mut conn = workspace.get_db_connection()?;
                DocumentRepository::delete_document_records(&mut conn, document_id)?.ok_or_else(
                    || AppError::new("document_not_found", "找不到指定的文档。", "import", false),
                )?
            };

            let _ = Self::remove_workspace_file(&layout, Path::new(&deleted.original_path));
            for image_path in deleted.removable_image_paths {
                let _ = Self::remove_workspace_file(&layout, Path::new(&image_path));
            }
            let _ =
                Self::remove_managed_document_dir(&layout, &layout.pages_dir(), document_id, false);
            for parent in [layout.canonical_pdfs_dir(), layout.pdf_structure_dir()] {
                let _ = Self::remove_managed_document_dir(&layout, &parent, document_id, true);
            }

            if ext == "pdf" {
                Self::import_pdf(workspace, &retry_path, &renderer)
            } else if Self::is_image_extension(&ext) {
                Self::import_image(workspace, &retry_path)
            } else {
                let converter = office_converter.as_ref().ok_or_else(|| {
                    AppError::new(
                        "libreoffice_retry_preflight_missing",
                        "Office 重试预检状态缺失，未删除新的重试副本。",
                        "import",
                        true,
                    )
                })?;
                Self::import_document(workspace, &retry_path, &renderer, converter)
            }
        })();

        Self::restore_retry_original_on_failure(&result, &retry_path, &original_path)?;
        let _ = fs::remove_dir_all(&retry_dir);
        result
    }

    pub fn delete_document(workspace: &WorkspaceService, document_id: &str) -> AppResult<()> {
        let layout = workspace.workspace_layout()?;
        layout.validate_storage_id(document_id, "document")?;
        let document_dirs = [
            layout.resolve_existing_managed_document_dir(&layout.pages_dir(), document_id)?,
            layout
                .resolve_existing_managed_document_dir(&layout.canonical_pdfs_dir(), document_id)?,
            layout
                .resolve_existing_managed_document_dir(&layout.pdf_structure_dir(), document_id)?,
        ];
        let mut conn = workspace.get_db_connection()?;
        let artifacts = DocumentRepository::delete_document_records(&mut conn, document_id)?
            .ok_or_else(|| {
                AppError::new(
                    "document_not_found",
                    "找不到指定的文档。",
                    "document",
                    false,
                )
            })?;
        drop(conn);

        Self::remove_workspace_file(&layout, Path::new(&artifacts.original_path))?;
        for image_path in artifacts.removable_image_paths {
            Self::remove_workspace_file(&layout, Path::new(&image_path))?;
        }

        if let Some(doc_pages_dir) = &document_dirs[0] {
            match fs::remove_dir(&doc_pages_dir) {
                Ok(()) => {}
                Err(err) if err.kind() == ErrorKind::DirectoryNotEmpty => {}
                Err(err) if err.kind() == ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(
                        AppError::io("document", "document_pages_dir_delete_failed", err)
                            .with_details(doc_pages_dir.to_string_lossy().to_string()),
                    );
                }
            }
        }

        for document_dir in document_dirs.into_iter().skip(1).flatten() {
            if document_dir.exists() {
                fs::remove_dir_all(&document_dir).map_err(|err| {
                    AppError::io("document", "document_pdf_artifacts_delete_failed", err)
                        .with_details(document_dir.to_string_lossy().to_string())
                })?;
            }
        }

        if let Err(e) = ArtifactExporter::export_all(workspace) {
            eprintln!("[WARN] JSONL 导出失败，不影响文档删除结果: {}", e);
        }

        Ok(())
    }

    fn resolve_workspace_file(layout: &WorkspaceLayout, path: &Path) -> AppResult<Option<PathBuf>> {
        let workspace_root = layout.managed_parent(layout.root())?;
        let target = if path.is_absolute() {
            path.to_path_buf()
        } else {
            workspace_root.join(path)
        };

        if !target.exists() {
            return Ok(None);
        }
        let metadata = fs::symlink_metadata(&target).map_err(|err| {
            AppError::io("document", "document_file_metadata_failed", err)
                .with_details(target.to_string_lossy().to_string())
        })?;
        if is_link_or_reparse_point(&metadata) {
            return Err(AppError::new(
                "workspace_file_link_rejected",
                "工作区文件不能是符号链接或 junction。",
                "document",
                false,
            )
            .with_details(target.to_string_lossy().to_string()));
        }
        let target = fs::canonicalize(&target).map_err(|err| {
            AppError::io("document", "document_file_canonicalize_failed", err)
                .with_details(target.to_string_lossy().to_string())
        })?;
        if !target.starts_with(&workspace_root) || !target.is_file() {
            return Err(AppError::new(
                "workspace_file_outside_root",
                "工作区文件路径越界，已拒绝文件操作。",
                "document",
                false,
            )
            .with_details(target.to_string_lossy().to_string()));
        }
        Ok(Some(target))
    }

    fn remove_workspace_file(layout: &WorkspaceLayout, path: &Path) -> AppResult<()> {
        if let Some(target) = Self::resolve_workspace_file(layout, path)? {
            fs::remove_file(&target).map_err(|err| {
                AppError::io("document", "document_file_delete_failed", err)
                    .with_details(target.to_string_lossy().to_string())
            })?;
        }
        Ok(())
    }

    fn remove_managed_document_dir(
        layout: &WorkspaceLayout,
        parent: &Path,
        document_id: &str,
        recursive: bool,
    ) -> AppResult<()> {
        let Some(path) = layout.resolve_existing_managed_document_dir(parent, document_id)? else {
            return Ok(());
        };
        let result = if recursive {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_dir(&path)
        };
        match result {
            Ok(()) => Ok(()),
            Err(err)
                if err.kind() == ErrorKind::NotFound
                    || (!recursive && err.kind() == ErrorKind::DirectoryNotEmpty) =>
            {
                Ok(())
            }
            Err(err) => Err(
                AppError::io("document", "document_artifact_dir_delete_failed", err)
                    .with_details(path.to_string_lossy().to_string()),
            ),
        }
    }

    fn retry_snapshot_filename(
        original_filename: &str,
        extension: &str,
        file_hash: &str,
    ) -> String {
        let mut filename = sanitize_filename(original_filename);
        let storage_prefix = file_hash
            .get(..16)
            .filter(|prefix| prefix.bytes().all(|value| value.is_ascii_hexdigit()))
            .map(|prefix| format!("{prefix}_"));
        if let Some(prefix) = storage_prefix {
            while filename
                .get(..prefix.len())
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(&prefix))
            {
                filename = filename[prefix.len()..].to_string();
            }
        }
        if filename.is_empty()
            || !Path::new(&filename)
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case(extension))
        {
            return format!("document.{extension}");
        }
        filename
    }

    fn restore_retry_original_on_failure(
        result: &AppResult<DocumentDto>,
        retry_path: &Path,
        original_path: &Path,
    ) -> AppResult<()> {
        let Err(retry_error) = result else {
            return Ok(());
        };
        if original_path.is_file() {
            return Ok(());
        }
        Self::copy_original_file(retry_path, original_path).map_err(|restore_error| {
            AppError::new(
                "retry_original_restore_failed",
                "The retry failed and the original file could not be restored. The recovery copy was retained.",
                "import",
                false,
            )
            .with_details(format!(
                "retry_code={}; restore_error={}; recovery_copy={}",
                retry_error.code,
                restore_error,
                retry_path.display()
            ))
        })
    }

    fn copy_original_file(source: &Path, destination: &Path) -> std::io::Result<()> {
        match fs::symlink_metadata(destination) {
            Ok(metadata) => {
                if is_link_or_reparse_point(&metadata) || !metadata.is_file() {
                    return Err(std::io::Error::new(
                        ErrorKind::AlreadyExists,
                        "destination is not a regular file",
                    ));
                }
                let source_canonical = fs::canonicalize(source)?;
                let destination_canonical = fs::canonicalize(destination)?;
                if source_canonical == destination_canonical {
                    return Ok(());
                }
                return Err(std::io::Error::new(
                    ErrorKind::AlreadyExists,
                    "destination already exists",
                ));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }

        let parent = destination.parent().ok_or_else(|| {
            std::io::Error::new(ErrorKind::InvalidInput, "destination has no parent")
        })?;
        let temporary = parent.join(format!(".slicer-copy-{}.tmp", Uuid::new_v4()));
        let result = (|| -> std::io::Result<()> {
            let mut source_file = fs::File::open(source)?;
            let mut temporary_file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)?;
            std::io::copy(&mut source_file, &mut temporary_file)?;
            temporary_file.sync_all()?;
            drop(temporary_file);

            if fs::symlink_metadata(destination).is_ok() {
                return Err(std::io::Error::new(
                    ErrorKind::AlreadyExists,
                    "destination appeared while copying",
                ));
            }
            fs::rename(&temporary, destination)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    fn write_file_atomically_new(destination: &Path, bytes: &[u8]) -> std::io::Result<()> {
        if fs::symlink_metadata(destination).is_ok() {
            return Err(std::io::Error::new(
                ErrorKind::AlreadyExists,
                "destination already exists",
            ));
        }
        let parent = destination.parent().ok_or_else(|| {
            std::io::Error::new(ErrorKind::InvalidInput, "destination has no parent")
        })?;
        let temporary = parent.join(format!(".slicer-write-{}.tmp", Uuid::new_v4()));
        let result = (|| -> std::io::Result<()> {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            drop(file);
            if fs::symlink_metadata(destination).is_ok() {
                return Err(std::io::Error::new(
                    ErrorKind::AlreadyExists,
                    "destination appeared while writing",
                ));
            }
            fs::rename(&temporary, destination)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    fn persist_canonical_pdf(
        layout: &WorkspaceLayout,
        document_id: &str,
        source: &Path,
    ) -> AppResult<PathBuf> {
        let document_dir =
            layout.ensure_managed_document_dir(&layout.canonical_pdfs_dir(), document_id)?;
        let destination = document_dir.join("canonical.pdf");
        match fs::symlink_metadata(&destination) {
            Ok(metadata) => {
                if is_link_or_reparse_point(&metadata) || !metadata.is_file() {
                    return Err(AppError::new(
                        "canonical_pdf_target_invalid",
                        "规范 PDF 目标不能是符号链接、reparse point 或目录。",
                        "canonical_pdf",
                        false,
                    ));
                }
                let source_canonical = fs::canonicalize(source).map_err(|error| {
                    AppError::io("canonical_pdf", "canonical_pdf_source_invalid", error)
                })?;
                let destination_canonical = fs::canonicalize(&destination).map_err(|error| {
                    AppError::io("canonical_pdf", "canonical_pdf_target_invalid", error)
                })?;
                if source_canonical == destination_canonical {
                    return Ok(destination);
                }
                let existing_hash = compute_file_hash(&destination)?;
                let incoming_hash = compute_file_hash(source)?;
                if existing_hash == incoming_hash {
                    return Ok(destination);
                }
                return Err(AppError::new(
                    "canonical_pdf_target_exists",
                    "规范 PDF 目标已存在且内容不同，已拒绝覆盖。",
                    "canonical_pdf",
                    false,
                ));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(AppError::io(
                    "canonical_pdf",
                    "canonical_pdf_metadata_failed",
                    error,
                ));
            }
        }
        Self::copy_original_file(source, &destination)
            .map_err(|err| AppError::io("canonical_pdf", "canonical_pdf_copy_failed", err))?;
        Ok(destination)
    }

    fn record_canonical_pdf(
        conn: &mut sqlx::SqliteConnection,
        layout: &crate::artifacts::workspace_layout::WorkspaceLayout,
        document_id: &str,
        canonical_pdf: &Path,
    ) -> AppResult<()> {
        let relative_path = Self::relative_workspace_path(layout.root(), canonical_pdf)?;
        PdfStructureRepository::upsert_canonical_pdf(
            conn,
            &DocumentArtifactInput {
                artifact_id: Uuid::new_v4().to_string(),
                document_id: document_id.to_string(),
                kind: "canonical_pdf".to_string(),
                relative_path,
                content_hash: compute_file_hash(canonical_pdf)?,
                parser_name: None,
                parser_version: None,
                parser_options_json: None,
            },
        )
    }

    fn verify_registered_canonical_pdf(
        conn: &mut sqlx::SqliteConnection,
        layout: &crate::artifacts::workspace_layout::WorkspaceLayout,
        document_id: &str,
        canonical_pdf: &Path,
    ) -> AppResult<()> {
        let relative_path = Self::relative_workspace_path(layout.root(), canonical_pdf)?;
        let expected_hash = PdfStructureRepository::find_document_artifact_content_hash(
            conn,
            document_id,
            "canonical_pdf",
            &relative_path,
        )?
        .ok_or_else(|| {
            AppError::new(
                "canonical_pdf_artifact_missing",
                "规范 PDF 制品登记缺失。",
                "pdf_structure_persist",
                false,
            )
        })?;
        let actual_hash = compute_file_hash(canonical_pdf)?;
        if actual_hash != expected_hash {
            return Err(AppError::new(
                "canonical_pdf_hash_mismatch",
                "规范 PDF 在结构化期间发生变化，已拒绝保存解析结果。",
                "pdf_structure_persist",
                false,
            )
            .with_details(format!(
                "expected={expected_hash}; actual={actual_hash}; path={relative_path}"
            )));
        }
        Ok(())
    }

    fn extract_and_persist_pdf_structure(
        workspace: &WorkspaceService,
        conn: &mut sqlx::SqliteConnection,
        document_id: &str,
        canonical_pdf: &Path,
        page_metadata: &[PdfPageMetadata],
    ) -> AppResult<()> {
        let layout = workspace.workspace_layout()?;
        let pages: Vec<PdfStructurePage> = page_metadata
            .iter()
            .map(|page| PdfStructurePage {
                page_id: format!("{}_{}", document_id, page.page_number),
                page_number: page.page_number,
                geometry: page.geometry,
            })
            .collect();
        let staging_token = Uuid::new_v4().to_string();
        let tmp_dir = layout.managed_parent(&layout.tmp_dir())?;
        let staging_dir = tmp_dir.join(format!("pdf-structure-{staging_token}"));
        let mut artifact_cleanup = ArtifactDirectoryCleanup::new(staging_dir.clone());
        let provider = OpenDataLoaderPdfProvider::discover(layout.root())?;
        let extraction = match provider.extract(document_id, canonical_pdf, &pages, &staging_dir) {
            Ok(extraction) => extraction,
            Err(error) => {
                let parse_id = Uuid::new_v4().to_string();
                let raw_json_path = Self::preserve_failed_pdf_structure_json(
                    &layout,
                    document_id,
                    &parse_id,
                    &staging_dir,
                )
                .unwrap_or_default();
                let mut failed_artifact_cleanup = (!raw_json_path.is_empty())
                    .then(|| layout.root().join(&raw_json_path))
                    .and_then(|path| path.parent().map(Path::to_path_buf))
                    .map(ArtifactDirectoryCleanup::new);
                let failed_run = PdfParseRun {
                    parse_id,
                    document_id: document_id.to_string(),
                    parser_name: PDF_STRUCTURE_PARSER_NAME.to_string(),
                    parser_version: PDF_STRUCTURE_PARSER_VERSION.to_string(),
                    schema_version: PDF_STRUCTURE_SCHEMA_VERSION.to_string(),
                    parser_options_json: PDF_STRUCTURE_OPTIONS_JSON.to_string(),
                    raw_json_path,
                };
                PdfStructureRepository::record_parse_failure(conn, &failed_run, &error)?;
                if let Some(cleanup) = failed_artifact_cleanup.as_mut() {
                    cleanup.disarm();
                }
                return Err(error);
            }
        };

        Self::verify_registered_canonical_pdf(conn, &layout, document_id, canonical_pdf)?;

        let document_structure_dir =
            layout.ensure_managed_document_dir(&layout.pdf_structure_dir(), document_id)?;
        let final_dir = document_structure_dir.join(&extraction.run.parse_id);
        if final_dir.exists() {
            return Err(AppError::new(
                "pdf_structure_destination_exists",
                "PDF 结构化制品目录冲突，请重试。",
                "pdf_structure_persist",
                true,
            ));
        }
        fs::rename(&extraction.staging_dir, &final_dir).map_err(|err| {
            AppError::io(
                "pdf_structure_persist",
                "pdf_structure_artifacts_commit_failed",
                err,
            )
        })?;
        artifact_cleanup.retarget(final_dir.clone());

        let raw_json_name = extraction
            .raw_json_path
            .file_name()
            .ok_or_else(|| {
                AppError::new(
                    "pdf_structure_json_name_missing",
                    "PDF 结构化 JSON 文件名缺失。",
                    "pdf_structure_persist",
                    false,
                )
            })?
            .to_owned();
        let final_raw_json = final_dir.join(raw_json_name);
        let raw_relative = Self::relative_workspace_path(layout.root(), &final_raw_json)?;
        let structure_relative = Self::relative_workspace_path(layout.root(), &final_dir)?;
        let mut run = extraction.run;
        run.raw_json_path = raw_relative.clone();
        let mut blocks = extraction.blocks;
        for block in &mut blocks {
            if let Some(source) = block.source_image_path.as_deref() {
                block.source_image_path = Some(format!(
                    "{}/{}",
                    structure_relative.trim_end_matches('/'),
                    source.trim_start_matches('/')
                ));
            }
        }

        let mut artifacts = vec![DocumentArtifactInput {
            artifact_id: Uuid::new_v4().to_string(),
            document_id: document_id.to_string(),
            kind: "pdf_structure_json".to_string(),
            relative_path: raw_relative,
            content_hash: compute_file_hash(&final_raw_json)?,
            parser_name: Some(PDF_STRUCTURE_PARSER_NAME.to_string()),
            parser_version: Some(PDF_STRUCTURE_PARSER_VERSION.to_string()),
            parser_options_json: Some(PDF_STRUCTURE_OPTIONS_JSON.to_string()),
        }];
        let image_dir = final_dir.join("images");
        if image_dir.is_dir() {
            for path in Self::collect_managed_files(&image_dir)? {
                artifacts.push(DocumentArtifactInput {
                    artifact_id: Uuid::new_v4().to_string(),
                    document_id: document_id.to_string(),
                    kind: "pdf_structure_image".to_string(),
                    relative_path: Self::relative_workspace_path(layout.root(), &path)?,
                    content_hash: compute_file_hash(&path)?,
                    parser_name: Some(PDF_STRUCTURE_PARSER_NAME.to_string()),
                    parser_version: Some(PDF_STRUCTURE_PARSER_VERSION.to_string()),
                    parser_options_json: Some(PDF_STRUCTURE_OPTIONS_JSON.to_string()),
                });
            }
        }

        PdfStructureRepository::replace_document_structure(conn, &run, &artifacts, &blocks)?;
        artifact_cleanup.disarm();

        if let Ok(entries) = fs::read_dir(&document_structure_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path != final_dir && path.is_dir() {
                    let _ = fs::remove_dir_all(path);
                }
            }
        }
        Ok(())
    }

    fn preserve_failed_pdf_structure_json(
        layout: &WorkspaceLayout,
        document_id: &str,
        parse_id: &str,
        staging_dir: &Path,
    ) -> AppResult<String> {
        const MAX_FAILED_JSON_BYTES: u64 = 64 * 1024 * 1024;
        if !staging_dir.is_dir() {
            return Ok(String::new());
        }
        layout.validate_storage_id(parse_id, "parse")?;
        let staging_parent = layout.managed_parent(&layout.tmp_dir())?;
        let staging = fs::canonicalize(staging_dir).map_err(|err| {
            AppError::io(
                "pdf_structure_persist",
                "pdf_structure_failed_staging_invalid",
                err,
            )
        })?;
        if !staging.starts_with(&staging_parent) {
            return Err(AppError::new(
                "pdf_structure_failed_staging_outside_workspace",
                "失败的 PDF 结构化制品路径越界。",
                "pdf_structure_persist",
                false,
            ));
        }
        let raw_json = fs::read_dir(&staging)
            .map_err(|err| {
                AppError::io(
                    "pdf_structure_persist",
                    "pdf_structure_failed_staging_list_failed",
                    err,
                )
            })?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.is_file()
                    && path
                        .extension()
                        .and_then(|value| value.to_str())
                        .is_some_and(|value| value.eq_ignore_ascii_case("json"))
                    && fs::metadata(path)
                        .map(|metadata| metadata.len() <= MAX_FAILED_JSON_BYTES)
                        .unwrap_or(false)
            });
        let Some(raw_json) = raw_json else {
            return Ok(String::new());
        };
        let raw_json_name = raw_json.file_name().ok_or_else(|| {
            AppError::new(
                "pdf_structure_failed_json_name_missing",
                "失败的 PDF 结构化 JSON 文件名缺失。",
                "pdf_structure_persist",
                false,
            )
        })?;
        let document_dir =
            layout.ensure_managed_document_dir(&layout.pdf_structure_dir(), document_id)?;
        let final_dir = document_dir.join(parse_id);
        fs::create_dir(&final_dir).map_err(|err| {
            AppError::io(
                "pdf_structure_persist",
                "pdf_structure_failed_artifacts_dir_failed",
                err,
            )
        })?;
        let mut cleanup = ArtifactDirectoryCleanup::new(final_dir.clone());
        let final_json = final_dir.join(raw_json_name);
        fs::copy(&raw_json, &final_json).map_err(|err| {
            AppError::io(
                "pdf_structure_persist",
                "pdf_structure_failed_json_copy_failed",
                err,
            )
        })?;
        let relative = Self::relative_workspace_path(layout.root(), &final_json)?;
        cleanup.disarm();
        Ok(relative)
    }

    fn collect_managed_files(root: &Path) -> AppResult<Vec<PathBuf>> {
        let root = fs::canonicalize(root).map_err(|err| {
            AppError::io(
                "pdf_structure_persist",
                "pdf_structure_images_dir_invalid",
                err,
            )
        })?;
        let mut files = Vec::new();
        let mut pending = vec![root.clone()];
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(&directory).map_err(|err| {
                AppError::io(
                    "pdf_structure_persist",
                    "pdf_structure_images_list_failed",
                    err,
                )
            })? {
                let path = entry
                    .map_err(|err| {
                        AppError::io(
                            "pdf_structure_persist",
                            "pdf_structure_image_entry_failed",
                            err,
                        )
                    })?
                    .path();
                let metadata = fs::symlink_metadata(&path).map_err(|err| {
                    AppError::io(
                        "pdf_structure_persist",
                        "pdf_structure_image_metadata_failed",
                        err,
                    )
                })?;
                if is_link_or_reparse_point(&metadata) {
                    return Err(AppError::new(
                        "pdf_structure_image_link_rejected",
                        "PDF 结构化图片不能是符号链接或 junction。",
                        "pdf_structure_persist",
                        false,
                    ));
                }
                let path = fs::canonicalize(path).map_err(|err| {
                    AppError::io(
                        "pdf_structure_persist",
                        "pdf_structure_image_path_invalid",
                        err,
                    )
                })?;
                if !path.starts_with(&root) {
                    return Err(AppError::new(
                        "pdf_structure_image_outside_directory",
                        "PDF 结构化图片路径越界。",
                        "pdf_structure_persist",
                        false,
                    ));
                }
                if path.is_dir() {
                    pending.push(path);
                } else if path.is_file() {
                    files.push(path);
                }
            }
        }
        files.sort();
        Ok(files)
    }

    fn relative_workspace_path(workspace_root: &Path, path: &Path) -> AppResult<String> {
        let root = fs::canonicalize(workspace_root).map_err(|err| {
            AppError::io(
                "pdf_structure_persist",
                "workspace_canonicalize_failed",
                err,
            )
        })?;
        let path = fs::canonicalize(path).map_err(|err| {
            AppError::io("pdf_structure_persist", "artifact_canonicalize_failed", err)
        })?;
        let relative = path.strip_prefix(&root).map_err(|_| {
            AppError::new(
                "artifact_outside_workspace",
                "PDF 结构化制品不在工作区内。",
                "pdf_structure_persist",
                false,
            )
        })?;
        Ok(relative.to_string_lossy().replace('\\', "/"))
    }

    fn fail_preserving_pdf_artifacts(
        workspace: &WorkspaceService,
        document_id: &str,
        job_id: &str,
        error: AppError,
    ) -> AppError {
        if let Ok(layout) = workspace.workspace_layout() {
            let orchestrator = JobOrchestrator::new(layout);
            let _ = orchestrator.fail_import_if_active(document_id, job_id, &error, &error.message);
        }
        error
    }

    fn fail_with_cleanup(
        workspace: &WorkspaceService,
        document_id: &str,
        job_id: &str,
        code: &str,
        message: &str,
        details: &str,
    ) -> AppError {
        if let Ok(layout) = workspace.workspace_layout() {
            let orchestrator = JobOrchestrator::new(layout.clone());
            let error = AppError::new(code, message, "import", true);
            let _ = orchestrator.fail_import_if_active(document_id, job_id, &error, message);
        }

        if let Ok(layout) = workspace.workspace_layout() {
            let _ =
                Self::remove_managed_document_dir(&layout, &layout.pages_dir(), document_id, true);
        }

        AppError::new(code, message, "import", true).with_details(details.to_string())
    }

    fn fail_with_cleanup_safe(
        workspace: &WorkspaceService,
        document_id: &str,
        job_id: &str,
        code: &str,
        message: &str,
        details: &str,
        new_files: &HashSet<PathBuf>,
    ) -> AppError {
        if let Ok(layout) = workspace.workspace_layout() {
            let orchestrator = JobOrchestrator::new(layout.clone());
            let error = AppError::new(code, message, "import", true);
            let _ = orchestrator.fail_import_if_active(document_id, job_id, &error, message);
        }

        for file in new_files {
            let _ = fs::remove_file(file);
        }

        AppError::new(code, message, "import", true).with_details(details.to_string())
    }

    fn decode_image_to_png(image_path: &Path) -> AppResult<Vec<u8>> {
        let reader = image::ImageReader::open(image_path).map_err(|e| {
            AppError::new("image_read_failed", "无法读取图片文件。", "import", true)
                .with_details(e.to_string())
        })?;
        let reader = reader.with_guessed_format().map_err(|e| {
            AppError::new(
                "image_format_detect_failed",
                "无法识别图片格式。",
                "import",
                false,
            )
            .with_details(e.to_string())
        })?;
        let image = reader.decode().map_err(|e| {
            AppError::new(
                "image_decode_failed",
                "图片解码失败，文件可能已损坏或格式不受支持。",
                "import",
                false,
            )
            .with_details(e.to_string())
        })?;

        let mut png_bytes = Vec::new();
        let mut cursor = Cursor::new(&mut png_bytes);
        image.write_to(&mut cursor, ImageFormat::Png).map_err(|e| {
            AppError::new(
                "image_png_encode_failed",
                "图片转换为 PNG 失败。",
                "import",
                false,
            )
            .with_details(e.to_string())
        })?;
        Ok(png_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::{ArtifactDirectoryCleanup, ImportService, ImportStatusGuard};
    use crate::api::state::ApiAppState;
    use crate::domain::document::DocumentDto;
    use crate::errors::{AppError, AppResult};
    use crate::jobs::job_orchestrator::JobOrchestrator;
    use crate::repositories::document_repository::DocumentRepository;
    use crate::services::api_server_service::ApiServerService;
    use crate::services::workspace_service::WorkspaceService;
    use image::{ImageFormat, Rgb, RgbImage};
    use std::fs;
    use std::io::Cursor;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use uuid::Uuid;

    fn test_workspace(label: &str) -> (WorkspaceService, PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!(
            "slicer-import-image-{label}-{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ));
        let config = base.join("config");
        let workspace_dir = base.join("workspace");
        let service = WorkspaceService::new(config.clone());
        let api_state = ApiAppState::new(Arc::new(service.clone()));
        let api = ApiServerService::new(api_state);
        let selected = service.select_workspace(workspace_dir.to_string_lossy().into_owned(), &api);
        assert_eq!(selected.status, "ready");
        (service, base, workspace_dir)
    }

    fn test_image() -> RgbImage {
        RgbImage::from_pixel(3, 2, Rgb([180, 45, 90]))
    }

    fn write_png(path: &Path) {
        write_image(path, ImageFormat::Png);
    }

    fn write_jpeg(path: &Path) {
        write_image(path, ImageFormat::Jpeg);
    }

    fn write_blue_jpeg(path: &Path) {
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgb8(RgbImage::from_pixel(3, 2, Rgb([30, 80, 210])))
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Jpeg)
            .expect("encode blue jpeg");
        fs::write(path, bytes).expect("write blue jpeg");
    }

    fn write_image(path: &Path, format: ImageFormat) {
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgb8(test_image())
            .write_to(&mut Cursor::new(&mut bytes), format)
            .expect("encode image");
        fs::write(path, bytes).expect("write image");
    }

    #[test]
    fn artifact_directory_cleanup_follows_rename_until_persistence_succeeds() {
        let base = std::env::temp_dir().join(format!(
            "slicer-artifact-cleanup-{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ));
        fs::create_dir_all(&base).expect("base");

        let staging = base.join("staging");
        fs::create_dir(&staging).expect("staging");
        {
            let _cleanup = ArtifactDirectoryCleanup::new(staging.clone());
        }
        assert!(!staging.exists());

        fs::create_dir(&staging).expect("renamed staging");
        let final_dir = base.join("final");
        {
            let mut cleanup = ArtifactDirectoryCleanup::new(staging.clone());
            fs::rename(&staging, &final_dir).expect("rename");
            cleanup.retarget(final_dir.clone());
        }
        assert!(!staging.exists());
        assert!(!final_dir.exists());

        fs::create_dir(&final_dir).expect("persisted final");
        {
            let mut cleanup = ArtifactDirectoryCleanup::new(final_dir.clone());
            cleanup.disarm();
        }
        assert!(final_dir.is_dir());

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn import_image_creates_single_page_document_and_asset() {
        let (service, base, workspace_dir) = test_workspace("single");
        let source = base.join("source.png");
        write_png(&source);

        let document = ImportService::import_image(&service, &source).expect("import image");

        assert_eq!(document.original_filename, "source.png");
        assert_eq!(document.file_type, "png");
        assert_eq!(document.status, "ready");
        assert_eq!(document.page_count, Some(1));
        assert!(PathBuf::from(&document.original_path).is_file());

        let mut conn = service.get_db_connection().expect("db");
        let pages = DocumentRepository::list_pages_by_document(&mut conn, &document.document_id)
            .expect("pages");
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].page_number, 1);
        assert_eq!(pages[0].status, "rendered");

        let asset = DocumentRepository::find_image_asset_by_hash(
            &mut conn,
            pages[0].image_hash.as_deref().expect("image hash"),
        )
        .expect("asset query")
        .expect("asset");
        assert!(asset.file_path.starts_with("pages/"));
        assert!(workspace_dir.join(&asset.file_path).is_file());

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn import_image_returns_existing_document_for_same_file_hash() {
        let (service, base, _) = test_workspace("duplicate");
        let source = base.join("duplicate.png");
        write_png(&source);

        let first = ImportService::import_image(&service, &source).expect("first import");
        let second = ImportService::import_image(&service, &source).expect("second import");

        assert_eq!(first.document_id, second.document_id);

        let mut conn = service.get_db_connection().expect("db");
        let documents = DocumentRepository::list_documents(&mut conn).expect("documents");
        assert_eq!(documents.len(), 1);

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn import_image_accepts_jpg_and_jpeg_extensions() {
        let (service, base, _) = test_workspace("jpeg");
        let jpg_source = base.join("source.jpg");
        let jpeg_source = base.join("source-copy.jpeg");
        write_jpeg(&jpg_source);
        write_blue_jpeg(&jpeg_source);

        let jpg_doc = ImportService::import_image(&service, &jpg_source).expect("jpg import");
        let jpeg_doc = ImportService::import_image(&service, &jpeg_source).expect("jpeg import");

        assert_eq!(jpg_doc.file_type, "jpg");
        assert_eq!(jpg_doc.page_count, Some(1));
        assert_eq!(jpeg_doc.file_type, "jpeg");
        assert_eq!(jpeg_doc.page_count, Some(1));

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn deleting_first_document_preserves_shared_image_for_second_document() {
        let (service, base, workspace_dir) = test_workspace("shared-delete");
        let first_source = base.join("first.png");
        let second_source = base.join("second.jpg");
        write_png(&first_source);
        write_jpeg(&second_source);

        let first_doc = ImportService::import_image(&service, &first_source).expect("first import");
        let second_doc =
            ImportService::import_image(&service, &second_source).expect("second import");

        assert_ne!(first_doc.document_id, second_doc.document_id);

        let mut conn = service.get_db_connection().expect("db");
        let second_page =
            DocumentRepository::list_pages_by_document(&mut conn, &second_doc.document_id)
                .expect("second pages")
                .pop()
                .expect("second page");
        let asset_before = DocumentRepository::find_image_asset_by_hash(
            &mut conn,
            second_page.image_hash.as_deref().expect("image hash"),
        )
        .expect("asset before")
        .expect("asset before");
        drop(conn);

        ImportService::delete_document(&service, &first_doc.document_id).expect("delete first");

        let mut conn = service.get_db_connection().expect("db");
        let asset_after = DocumentRepository::find_image_asset_by_hash(
            &mut conn,
            second_page.image_hash.as_deref().expect("image hash"),
        )
        .expect("asset after")
        .expect("asset after");
        assert_eq!(asset_after.file_path, asset_before.file_path);
        assert!(workspace_dir.join(&asset_after.file_path).is_file());

        ImportService::delete_document(&service, &second_doc.document_id).expect("delete second");
        assert!(!workspace_dir.join(&asset_after.file_path).exists());

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn copying_original_to_itself_is_a_noop_for_workspace_retry() {
        let (service, base, _) = test_workspace("same-copy");
        let source = base.join("source.png");
        write_png(&source);
        let document = ImportService::import_image(&service, &source).expect("import image");
        let original_path = PathBuf::from(&document.original_path);
        let before = fs::metadata(&original_path).expect("metadata before").len();

        ImportService::copy_original_file(&original_path, &original_path).expect("same file copy");

        let after = fs::metadata(&original_path).expect("metadata after").len();
        assert_eq!(after, before);

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn copying_original_refuses_to_replace_an_existing_leaf() {
        let root = std::env::temp_dir().join(format!(
            "slicer-copy-leaf-{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("temp root");
        let source = root.join("source.txt");
        let destination = root.join("destination.txt");
        fs::write(&source, "incoming").expect("source");
        fs::write(&destination, "sentinel").expect("destination");

        let error = ImportService::copy_original_file(&source, &destination)
            .expect_err("existing leaf must not be replaced");
        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(
            fs::read_to_string(&destination).expect("sentinel remains"),
            "sentinel"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unfinished_guard_does_not_regress_a_completed_import() {
        let (service, base, _) = test_workspace("guard-terminal-cas");
        let source = base.join("guard.png");
        write_png(&source);
        let document = ImportService::import_image(&service, &source).expect("import image");
        let job_id = document.job_id.clone().expect("job id");

        drop(ImportStatusGuard::new(
            &service,
            &document.document_id,
            &job_id,
        ));

        let mut conn = service.get_db_connection().expect("db");
        let stored = DocumentRepository::find_document_by_id(&mut conn, &document.document_id)
            .expect("read document")
            .expect("document");
        assert_eq!(stored.status, "ready");
        drop(conn);
        let job = JobOrchestrator::new(service.workspace_layout().expect("layout"))
            .list_jobs()
            .expect("jobs")
            .into_iter()
            .find(|job| job.job_id == job_id)
            .expect("import job");
        assert_eq!(job.status, "succeeded");

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn retry_import_preserves_display_filename_without_hash_prefixes() {
        let (service, base, _) = test_workspace("retry-name");
        let source = base.join("quarterly-report.png");
        write_png(&source);
        let document = ImportService::import_image(&service, &source).expect("initial import");
        let mut conn = service.get_db_connection().expect("db");
        DocumentRepository::update_document_status(
            &mut conn,
            &document.document_id,
            "failed",
            document.page_count,
            Some("forced retry"),
        )
        .expect("mark failed");
        drop(conn);

        let retried = ImportService::retry_import(&service, &document.document_id).expect("retry");

        assert_eq!(retried.original_filename, "quarterly-report.png");
        assert_eq!(retried.status, "ready");
        assert_ne!(retried.document_id, document.document_id);
        let mut conn = service.get_db_connection().expect("db");
        let documents = DocumentRepository::list_documents(&mut conn).expect("documents");
        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].original_filename, "quarterly-report.png");

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn retry_snapshot_filename_removes_repeated_storage_hash_prefixes() {
        assert_eq!(
            ImportService::retry_snapshot_filename(
                "0123456789abcdef_0123456789abcdef_report.pdf",
                "pdf",
                "0123456789abcdef000000000000000000000000000000000000000000000000"
            ),
            "report.pdf"
        );
        assert_eq!(
            ImportService::retry_snapshot_filename(
                "0123456789abcdef_report.pdf",
                "pdf",
                "fedcba987654321000000000000000000000000000000000000000000000000"
            ),
            "0123456789abcdef_report.pdf"
        );
        assert_eq!(
            ImportService::retry_snapshot_filename(
                "wrong-name.txt",
                "pdf",
                "0123456789abcdef000000000000000000000000000000000000000000000000"
            ),
            "document.pdf"
        );
    }

    #[test]
    fn failed_retry_restores_the_original_from_its_recovery_copy() {
        let base = std::env::temp_dir().join(format!("slicer-retry-restore-{}", Uuid::new_v4()));
        fs::create_dir_all(base.join("originals")).expect("originals");
        let retry_path = base.join("retry.pdf");
        let original_path = base.join("originals/document.pdf");
        fs::write(&retry_path, b"recoverable original").expect("retry snapshot");
        let failure: AppResult<DocumentDto> = Err(AppError::new(
            "retry_fixture_failed",
            "retry failed",
            "import",
            true,
        ));

        ImportService::restore_retry_original_on_failure(&failure, &retry_path, &original_path)
            .expect("restore original");

        assert_eq!(
            fs::read(&original_path).expect("restored bytes"),
            b"recoverable original"
        );
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn preserves_only_bounded_failed_json_and_finds_nested_structure_images() {
        let (service, base, workspace_dir) = test_workspace("failed-structure-artifacts");
        let layout = service.workspace_layout().expect("layout");
        let staging = layout
            .tmp_dir()
            .join(format!("pdf-structure-{}", Uuid::new_v4()));
        let nested_images = staging.join("images").join("figures");
        fs::create_dir_all(&nested_images).expect("staging");
        fs::write(staging.join("broken.json"), br#"{"kids":"invalid"}"#).expect("raw json");
        fs::write(nested_images.join("figure.png"), b"png bytes").expect("image");
        let document_id = Uuid::new_v4().to_string();
        let parse_id = Uuid::new_v4().to_string();

        let relative = ImportService::preserve_failed_pdf_structure_json(
            &layout,
            &document_id,
            &parse_id,
            &staging,
        )
        .expect("preserve");

        assert_eq!(
            relative,
            format!("structure/{document_id}/{parse_id}/broken.json")
        );
        assert!(workspace_dir.join(&relative).is_file());
        let persisted_dir = workspace_dir
            .join("structure")
            .join(&document_id)
            .join(&parse_id);
        assert!(!persisted_dir.join("images").exists());
        let images =
            ImportService::collect_managed_files(&staging.join("images")).expect("nested images");
        assert_eq!(images.len(), 1);
        assert!(images[0].ends_with(Path::new("figures").join("figure.png")));

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn import_status_guard_marks_unfinished_documents_failed() {
        let (service, base, _) = test_workspace("status-guard");
        let layout = service.workspace_layout().expect("layout");
        let orchestrator = JobOrchestrator::new(layout);
        let job = orchestrator.create_job("image_import").expect("job");
        let mut conn = service.get_db_connection().expect("db");
        let document = DocumentRepository::create_document(
            &mut conn,
            "guard.png",
            "png",
            "guard-hash",
            "originals/guard.png",
            Some(&job.job_id),
        )
        .expect("document");
        drop(conn);

        {
            let _guard = ImportStatusGuard::new(&service, &document.document_id, &job.job_id);
        }

        let mut conn = service.get_db_connection().expect("db");
        let recovered = DocumentRepository::find_document_by_id(&mut conn, &document.document_id)
            .expect("lookup")
            .expect("document");
        assert_eq!(recovered.status, "failed");
        assert_eq!(
            recovered.error_summary.as_deref(),
            Some("导入流程意外中止，文档已保留并可重试。")
        );

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn import_image_rejects_corrupt_supported_extension() {
        let (service, base, _) = test_workspace("corrupt");
        let source = base.join("broken.png");
        fs::write(&source, b"not a png").expect("write corrupt image");

        let error = ImportService::import_image(&service, &source).expect_err("corrupt image");

        assert_eq!(error.code, "image_decode_failed");

        let _ = fs::remove_dir_all(base);
    }
}
