use crate::errors::AppResult;
use std::path::{Path, PathBuf};

pub trait DocumentConverter: Send + Sync {
    fn convert_to_pdf(&self, input_path: &Path, output_dir: &Path) -> AppResult<PathBuf>;
}

pub fn is_office_extension(ext: &str) -> bool {
    matches!(ext.to_lowercase().as_str(), "doc" | "docx" | "ppt" | "pptx")
}

pub fn detect_file_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("pdf") | Some("PDF") => "pdf",
        Some("doc") | Some("DOC") => "doc",
        Some("docx") | Some("DOCX") => "docx",
        Some("ppt") | Some("PPT") => "ppt",
        Some("pptx") | Some("PPTX") => "pptx",
        _ => "unknown",
    }
}
