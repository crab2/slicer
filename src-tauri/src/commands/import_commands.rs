use crate::domain::document::DocumentDto;
use crate::domain::page::PageRecordDto;
use crate::providers::converter::is_office_extension;
use crate::providers::libreoffice_converter::LibreOfficeConverter;
use crate::providers::pdf_renderer::PdfiumRenderer;
use crate::services::import_service::ImportService;
use crate::services::settings_service::SettingsService;
use crate::services::workspace_service::WorkspaceService;
use std::path::PathBuf;
use tauri::State;

#[tauri::command]
pub async fn import_pdf(
    workspace: State<'_, WorkspaceService>,
    file_path: String,
) -> Result<DocumentDto, String> {
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let path = PathBuf::from(&file_path);
        let renderer = PdfiumRenderer;

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        match ext.as_deref() {
            Some("pdf") => {
                ImportService::import_pdf(&workspace, &path, &renderer).map_err(|e| e.to_string())
            }
            Some(ext) if ImportService::is_image_extension(ext) => {
                ImportService::import_image(&workspace, &path).map_err(|e| e.to_string())
            }
            Some(ext) if is_office_extension(ext) => {
                let lo_path =
                    SettingsService::get_libreoffice_path(&workspace).map_err(|e| e.to_string())?;
                let converter = LibreOfficeConverter::new(lo_path);
                ImportService::import_document(&workspace, &path, &renderer, &converter)
                    .map_err(|e| e.to_string())
            }
            Some(ext) => Err(format!(
                "不支持的文件类型: .{ext}，当前支持 PDF、DOC、DOCX、PPT、PPTX、PNG、JPG、JPEG。"
            )),
            None => Err("无法识别文件类型，文件缺少扩展名。".to_string()),
        }
    })
    .await
    .map_err(|e| format!("导入任务执行失败: {e}"))?
}

#[tauri::command]
pub async fn import_image(
    workspace: State<'_, WorkspaceService>,
    file_path: String,
) -> Result<DocumentDto, String> {
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        ImportService::import_image(&workspace, &PathBuf::from(&file_path))
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("图片导入任务执行失败: {e}"))?
}

#[tauri::command]
pub fn list_documents(workspace: State<'_, WorkspaceService>) -> Result<Vec<DocumentDto>, String> {
    let mut conn = workspace.get_db_connection().map_err(|e| e.to_string())?;
    crate::repositories::document_repository::DocumentRepository::list_documents(&mut conn)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn retry_import(
    workspace: State<'_, WorkspaceService>,
    document_id: String,
) -> Result<DocumentDto, String> {
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        ImportService::retry_import(&workspace, &document_id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("重试任务执行失败: {e}"))?
}

#[tauri::command]
pub async fn delete_document(
    workspace: State<'_, WorkspaceService>,
    document_id: String,
) -> Result<(), String> {
    let workspace = workspace.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        ImportService::delete_document(&workspace, &document_id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("删除文档任务执行失败: {e}"))?
}

#[tauri::command]
pub fn list_pages(
    workspace: State<'_, WorkspaceService>,
    document_id: String,
) -> Result<Vec<PageRecordDto>, String> {
    let mut conn = workspace.get_db_connection().map_err(|e| e.to_string())?;
    crate::repositories::document_repository::DocumentRepository::list_pages_by_document(
        &mut conn,
        &document_id,
    )
    .map_err(|e| e.to_string())
}
