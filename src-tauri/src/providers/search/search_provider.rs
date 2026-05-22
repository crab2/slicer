use crate::domain::index::{ProviderBuildStats, SearchHitDto, SearchIndexDocument};
use crate::errors::AppResult;
use std::path::Path;

pub trait SearchProvider: Send + Sync {
    fn provider_id(&self) -> &'static str;
    fn analyzer_version(&self) -> &'static str;

    fn health_check(&self, index_path: &Path) -> AppResult<()>;

    fn build_index(
        &self,
        build_path: &Path,
        documents: &[SearchIndexDocument],
    ) -> AppResult<ProviderBuildStats>;

    fn search(&self, index_path: &Path, query: &str, limit: usize) -> AppResult<Vec<SearchHitDto>>;
}
