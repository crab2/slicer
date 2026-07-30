use serde::{Deserialize, Serialize};

use crate::domain::pdf_structure::NormalizedBbox;

pub const DEFAULT_SEARCH_PROVIDER_ID: &str = "tantivy_bm25";
pub const MODULE_INDEX_SCHEMA_VERSION: &str = "pdf_modules_v2";
pub const TANTIVY_ANALYZER_VERSION: &str = "cjk_bigram_v2";

pub fn legacy_page_hit_id(page_id: &str) -> String {
    format!("page:{page_id}")
}

pub fn module_hit_id(module_id: &str) -> String {
    format!("module:{module_id}")
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct IndexVersionDto {
    pub version_id: String,
    pub provider: String,
    pub analyzer_version: String,
    pub content_schema_version: String,
    pub content_fingerprint: String,
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

#[derive(Debug, Clone, Serialize)]
pub struct SearchIndexDocument {
    pub hit_id: String,
    pub page_id: String,
    pub module_id: Option<String>,
    pub module_type: String,
    pub snippet: String,
    pub bbox: Option<NormalizedBbox>,
    pub module_json: Option<String>,
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
    pub hit_id: String,
    pub page_id: String,
    pub module_id: Option<String>,
    pub module_type: String,
    pub snippet: String,
    pub bbox: Option<NormalizedBbox>,
    pub module_json: Option<String>,
    pub document_id: Option<String>,
    pub page_number: Option<i64>,
    pub image_path: Option<String>,
    pub original_filename: Option<String>,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SearchResultPageDto {
    pub page_id: String,
    pub document_id: String,
    pub page_number: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SearchResultItemDto {
    pub hit_id: String,
    pub module_id: Option<String>,
    #[serde(rename = "type")]
    pub module_type: String,
    pub snippet: String,
    pub page: SearchResultPageDto,
    pub bbox: Option<NormalizedBbox>,
    pub module_json: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::{module_hit_id, SearchResultItemDto, SearchResultPageDto};
    use crate::domain::pdf_structure::NormalizedBbox;

    #[test]
    fn module_result_serializes_localization_contract_and_legacy_fields() {
        let item = SearchResultItemDto {
            hit_id: module_hit_id("block-2"),
            module_id: Some("block-2".to_string()),
            module_type: "paragraph".to_string(),
            snippet: "matched text".to_string(),
            page: SearchResultPageDto {
                page_id: "page-1".to_string(),
                document_id: "doc-1".to_string(),
                page_number: 3,
            },
            bbox: Some(NormalizedBbox {
                x: 0.1,
                y: 0.2,
                width: 0.3,
                height: 0.4,
            }),
            module_json: Some("{}".to_string()),
            page_id: "page-1".to_string(),
            document_id: "doc-1".to_string(),
            page_number: 3,
            original_filename: Some("source.pdf".to_string()),
            score: 1.25,
            title: None,
            summary: None,
            image_path: None,
            image_available: false,
            page_json: "{}".to_string(),
        };

        let json = serde_json::to_value(item).expect("serialize search result");
        assert_eq!(json["hit_id"], "module:block-2");
        assert_eq!(json["module_id"], "block-2");
        assert_eq!(json["type"], "paragraph");
        assert!(json.get("module_type").is_none());
        assert_eq!(json["page"]["page_id"], "page-1");
        assert_eq!(json["bbox"]["x"], 0.1);
        assert_eq!(json["page_id"], "page-1");
    }
}
