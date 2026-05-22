use crate::domain::index::{
    ProviderBuildStats, SearchHitDto, SearchIndexDocument, DEFAULT_SEARCH_PROVIDER_ID,
    TANTIVY_ANALYZER_VERSION,
};
use crate::errors::{AppError, AppResult};
use crate::providers::search::chinese_analyzer::cjk_bigram_analyzer;
use crate::providers::search::search_provider::SearchProvider;
use std::fs;
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, TextFieldIndexing, TextOptions, Value, STORED, STRING};
use tantivy::{doc, Index, IndexWriter, ReloadPolicy, TantivyDocument};

const FIELD_PAGE_ID: &str = "page_id";
const FIELD_DOCUMENT_ID: &str = "document_id";
const FIELD_PAGE_NUMBER: &str = "page_number";
const FIELD_IMAGE_PATH: &str = "image_path";
const FIELD_ORIGINAL_FILENAME: &str = "original_filename";
const FIELD_BODY: &str = "body";

pub struct TantivyBm25SearchProvider;

impl TantivyBm25SearchProvider {
    fn build_schema() -> (Schema, SearchFields) {
        let mut builder = Schema::builder();
        let text_options = TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("cjk_bigram")
                .set_index_option(tantivy::schema::IndexRecordOption::WithFreqsAndPositions),
        );
        let page_id = builder.add_text_field(FIELD_PAGE_ID, STRING | STORED);
        let document_id = builder.add_text_field(FIELD_DOCUMENT_ID, STRING | STORED);
        let page_number = builder.add_i64_field(FIELD_PAGE_NUMBER, STORED);
        let image_path = builder.add_text_field(FIELD_IMAGE_PATH, STRING | STORED);
        let original_filename = builder.add_text_field(FIELD_ORIGINAL_FILENAME, STRING | STORED);
        let body = builder.add_text_field(FIELD_BODY, text_options);
        (
            builder.build(),
            SearchFields {
                page_id,
                document_id,
                page_number,
                image_path,
                original_filename,
                body,
            },
        )
    }

    fn open_index(index_path: &Path) -> AppResult<(Index, SearchFields)> {
        if !index_path.exists() {
            return Err(AppError::new(
                "index_not_found",
                "索引目录不存在，请先构建索引。",
                "search",
                true,
            ));
        }
        let (schema, fields) = Self::build_schema();
        let index = Index::open_in_dir(index_path).map_err(|err| {
            AppError::new(
                "index_open_failed",
                "无法打开本地搜索索引。",
                "search",
                true,
            )
            .with_details(err.to_string())
        })?;
        if index.schema() != schema {
            return Err(AppError::new(
                "index_schema_mismatch",
                "索引结构不匹配，请重建索引。",
                "search",
                true,
            ));
        }
        Ok((index, fields))
    }
}

struct SearchFields {
    page_id: Field,
    document_id: Field,
    page_number: Field,
    image_path: Field,
    original_filename: Field,
    body: Field,
}

