use serde::Serialize;

pub const DOCUMENT_VIEWER_FORMATS: [&str; 6] = ["pdf", "annot", "preview", "html", "md", "json"];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DocumentViewerFormatAvailabilityDto {
    pub format: String,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DocumentViewerManifestDto {
    pub document_id: String,
    pub original_filename: String,
    pub page_count: Option<i64>,
    pub formats: Vec<DocumentViewerFormatAvailabilityDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DocumentViewerAssetDto {
    pub source: String,
    pub mime_type: String,
    pub data_url: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DocumentViewerContentDto {
    pub format: String,
    pub mime_type: String,
    pub encoding: String,
    pub content: String,
    pub assets: Vec<DocumentViewerAssetDto>,
}
