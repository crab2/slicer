use crate::artifacts::jsonl_exporter::ArtifactExporter;
use crate::domain::document::DocumentDto;
use crate::errors::{AppError, AppResult};
use crate::jobs::job_orchestrator::JobOrchestrator;
use crate::providers::converter::{detect_file_type, is_office_extension, DocumentConverter};
use crate::providers::pdf_renderer::{
    compute_file_hash, compute_image_hash, sanitize_filename, PdfRenderer,
};
use crate::repositories::db::block_on_db;
use crate::repositories::document_repository::DocumentRepository;
use crate::services::workspace_service::WorkspaceService;
use image::ImageFormat;
use std::collections::HashSet;
use std::fs;
use std::io::{Cursor, ErrorKind};
use std::path::{Path, PathBuf};

pub struct ImportService;

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
        let originals_dir = layout.originals_dir();
        let pages_dir = layout.pages_dir();
        let tmp_dir = layout.tmp_dir();

        let original_name = image_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown-image")
            .to_string();

        let file_hash = compute_file_hash(image_path)?;
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

        Self::copy_original_file(image_path, &dest_path).map_err(|e| {
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

        let png_bytes = Self::decode_image_to_png(image_path).map_err(|e| {
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
        let doc_pages_dir = pages_dir.join(&document.document_id);
        fs::create_dir_all(&doc_pages_dir).map_err(|e| {
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
            let tmp_path = tmp_dir.join(format!("{}_{}", document.document_id, png_filename));
            let final_path = doc_pages_dir.join(&png_filename);

            fs::write(&tmp_path, &png_bytes).map_err(|e| {
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

            fs::rename(&tmp_path, &final_path).map_err(|e| {
                Self::fail_with_cleanup_safe(
                    workspace,
                    &document.document_id,
                    job_id,
                    "page_rename_failed",
                    "页面图片原子写入失败。",
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

        DocumentRepository::update_document_status(
            &mut conn,
            &document.document_id,
            "ready",
            Some(1),
            None,
        )?;

        orchestrator.update_progress(job_id, 100, Some("导入完成"))?;

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
        let originals_dir = layout.originals_dir();
        let pages_dir = layout.pages_dir();
        let tmp_dir = layout.tmp_dir();

        let original_name = pdf_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown.pdf")
            .to_string();

        let file_hash = compute_file_hash(pdf_path)?;
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

        Self::copy_original_file(pdf_path, &dest_path).map_err(|e| {
            Self::fail_with_cleanup(
                workspace,
                &document.document_id,
                job_id,
                "file_copy_failed",
                "无法复制原文件到工作区。",
                &e.to_string(),
            )
        })?;

        orchestrator.update_progress(job_id, 10, Some("正在渲染 PDF 页面"))?;

        let expected_page_count = renderer.page_count(pdf_path).map_err(|e| {
            Self::fail_with_cleanup(
                workspace,
                &document.document_id,
                job_id,
                "pdf_page_count_failed",
                "PDF 页数读取失败。",
                &e.to_string(),
            )
        })?;
        if expected_page_count == 0 {
            return Err(Self::fail_with_cleanup(
                workspace,
                &document.document_id,
                job_id,
                "pdf_empty_document",
                "PDF 文件没有可渲染页面。",
                "page_count=0",
            ));
        }

        let pages = renderer.render_pdf(pdf_path, 144.0).map_err(|e| {
            Self::fail_with_cleanup(
                workspace,
                &document.document_id,
                job_id,
                "pdf_render_failed",
                "PDF 页面渲染失败。",
                &e.to_string(),
            )
        })?;

        let page_count = pages.len() as i64;
        if page_count == 0 {
            return Err(Self::fail_with_cleanup(
                workspace,
                &document.document_id,
                job_id,
                "pdf_empty_document",
                "PDF 文件没有可渲染页面。",
                "rendered_pages=0",
            ));
        }
        let doc_pages_dir = pages_dir.join(&document.document_id);
        fs::create_dir_all(&doc_pages_dir).map_err(|e| {
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
        for (i, page) in pages.iter().enumerate() {
            let progress = (10 + ((i + 1) * 80 / pages.len())).min(90) as u8;
            orchestrator.update_progress(
                job_id,
                progress,
                Some(&format!("正在写入第 {} 页", page.page_number)),
            )?;

            let existing =
                DocumentRepository::find_image_asset_by_hash(&mut conn, &page.image_hash)?;
            if existing.is_some() {
                DocumentRepository::create_page_record(
                    &mut conn,
                    &document.document_id,
                    page.page_number,
                    &page.image_hash,
                )?;
                continue;
            }

            let png_filename = format!("{}.png", page.image_hash);
            let tmp_path = tmp_dir.join(format!("{}_{}", document.document_id, png_filename));
            let final_path = doc_pages_dir.join(&png_filename);

            fs::write(&tmp_path, &page.png_bytes).map_err(|e| {
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

            fs::rename(&tmp_path, &final_path).map_err(|e| {
                Self::fail_with_cleanup_safe(
                    workspace,
                    &document.document_id,
                    job_id,
                    "page_rename_failed",
                    "页面图片原子写入失败。",
                    &e.to_string(),
                    &new_files,
                )
            })?;

            new_files.insert(final_path);

            let file_size = page.png_bytes.len() as i64;
            let rel_path = format!("pages/{}/{}.png", document.document_id, page.image_hash);

            DocumentRepository::create_image_asset(
                &mut conn,
                &page.image_hash,
                &rel_path,
                file_size,
            )?;
            DocumentRepository::create_page_record(
                &mut conn,
                &document.document_id,
                page.page_number,
                &page.image_hash,
            )?;
        }

        DocumentRepository::update_document_status(
            &mut conn,
            &document.document_id,
            "ready",
            Some(page_count),
            None,
        )?;

        orchestrator.update_progress(job_id, 100, Some("导入完成"))?;

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
        let originals_dir = layout.originals_dir();
        let tmp_dir = layout.tmp_dir();
        let pages_dir = layout.pages_dir();

        let original_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let file_hash = compute_file_hash(file_path)?;
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

        Self::copy_original_file(file_path, &dest_path).map_err(|e| {
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

        let converted_pdf = converter.convert_to_pdf(file_path, &tmp_dir).map_err(|e| {
            Self::fail_with_cleanup(
                workspace,
                &document.document_id,
                job_id,
                "conversion_failed",
                "Office 文档转换为 PDF 失败。",
                &e.to_string(),
            )
        })?;

        orchestrator.update_progress(job_id, 30, Some("正在渲染 PDF 页面"))?;

        let expected_page_count = renderer.page_count(&converted_pdf).map_err(|e| {
            let _ = fs::remove_file(&converted_pdf);
            Self::fail_with_cleanup(
                workspace,
                &document.document_id,
                job_id,
                "pdf_page_count_failed",
                "PDF 页数读取失败。",
                &e.to_string(),
            )
        })?;
        if expected_page_count == 0 {
            let _ = fs::remove_file(&converted_pdf);
            return Err(Self::fail_with_cleanup(
                workspace,
                &document.document_id,
                job_id,
                "pdf_empty_document",
                "PDF 文件没有可渲染页面。",
                "page_count=0",
            ));
        }

        let pages = renderer.render_pdf(&converted_pdf, 144.0).map_err(|e| {
            let _ = fs::remove_file(&converted_pdf);
            Self::fail_with_cleanup(
                workspace,
                &document.document_id,
                job_id,
                "pdf_render_failed",
                "PDF 页面渲染失败。",
                &e.to_string(),
            )
        })?;

        let page_count = pages.len() as i64;
        if page_count == 0 {
            let _ = fs::remove_file(&converted_pdf);
            return Err(Self::fail_with_cleanup(
                workspace,
                &document.document_id,
                job_id,
                "pdf_empty_document",
                "PDF 文件没有可渲染页面。",
                "rendered_pages=0",
            ));
        }
        let doc_pages_dir = pages_dir.join(&document.document_id);
        fs::create_dir_all(&doc_pages_dir).map_err(|e| {
            let _ = fs::remove_file(&converted_pdf);
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
        for (i, page) in pages.iter().enumerate() {
            let progress = (30 + ((i + 1) * 60 / pages.len())).min(90) as u8;
            orchestrator.update_progress(
                job_id,
                progress,
                Some(&format!("正在写入第 {} 页", page.page_number)),
            )?;

            let existing =
                DocumentRepository::find_image_asset_by_hash(&mut conn, &page.image_hash)?;
            if existing.is_some() {
                DocumentRepository::create_page_record(
                    &mut conn,
                    &document.document_id,
                    page.page_number,
                    &page.image_hash,
                )?;
                continue;
            }

            let png_filename = format!("{}.png", page.image_hash);
            let tmp_path = tmp_dir.join(format!("{}_{}", document.document_id, png_filename));
            let final_path = doc_pages_dir.join(&png_filename);

            fs::write(&tmp_path, &page.png_bytes).map_err(|e| {
                let _ = fs::remove_file(&converted_pdf);
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

            fs::rename(&tmp_path, &final_path).map_err(|e| {
                let _ = fs::remove_file(&converted_pdf);
                Self::fail_with_cleanup_safe(
                    workspace,
                    &document.document_id,
                    job_id,
                    "page_rename_failed",
                    "页面图片原子写入失败。",
                    &e.to_string(),
                    &new_files,
                )
            })?;

            new_files.insert(final_path);

            let file_size = page.png_bytes.len() as i64;
            let rel_path = format!("pages/{}/{}.png", document.document_id, page.image_hash);

            DocumentRepository::create_image_asset(
                &mut conn,
                &page.image_hash,
                &rel_path,
                file_size,
            )?;
            DocumentRepository::create_page_record(
                &mut conn,
                &document.document_id,
                page.page_number,
                &page.image_hash,
            )?;
        }

        let _ = fs::remove_file(&converted_pdf);

        DocumentRepository::update_document_status(
            &mut conn,
            &document.document_id,
            "ready",
            Some(page_count),
            None,
        )?;

        orchestrator.update_progress(job_id, 100, Some("导入完成"))?;

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
        let original_path;
        {
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

            original_path = PathBuf::from(&doc.original_path);
        }

        if !original_path.exists() {
            return Err(AppError::new(
                "original_file_missing",
                "原文件不存在，无法重试。请重新选择文件导入。",
                "import",
                false,
            ));
        }

        if let Ok(layout) = workspace.workspace_layout() {
            let pages_dir = layout.pages_dir().join(document_id);
            let _ = fs::remove_dir_all(&pages_dir);
        }

        {
            let mut conn = workspace.get_db_connection()?;
            let did = document_id.to_string();
            block_on_db(async move {
                sqlx::query("DELETE FROM page_records WHERE document_id = ?1")
                    .bind(&did)
                    .execute(&mut conn)
                    .await
                    .map_err(|err| {
                        crate::repositories::db::database_error(
                            "import",
                            "page_records_cleanup_failed",
                            err,
                        )
                    })?;
                sqlx::query("DELETE FROM documents WHERE document_id = ?1")
                    .bind(&did)
                    .execute(&mut conn)
                    .await
                    .map_err(|err| {
                        crate::repositories::db::database_error(
                            "import",
                            "document_cleanup_failed",
                            err,
                        )
                    })?;
                Ok::<(), AppError>(())
            })?;
        }

        let ext = original_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if ext == "pdf" {
            let renderer = crate::providers::pdf_renderer::PdfiumRenderer;
            Self::import_pdf(workspace, &original_path, &renderer)
        } else if Self::is_image_extension(&ext) {
            Self::import_image(workspace, &original_path)
        } else if is_office_extension(&ext) {
            let renderer = crate::providers::pdf_renderer::PdfiumRenderer;
            let lo_path = crate::services::settings_service::SettingsService::get_libreoffice_path(
                workspace,
            )?;
            let converter =
                crate::providers::libreoffice_converter::LibreOfficeConverter::new(lo_path);
            Self::import_document(workspace, &original_path, &renderer, &converter)
        } else {
            Err(AppError::new(
                "unsupported_file_type",
                format!("不支持的文件类型: .{ext}"),
                "import",
                false,
            ))
        }
    }

    pub fn delete_document(workspace: &WorkspaceService, document_id: &str) -> AppResult<()> {
        let layout = workspace.workspace_layout()?;
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

        Self::remove_workspace_file(layout.root(), Path::new(&artifacts.original_path))?;
        for image_path in artifacts.removable_image_paths {
            Self::remove_workspace_file(layout.root(), Path::new(&image_path))?;
        }

        let doc_pages_dir = layout.pages_dir().join(document_id);
        if doc_pages_dir.exists() && doc_pages_dir.starts_with(layout.root()) {
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

        if let Err(e) = ArtifactExporter::export_all(workspace) {
            eprintln!("[WARN] JSONL 导出失败，不影响文档删除结果: {}", e);
        }

        Ok(())
    }

    fn remove_workspace_file(workspace_root: &Path, path: &Path) -> AppResult<()> {
        let target = if path.is_absolute() {
            path.to_path_buf()
        } else {
            workspace_root.join(path)
        };

        if !target.starts_with(workspace_root) || !target.exists() {
            return Ok(());
        }
        if target.is_file() {
            fs::remove_file(&target).map_err(|err| {
                AppError::io("document", "document_file_delete_failed", err)
                    .with_details(target.to_string_lossy().to_string())
            })?;
        }
        Ok(())
    }

    fn copy_original_file(source: &Path, destination: &Path) -> std::io::Result<()> {
        if destination.exists() {
            let source_canonical = fs::canonicalize(source)?;
            let destination_canonical = fs::canonicalize(destination)?;
            if source_canonical == destination_canonical {
                return Ok(());
            }
        }
        fs::copy(source, destination).map(|_| ())
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
            let _ = orchestrator.mark_failed(job_id, &error, message);
        }

        if let Ok(mut conn) = workspace.get_db_connection() {
            let _ = DocumentRepository::update_document_status(
                &mut conn,
                document_id,
                "failed",
                None,
                Some(message),
            );
        }

        if let Ok(layout) = workspace.workspace_layout() {
            let pages_dir = layout.pages_dir().join(document_id);
            let _ = fs::remove_dir_all(&pages_dir);
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
            let _ = orchestrator.mark_failed(job_id, &error, message);
        }

        if let Ok(mut conn) = workspace.get_db_connection() {
            let _ = DocumentRepository::update_document_status(
                &mut conn,
                document_id,
                "failed",
                None,
                Some(message),
            );
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
    use super::ImportService;
    use crate::api::state::ApiAppState;
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

        let asset = DocumentRepository::find_image_asset_by_hash(&mut conn, &pages[0].image_hash)
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
        let asset_before =
            DocumentRepository::find_image_asset_by_hash(&mut conn, &second_page.image_hash)
                .expect("asset before")
                .expect("asset before");
        drop(conn);

        ImportService::delete_document(&service, &first_doc.document_id).expect("delete first");

        let mut conn = service.get_db_connection().expect("db");
        let asset_after =
            DocumentRepository::find_image_asset_by_hash(&mut conn, &second_page.image_hash)
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
    fn import_image_rejects_corrupt_supported_extension() {
        let (service, base, _) = test_workspace("corrupt");
        let source = base.join("broken.png");
        fs::write(&source, b"not a png").expect("write corrupt image");

        let error = ImportService::import_image(&service, &source).expect_err("corrupt image");

        assert_eq!(error.code, "image_decode_failed");

        let _ = fs::remove_dir_all(base);
    }
}