impl SearchProvider for TantivyBm25SearchProvider {
    fn provider_id(&self) -> &'static str {
        DEFAULT_SEARCH_PROVIDER_ID
    }

    fn analyzer_version(&self) -> &'static str {
        TANTIVY_ANALYZER_VERSION
    }

    fn health_check(&self, index_path: &Path) -> AppResult<()> {
        let (index, fields) = Self::open_index(index_path)?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|err| {
                AppError::new("index_reader_failed", "索引读取失败。", "search", true)
                    .with_details(err.to_string())
            })?;
        let searcher = reader.searcher();
        if searcher.num_docs() == 0 {
            return Err(AppError::new(
                "index_empty",
                "索引中没有可搜索的页面。",
                "search",
                true,
            ));
        }
        let _ = fields;
        Ok(())
    }

    fn build_index(
        &self,
        build_path: &Path,
        documents: &[SearchIndexDocument],
    ) -> AppResult<ProviderBuildStats> {
        if build_path.exists() {
            fs::remove_dir_all(build_path)
                .map_err(|err| AppError::io("index", "index_build_dir_cleanup_failed", err))?;
        }
        fs::create_dir_all(build_path)
            .map_err(|err| AppError::io("index", "index_build_dir_create_failed", err))?;

        let (schema, fields) = Self::build_schema();
        let index = Index::create_in_dir(build_path, schema.clone()).map_err(|err| {
            AppError::new("index_create_failed", "创建搜索索引失败。", "index", true)
                .with_details(err.to_string())
        })?;
        index
            .tokenizers()
            .register("cjk_bigram", cjk_bigram_analyzer());

        let mut writer: IndexWriter = index.writer(50_000_000).map_err(|err| {
            AppError::new(
                "index_writer_failed",
                "索引写入器初始化失败。",
                "index",
                true,
            )
            .with_details(err.to_string())
        })?;

        for document in documents {
            let combined = document.combined_index_text();
            if combined.trim().is_empty() {
                continue;
            }
            let tantivy_doc = doc!(
                fields.page_id => document.page_id.clone(),
                fields.document_id => document.document_id.clone(),
                fields.page_number => document.page_number,
                fields.image_path => document.image_path.clone(),
                fields.original_filename => document.original_filename.clone().unwrap_or_default(),
                fields.body => combined,
            );
            writer.add_document(tantivy_doc).map_err(|err| {
                AppError::new(
                    "index_document_add_failed",
                    "写入索引文档失败。",
                    "index",
                    true,
                )
                .with_details(err.to_string())
            })?;
        }

        writer.commit().map_err(|err| {
            AppError::new("index_commit_failed", "提交搜索索引失败。", "index", true)
                .with_details(err.to_string())
        })?;

        Ok(ProviderBuildStats {
            document_count: documents.len(),
        })
    }

    fn search(&self, index_path: &Path, query: &str, limit: usize) -> AppResult<Vec<SearchHitDto>> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }
        let limit = limit.clamp(1, 100);
        let (index, fields) = Self::open_index(index_path)?;
        index
            .tokenizers()
            .register("cjk_bigram", cjk_bigram_analyzer());
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|err| {
                AppError::new("index_reader_failed", "索引读取失败。", "search", true)
                    .with_details(err.to_string())
            })?;
        let searcher = reader.searcher();
        let query_parser = QueryParser::for_index(&index, vec![fields.body]);
        let parsed = query_parser.parse_query(trimmed).map_err(|err| {
            AppError::new(
                "search_query_invalid",
                "搜索关键词无效，请调整后重试。",
                "search",
                false,
            )
            .with_details(err.to_string())
        })?;
        let top_docs = searcher
            .search(&parsed, &TopDocs::with_limit(limit))
            .map_err(|err| {
                AppError::new("search_query_failed", "搜索执行失败。", "search", true)
                    .with_details(err.to_string())
            })?;

        let mut hits = Vec::with_capacity(top_docs.len());
        for (score, doc_address) in top_docs {
            let retrieved: TantivyDocument = searcher.doc(doc_address).map_err(|err| {
                AppError::new(
                    "search_doc_load_failed",
                    "读取搜索结果失败。",
                    "search",
                    true,
                )
                .with_details(err.to_string())
            })?;
            let page_id = retrieved
                .get_first(fields.page_id)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            hits.push(SearchHitDto { page_id, score });
        }
        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::TantivyBm25SearchProvider;
    use crate::domain::index::SearchIndexDocument;
    use crate::providers::search::search_provider::SearchProvider;
    use std::fs;

    fn sample_doc(page_id: &str, title: &str, body: &str, filename: &str) -> SearchIndexDocument {
        SearchIndexDocument {
            page_id: page_id.to_string(),
            document_id: "doc-1".to_string(),
            page_number: 1,
            image_path: format!("pages/doc-1/{page_id}.png"),
            original_filename: Some(filename.to_string()),
            title: Some(title.to_string()),
            summary: Some("摘要".to_string()),
            visible_text: Some(body.to_string()),
            topics: vec!["主题".to_string()],
            keywords: vec!["关键词".to_string()],
            bm25_text: body.to_string(),
        }
    }

    #[test]
    fn chinese_query_ranks_relevant_page_higher() {
        let root = std::env::temp_dir().join(format!(
            "slicer-tantivy-中文路径 test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("temp dir");
        let index_path = root.join("index");

        let provider = TantivyBm25SearchProvider;
        let docs = vec![
            sample_doc("page-a", "采购合同", "本页讨论采购合同条款", "采购合同.pdf"),
            sample_doc("page-b", "会议纪要", "团队周会安排", "会议.pdf"),
        ];
        provider
            .build_index(&index_path, &docs)
            .expect("build index");

        let hits = provider.search(&index_path, "采购合同", 5).expect("search");
        assert!(!hits.is_empty());
        assert_eq!(hits[0].page_id, "page-a");
        assert!(hits[0].score >= hits.last().map(|h| h.score).unwrap_or(0.0));

        let _ = fs::remove_dir_all(&root);
    }
}
