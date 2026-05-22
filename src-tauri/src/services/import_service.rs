use crate::artifacts::jsonl_exporter::ArtifactExporter;
use crate::domain::document::DocumentDto;
use crate::errors::{AppError, AppResult};
use crate::jobs::job_orchestrator::JobOrchestrator;
use crate::providers::converter::{detect_file_type, is_office_extension, DocumentConverter};
use crate::providers::pdf_renderer::{compute_file_hash, sanitize_filename, PdfRenderer};
use crate::repositories::db::block_on_db;
use crate::repositories::document_repository::DocumentRepository;
use crate::services::workspace_service::WorkspaceService;
use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub struct ImportService;

impl ImportService {
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

        fs::copy(pdf_path, &dest_path).map_err(|e| {
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

        if !is_office_extension(&ext) {
            return Err(AppError::new(
                "unsupported_file_type",
                format!("不支持的文件类型: .{ext}，当前支持 PDF、DOC、DOCX、PPT、PPTX。"),
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

        fs::copy(file_path, &dest_path).map_err(|e| {
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
}
