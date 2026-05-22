use serde::{Deserialize, Serialize};

pub const DEFAULT_SEARCH_PROVIDER_ID: &str = "tantivy_bm25";
pub const TANTIVY_ANALYZER_VERSION: &str = "cjk_bigram_v1";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct IndexVersionDto {
    pub version_id: String,
    pub provider: String,
    pub analyzer_version: String,
    pub status: String,
    pub index_directory: String,
    pub document_count: i64,
    pub build_started_at: Option<String>,
    pub build_finished_at: Option<String>,
    pub activated_at: Option<String>,
    pub error_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct IndexStatusDto {
    pub status: String,
    pub provider: String,
    pub active_version_id: Option<String>,
    pub indexed_page_count: i64,
    pub analyzable_page_count: i64,
    pub pending_index_page_count: i64,
    pub building_version_id: Option<String>,
    pub building_job_id: Option<String>,
    pub error_summary: Option<String>,
    pub correlation_id: Option<String>,
    pub can_search: bool,
    pub can_rebuild: bool,
    pub stale: bool,
    pub stale_reason: Option<String>,
    pub search_uses_stale_index: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct IndexRebuildStartDto {
    pub job_id: String,
    pub version_id: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct IndexRebuildResultDto {
    pub job_id: String,
    pub version_id: String,
    pub status: String,
    pub indexed_pages: i64,
    pub skipped_pages: i64,
    pub failed_pages: i64,
    pub error_summary: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct SearchIndexDocument {
    pub page_id: String,
    pub document_id: String,
    pub page_number: i64,
    pub image_path: String,
    pub original_filename: Option<String>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub visible_text: Option<String>,
    pub topics: Vec<String>,
    pub keywords: Vec<String>,
    pub bm25_text: String,
}

impl SearchIndexDocument {
    pub fn combined_index_text(&self) -> String {
        let mut parts = Vec::new();
        if let Some(title) = &self.title {
            if !title.trim().is_empty() {
                parts.push(title.trim().to_string());
            }
        }
        if let Some(summary) = &self.summary {
            if !summary.trim().is_empty() {
                parts.push(summary.trim().to_string());
            }
        }
        if let Some(text) = &self.visible_text {
            if !text.trim().is_empty() {
                parts.push(text.trim().to_string());
            }
        }
        if !self.topics.is_empty() {
            parts.push(self.topics.join(" "));
        }
        if !self.keywords.is_empty() {
            parts.push(self.keywords.join(" "));
        }
        if !self.bm25_text.trim().is_empty() {
            parts.push(self.bm25_text.trim().to_string());
        }
        if let Some(name) = &self.original_filename {
            if !name.trim().is_empty() {
                parts.push(name.trim().to_string());
            }
        }
        parts.join("\n")
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SearchHitDto {
    pub page_id: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SearchResultItemDto {
    pub page_id: String,
    pub document_id: String,
    pub page_number: i64,
    pub original_filename: Option<String>,
    pub score: f32,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub image_path: Option<String>,
    pub image_available: bool,
    pub page_json: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SearchResponseDto {
    pub items: Vec<SearchResultItemDto>,
    pub query: String,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProviderBuildStats {
    pub document_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ActiveIndexPointer {
    pub version_id: String,
    pub provider: String,
    pub analyzer_version: String,
}
