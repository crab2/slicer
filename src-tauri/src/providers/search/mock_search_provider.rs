use crate::domain::index::{
    legacy_page_hit_id, ProviderBuildStats, SearchHitDto, SearchIndexDocument,
};
use crate::errors::AppResult;
use crate::providers::search::search_provider::SearchProvider;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

pub struct MockSearchProvider {
    pub hits: Mutex<HashMap<String, Vec<SearchHitDto>>>,
}

impl MockSearchProvider {
    pub fn new() -> Self {
        Self {
            hits: Mutex::new(HashMap::new()),
        }
    }

    pub fn set_hits(&self, query: &str, hits: Vec<(String, f32)>) {
        let hits = hits
            .into_iter()
            .map(|(page_id, score)| SearchHitDto {
                hit_id: legacy_page_hit_id(&page_id),
                page_id,
                module_id: None,
                module_type: "page".to_string(),
                snippet: String::new(),
                bbox: None,
                module_json: None,
                document_id: None,
                page_number: None,
                image_path: None,
                original_filename: None,
                score,
            })
            .collect();
        self.hits
            .lock()
            .expect("mock hits lock")
            .insert(query.to_string(), hits);
    }

    pub fn set_module_hits(&self, query: &str, hits: Vec<SearchHitDto>) {
        self.hits
            .lock()
            .expect("mock hits lock")
            .insert(query.to_string(), hits);
    }
}

impl Default for MockSearchProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchProvider for MockSearchProvider {
    fn provider_id(&self) -> &'static str {
        "mock_search"
    }

    fn analyzer_version(&self) -> &'static str {
        "mock_v1"
    }

    fn health_check(&self, _index_path: &Path) -> AppResult<()> {
        Ok(())
    }

    fn build_index(
        &self,
        _build_path: &Path,
        documents: &[SearchIndexDocument],
    ) -> AppResult<ProviderBuildStats> {
        Ok(ProviderBuildStats {
            document_count: documents.len(),
        })
    }

    fn search(
        &self,
        _index_path: &Path,
        query: &str,
        limit: usize,
    ) -> AppResult<Vec<SearchHitDto>> {
        let map = self.hits.lock().expect("mock hits lock");
        let hits = map.get(query).cloned().unwrap_or_default();
        Ok(hits.into_iter().take(limit).collect())
    }
}
