use crate::artifacts::page_json_exporter::{PageJsonExporter, PageJsonlLine};
use crate::errors::AppResult;
use crate::repositories::document_repository::DocumentRepository;
use crate::repositories::ledger_repository::LedgerRepository;
use crate::services::workspace_service::WorkspaceService;
use std::fs;
use std::io::Write;
use std::path::Path;

pub struct ArtifactExporter;

impl ArtifactExporter {
    pub fn export_all(workspace: &WorkspaceService) -> AppResult<()> {
        let layout = workspace.workspace_layout()?;
        let metadata_dir = layout.metadata_dir();
        fs::create_dir_all(&metadata_dir)
            .map_err(|e| crate::errors::AppError::io("export", "metadata_dir_create_failed", e))?;

        let mut conn = workspace.get_db_connection()?;
        let page_lines = PageJsonExporter::build_lines(&mut conn)?;
        let documents = DocumentRepository::list_documents(&mut conn)?;
        let jobs = LedgerRepository::new(layout.clone()).list_jobs()?;

        atomic_write_page_jsonl(&metadata_dir.join("pages.jsonl"), &page_lines)?;
        atomic_write_jsonl(&metadata_dir.join("documents.jsonl"), &documents)?;
        atomic_write_jsonl(&metadata_dir.join("jobs.jsonl"), &jobs)?;

        Ok(())
    }

    pub fn export_pages(workspace: &WorkspaceService) -> AppResult<()> {
        let layout = workspace.workspace_layout()?;
        let metadata_dir = layout.metadata_dir();
        fs::create_dir_all(&metadata_dir)
            .map_err(|e| crate::errors::AppError::io("export", "metadata_dir_create_failed", e))?;

        let mut conn = workspace.get_db_connection()?;
        let page_lines = PageJsonExporter::build_lines(&mut conn)?;
        atomic_write_page_jsonl(&metadata_dir.join("pages.jsonl"), &page_lines)?;
        Ok(())
    }
}

fn atomic_write_page_jsonl(target: &Path, items: &[PageJsonlLine]) -> AppResult<()> {
    let tmp_path = target.with_extension("jsonl.tmp");

    let mut file = fs::File::create(&tmp_path)
        .map_err(|e| crate::errors::AppError::io("export", "jsonl_tmp_create_failed", e))?;

    for item in items {
        let line = PageJsonExporter::serialize_line(item)?;
        writeln!(file, "{}", line)
            .map_err(|e| crate::errors::AppError::io("export", "jsonl_write_failed", e))?;
    }

    fs::rename(&tmp_path, target)
        .map_err(|e| crate::errors::AppError::io("export", "jsonl_rename_failed", e))?;

    Ok(())
}

fn atomic_write_jsonl<T: serde::Serialize>(target: &Path, items: &[T]) -> AppResult<()> {
    let tmp_path = target.with_extension("jsonl.tmp");

    let mut file = fs::File::create(&tmp_path)
        .map_err(|e| crate::errors::AppError::io("export", "jsonl_tmp_create_failed", e))?;

    for item in items {
        let line = serde_json::to_string(item).map_err(|e| {
            crate::errors::AppError::new(
                "jsonl_serialize_failed",
                "JSONL 序列化失败。",
                "export",
                false,
            )
            .with_details(e.to_string())
        })?;
        writeln!(file, "{}", line)
            .map_err(|e| crate::errors::AppError::io("export", "jsonl_write_failed", e))?;
    }

    fs::rename(&tmp_path, target)
        .map_err(|e| crate::errors::AppError::io("export", "jsonl_rename_failed", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::atomic_write_page_jsonl;
    use crate::artifacts::page_json_exporter::{PageBaselineJsonl, PageJsonlLine};
    use std::fs;

    fn baseline(page_id: &str, page_number: i64) -> PageJsonlLine {
        PageJsonlLine::Baseline(PageBaselineJsonl {
            page_id: page_id.to_string(),
            document_id: "d1".to_string(),
            page_number,
            image_hash: format!("h{page_number}"),
            image_path: format!("pages/d1/h{page_number}.png"),
            status: "rendered".to_string(),
            error_summary: None,
            created_at: "2026-05-19T00:00:00Z".to_string(),
            updated_at: "2026-05-19T00:00:00Z".to_string(),
        })
    }

    #[test]
    fn atomic_write_replaces_target_and_leaves_no_tmp_file() {
        let root = std::env::temp_dir().join(format!(
            "slicer-jsonl-atomic-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("mkdir");

        let target = root.join("pages.jsonl");
        atomic_write_page_jsonl(&target, &[baseline("p1", 1)]).expect("first write");
        atomic_write_page_jsonl(&target, &[baseline("p2", 2)]).expect("second write");

        let content = fs::read_to_string(&target).expect("read");
        assert!(content.contains("\"page_id\":\"p2\""));
        assert!(!content.contains("\"page_id\":\"p1\""));
        assert!(!target.with_extension("jsonl.tmp").exists());

        let _ = fs::remove_dir_all(root);
    }
}
