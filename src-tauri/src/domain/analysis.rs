use serde::{Deserialize, Serialize};

pub const PAGE_ANALYSIS_SCHEMA_VERSION: &str = "page_analysis_v1";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PageAnalysisV1 {
    pub schema_version: String,
    pub page_id: String,
    pub image_hash: String,
    pub image_path: String,
    pub source: PageAnalysisSource,
    pub analysis: PageAnalysisContent,
    pub retrieval: PageRetrievalFields,
    pub model: PageAnalysisModelInfo,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_response: Option<ProviderResponseRecord>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PageAnalysisSource {
    pub document_id: String,
    pub page_number: i64,
    pub original_filename: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PageAnalysisContent {
    pub title: Option<String>,
    pub summary: Option<String>,
    pub visible_text: Option<String>,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PageRetrievalFields {
    pub bm25_text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PageAnalysisModelInfo {
    pub provider: String,
    pub model_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderResponseRecord {
    pub endpoint_kind: String,
    pub raw_json: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct AnalysisResultDto {
    pub analysis_id: String,
    pub page_id: String,
    pub schema_version: String,
    pub provider: String,
    pub model_name: String,
    pub status: String,
    pub result_json: Option<String>,
    pub error_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PageAnalysisSummaryDto {
    pub title: Option<String>,
    pub summary: Option<String>,
    pub keywords: Vec<String>,
    pub topic_count: usize,
    pub visible_text_char_count: usize,
}

impl PageAnalysisSummaryDto {
    pub fn from_analysis(analysis: &PageAnalysisV1) -> Self {
        let visible_text_char_count = analysis
            .analysis
            .visible_text
            .as_deref()
            .map(str::len)
            .unwrap_or(0);
        Self {
            title: analysis.analysis.title.clone(),
            summary: analysis.analysis.summary.clone(),
            keywords: analysis.analysis.keywords.clone(),
            topic_count: analysis.analysis.topics.len(),
            visible_text_char_count,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PageWorkbenchDto {
    pub page_id: String,
    pub document_id: String,
    pub page_number: i64,
    pub image_hash: String,
    pub image_path: Option<String>,
    pub status: String,
    pub error_summary: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub analysis_summary: Option<PageAnalysisSummaryDto>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct AnalysisBatchResultDto {
    pub job_id: String,
    pub total_pages: i64,
    pub succeeded_pages: i64,
    pub failed_pages: i64,
    pub skipped_pages: i64,
    pub status: String,
    pub updated_at: String,
}
