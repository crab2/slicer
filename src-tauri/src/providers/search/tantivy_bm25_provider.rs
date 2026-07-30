use crate::artifacts::workspace_layout::is_link_or_reparse_point;
use crate::domain::index::{
    legacy_page_hit_id, module_hit_id, ProviderBuildStats, SearchHitDto, SearchIndexDocument,
    DEFAULT_SEARCH_PROVIDER_ID, TANTIVY_ANALYZER_VERSION,
};
use crate::domain::pdf_structure::NormalizedBbox;
use crate::errors::{AppError, AppResult};
use crate::providers::search::chinese_analyzer::cjk_bigram_analyzer;
use crate::providers::search::search_provider::SearchProvider;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::{AllQuery, QueryParser};
use tantivy::schema::{Field, Schema, TextFieldIndexing, TextOptions, Value, STORED, STRING};
use tantivy::{doc, Index, IndexWriter, ReloadPolicy, TantivyDocument};

const FIELD_HIT_ID: &str = "hit_id";
const FIELD_PAGE_ID: &str = "page_id";
const FIELD_MODULE_ID: &str = "module_id";
const FIELD_MODULE_TYPE: &str = "module_type";
const FIELD_SNIPPET: &str = "snippet";
const FIELD_BBOX_JSON: &str = "bbox_json";
const FIELD_MODULE_JSON: &str = "module_json";
const FIELD_DOCUMENT_ID: &str = "document_id";
const FIELD_PAGE_NUMBER: &str = "page_number";
const FIELD_IMAGE_PATH: &str = "image_path";
const FIELD_ORIGINAL_FILENAME: &str = "original_filename";
const FIELD_BODY: &str = "body";

pub struct TantivyBm25SearchProvider;

impl TantivyBm25SearchProvider {
    fn text_options() -> TextOptions {
        TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("cjk_bigram")
                .set_index_option(tantivy::schema::IndexRecordOption::WithFreqsAndPositions),
        )
    }

    fn build_schema() -> (Schema, ModuleSearchFields) {
        let mut builder = Schema::builder();
        let hit_id = builder.add_text_field(FIELD_HIT_ID, STRING | STORED);
        let page_id = builder.add_text_field(FIELD_PAGE_ID, STRING | STORED);
        let module_id = builder.add_text_field(FIELD_MODULE_ID, STRING | STORED);
        let module_type = builder.add_text_field(FIELD_MODULE_TYPE, STRING | STORED);
        let snippet = builder.add_text_field(FIELD_SNIPPET, STORED);
        let bbox_json = builder.add_text_field(FIELD_BBOX_JSON, STORED);
        let module_json = builder.add_text_field(FIELD_MODULE_JSON, STORED);
        let document_id = builder.add_text_field(FIELD_DOCUMENT_ID, STRING | STORED);
        let page_number = builder.add_i64_field(FIELD_PAGE_NUMBER, STORED);
        let image_path = builder.add_text_field(FIELD_IMAGE_PATH, STRING | STORED);
        let original_filename = builder.add_text_field(FIELD_ORIGINAL_FILENAME, STRING | STORED);
        let body = builder.add_text_field(FIELD_BODY, Self::text_options());
        (
            builder.build(),
            ModuleSearchFields {
                hit_id,
                page_id,
                module_id,
                module_type,
                snippet,
                bbox_json,
                module_json,
                document_id,
                page_number,
                image_path,
                original_filename,
                body,
            },
        )
    }

    fn build_legacy_schema() -> (Schema, LegacySearchFields) {
        let mut builder = Schema::builder();
        let page_id = builder.add_text_field(FIELD_PAGE_ID, STRING | STORED);
        let document_id = builder.add_text_field(FIELD_DOCUMENT_ID, STRING | STORED);
        let page_number = builder.add_i64_field(FIELD_PAGE_NUMBER, STORED);
        let image_path = builder.add_text_field(FIELD_IMAGE_PATH, STRING | STORED);
        let original_filename = builder.add_text_field(FIELD_ORIGINAL_FILENAME, STRING | STORED);
        let body = builder.add_text_field(FIELD_BODY, Self::text_options());
        (
            builder.build(),
            LegacySearchFields {
                page_id,
                document_id,
                page_number,
                image_path,
                original_filename,
                body,
            },
        )
    }

    fn open_index(index_path: &Path) -> AppResult<(Index, SearchSchema)> {
        if !index_path.exists() {
            return Err(AppError::new(
                "index_not_found",
                "索引目录不存在，请先构建索引。",
                "search",
                true,
            ));
        }
        let index = Index::open_in_dir(index_path).map_err(|err| {
            AppError::new(
                "index_open_failed",
                "无法打开本地搜索索引。",
                "search",
                true,
            )
            .with_details(err.to_string())
        })?;
        let schema = index.schema();
        let (module_schema, module_fields) = Self::build_schema();
        if schema == module_schema {
            return Ok((index, SearchSchema::Module(module_fields)));
        }
        let (legacy_schema, legacy_fields) = Self::build_legacy_schema();
        if schema == legacy_schema {
            return Ok((index, SearchSchema::Legacy(legacy_fields)));
        }
        Err(AppError::new(
            "index_schema_mismatch",
            "索引结构不受支持，请重新构建索引。",
            "search",
            true,
        ))
    }

    fn validate_index_document(document: &SearchIndexDocument) -> AppResult<()> {
        if document.hit_id.trim().is_empty() || document.page_id.trim().is_empty() {
            return Err(AppError::new(
                "index_document_identity_invalid",
                "索引单元缺少稳定标识。",
                "index",
                false,
            ));
        }
        if document.module_type.trim().is_empty() {
            return Err(AppError::new(
                "index_document_type_invalid",
                "索引单元缺少模块类型。",
                "index",
                false,
            ));
        }
        match document.module_id.as_deref() {
            Some(module_id) if document.hit_id == module_hit_id(module_id) => {}
            None if document.module_type == "page"
                && document.hit_id == legacy_page_hit_id(&document.page_id) => {}
            _ => {
                return Err(AppError::new(
                    "index_document_hit_id_invalid",
                    "索引单元的模块标识或命中标识无效。",
                    "index",
                    false,
                )
                .with_details(document.hit_id.clone()));
            }
        }
        if document.module_id.is_some() && document.module_json.is_none() {
            return Err(AppError::new(
                "index_document_json_missing",
                "模块索引单元缺少模块 JSON。",
                "index",
                false,
            )
            .with_details(document.hit_id.clone()));
        }
        if let Some(bbox) = &document.bbox {
            if !valid_bbox(bbox) {
                return Err(AppError::new(
                    "index_document_bbox_invalid",
                    "索引单元的规范化坐标无效。",
                    "index",
                    false,
                )
                .with_details(document.hit_id.clone()));
            }
        }
        if let Some(module_json) = &document.module_json {
            serde_json::from_str::<serde_json::Value>(module_json).map_err(|err| {
                AppError::new(
                    "index_document_json_invalid",
                    "索引单元包含无效的模块 JSON。",
                    "index",
                    false,
                )
                .with_details(format!("{}: {err}", document.hit_id))
            })?;
        }
        Ok(())
    }

    pub fn validate_build(&self, index_path: &Path, expected_count: usize) -> AppResult<()> {
        let (index, schema) = Self::open_index(index_path)?;
        let SearchSchema::Module(fields) = schema else {
            return Err(AppError::new(
                "index_build_schema_invalid",
                "新索引未使用模块级结构构建。",
                "index",
                false,
            ));
        };
        let reader = index.reader().map_err(|err| {
            AppError::new("index_reader_failed", "无法验证新搜索索引。", "index", true)
                .with_details(err.to_string())
        })?;
        let searcher = reader.searcher();
        if searcher.num_docs() as usize != expected_count || expected_count == 0 {
            return Err(AppError::new(
                "index_build_count_mismatch",
                "新索引的单元数量与已验证输入不一致。",
                "index",
                false,
            )
            .with_details(format!(
                "expected={expected_count}, actual={}",
                searcher.num_docs()
            )));
        }
        let addresses = searcher
            .search(&AllQuery, &TopDocs::with_limit(expected_count))
            .map_err(|err| {
                AppError::new(
                    "index_build_scan_failed",
                    "无法扫描新索引以完成验证。",
                    "index",
                    true,
                )
                .with_details(err.to_string())
            })?;
        let mut hit_ids = HashSet::with_capacity(expected_count);
        for (_, address) in addresses {
            let document: TantivyDocument = searcher.doc(address).map_err(|err| {
                AppError::new(
                    "index_build_document_invalid",
                    "无法读取新索引中的单元。",
                    "index",
                    false,
                )
                .with_details(err.to_string())
            })?;
            let hit_id = non_empty_stored_string(&document, fields.hit_id).ok_or_else(|| {
                AppError::new(
                    "index_build_hit_id_missing",
                    "新索引中的单元缺少命中标识。",
                    "index",
                    false,
                )
            })?;
            if !hit_ids.insert(hit_id.clone()) {
                return Err(AppError::new(
                    "index_duplicate_hit_id",
                    "新索引包含重复的命中标识。",
                    "index",
                    false,
                )
                .with_details(hit_id));
            }
            if non_empty_stored_string(&document, fields.page_id).is_none()
                || non_empty_stored_string(&document, fields.module_type).is_none()
            {
                return Err(AppError::new(
                    "index_build_provenance_missing",
                    "新索引中的单元缺少页面来源。",
                    "index",
                    false,
                ));
            }
            let page_id = non_empty_stored_string(&document, fields.page_id)
                .expect("validated page identifier");
            let module_type = non_empty_stored_string(&document, fields.module_type)
                .expect("validated module type");
            let module_id = non_empty_stored_string(&document, fields.module_id);
            let canonical_hit_id = module_id
                .as_deref()
                .map(module_hit_id)
                .unwrap_or_else(|| legacy_page_hit_id(&page_id));
            if hit_id != canonical_hit_id || (module_id.is_none() && module_type != "page") {
                return Err(AppError::new(
                    "index_build_hit_id_invalid",
                    "新索引中的单元命中来源不一致。",
                    "index",
                    false,
                )
                .with_details(hit_id));
            }
            if let Some(raw_bbox) = non_empty_stored_string(&document, fields.bbox_json) {
                let bbox = serde_json::from_str::<NormalizedBbox>(&raw_bbox).map_err(|err| {
                    AppError::new(
                        "index_build_bbox_invalid",
                        "新索引中的单元包含无效的坐标 JSON。",
                        "index",
                        false,
                    )
                    .with_details(err.to_string())
                })?;
                if !valid_bbox(&bbox) {
                    return Err(AppError::new(
                        "index_build_bbox_invalid",
                        "新索引中的单元坐标超出有效范围。",
                        "index",
                        false,
                    ));
                }
            }
            if let Some(module_json) = non_empty_stored_string(&document, fields.module_json) {
                serde_json::from_str::<serde_json::Value>(&module_json).map_err(|err| {
                    AppError::new(
                        "index_build_module_json_invalid",
                        "新索引中的单元包含无效的模块 JSON。",
                        "index",
                        false,
                    )
                    .with_details(err.to_string())
                })?;
            } else if module_id.is_some() {
                return Err(AppError::new(
                    "index_build_module_json_missing",
                    "新索引中的模块单元缺少来源 JSON。",
                    "index",
                    false,
                ));
            }
        }
        if hit_ids.len() != expected_count {
            return Err(AppError::new(
                "index_build_scan_incomplete",
                "新索引验证扫描不完整。",
                "index",
                false,
            ));
        }
        Ok(())
    }
}

enum SearchSchema {
    Module(ModuleSearchFields),
    Legacy(LegacySearchFields),
}

impl SearchSchema {
    fn body(&self) -> Field {
        match self {
            Self::Module(fields) => fields.body,
            Self::Legacy(fields) => fields.body,
        }
    }
}

struct ModuleSearchFields {
    hit_id: Field,
    page_id: Field,
    module_id: Field,
    module_type: Field,
    snippet: Field,
    bbox_json: Field,
    module_json: Field,
    document_id: Field,
    page_number: Field,
    image_path: Field,
    original_filename: Field,
    body: Field,
}

struct LegacySearchFields {
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
        let (index, _schema) = Self::open_index(index_path)?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|err| {
                AppError::new(
                    "index_reader_failed",
                    "无法初始化搜索索引读取器。",
                    "search",
                    true,
                )
                .with_details(err.to_string())
            })?;
        if reader.searcher().num_docs() == 0 {
            return Err(AppError::new(
                "index_empty",
                "索引中没有可搜索的模块。",
                "search",
                true,
            ));
        }
        Ok(())
    }

    fn build_index(
        &self,
        build_path: &Path,
        documents: &[SearchIndexDocument],
    ) -> AppResult<ProviderBuildStats> {
        let mut prepared = Vec::with_capacity(documents.len());
        let mut hit_ids = HashSet::with_capacity(documents.len());
        for document in documents {
            let combined = document.combined_index_text();
            if combined.trim().is_empty() {
                continue;
            }
            Self::validate_index_document(document)?;
            if !hit_ids.insert(document.hit_id.as_str()) {
                return Err(AppError::new(
                    "index_duplicate_hit_id",
                    "新索引包含重复的命中标识。",
                    "index",
                    false,
                )
                .with_details(document.hit_id.clone()));
            }
            let bbox_json = document
                .bbox
                .as_ref()
                .map(serde_json::to_string)
                .transpose()
                .map_err(|err| {
                    AppError::new(
                        "index_bbox_serialize_failed",
                        "无法序列化模块坐标。",
                        "index",
                        false,
                    )
                    .with_details(err.to_string())
                })?
                .unwrap_or_default();
            prepared.push((document, combined, bbox_json));
        }

        if build_path.exists() {
            let metadata = fs::symlink_metadata(build_path)
                .map_err(|err| AppError::io("index", "index_build_dir_metadata_failed", err))?;
            if is_link_or_reparse_point(&metadata) {
                return Err(AppError::new(
                    "index_build_dir_link_rejected",
                    "索引构建目录不能是符号链接或重解析点。",
                    "index",
                    false,
                ));
            }
            fs::remove_dir_all(build_path)
                .map_err(|err| AppError::io("index", "index_build_dir_cleanup_failed", err))?;
        }
        fs::create_dir_all(build_path)
            .map_err(|err| AppError::io("index", "index_build_dir_create_failed", err))?;

        let (schema, fields) = Self::build_schema();
        let index = Index::create_in_dir(build_path, schema).map_err(|err| {
            AppError::new("index_create_failed", "无法创建搜索索引。", "index", true)
                .with_details(err.to_string())
        })?;
        index
            .tokenizers()
            .register("cjk_bigram", cjk_bigram_analyzer());

        let mut writer: IndexWriter = index.writer(50_000_000).map_err(|err| {
            AppError::new(
                "index_writer_failed",
                "无法初始化搜索索引写入器。",
                "index",
                true,
            )
            .with_details(err.to_string())
        })?;

        for (document, combined, bbox_json) in &prepared {
            let tantivy_doc = doc!(
                fields.hit_id => document.hit_id.clone(),
                fields.page_id => document.page_id.clone(),
                fields.module_id => document.module_id.clone().unwrap_or_default(),
                fields.module_type => document.module_type.clone(),
                fields.snippet => document.snippet.clone(),
                fields.bbox_json => bbox_json.clone(),
                fields.module_json => document.module_json.clone().unwrap_or_default(),
                fields.document_id => document.document_id.clone(),
                fields.page_number => document.page_number,
                fields.image_path => document.image_path.clone(),
                fields.original_filename => document.original_filename.clone().unwrap_or_default(),
                fields.body => combined.clone(),
            );
            writer.add_document(tantivy_doc).map_err(|err| {
                AppError::new(
                    "index_document_add_failed",
                    "无法将模块写入搜索索引。",
                    "index",
                    true,
                )
                .with_details(err.to_string())
            })?;
        }

        writer.commit().map_err(|err| {
            AppError::new("index_commit_failed", "无法提交搜索索引。", "index", true)
                .with_details(err.to_string())
        })?;

        Ok(ProviderBuildStats {
            document_count: prepared.len(),
        })
    }

    fn search(&self, index_path: &Path, query: &str, limit: usize) -> AppResult<Vec<SearchHitDto>> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }
        let limit = limit.clamp(1, 100);
        let (index, schema) = Self::open_index(index_path)?;
        index
            .tokenizers()
            .register("cjk_bigram", cjk_bigram_analyzer());
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|err| {
                AppError::new(
                    "index_reader_failed",
                    "无法初始化搜索索引读取器。",
                    "search",
                    true,
                )
                .with_details(err.to_string())
            })?;
        let searcher = reader.searcher();
        let query_parser = QueryParser::for_index(&index, vec![schema.body()]);
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
                    "无法读取搜索结果。",
                    "search",
                    true,
                )
                .with_details(err.to_string())
            })?;
            let hit = match &schema {
                SearchSchema::Module(fields) => module_hit(&retrieved, fields, score),
                SearchSchema::Legacy(fields) => legacy_hit(&retrieved, fields, score),
            };
            if let Some(hit) = hit {
                hits.push(hit);
            }
        }
        Ok(hits)
    }
}

fn module_hit(
    document: &TantivyDocument,
    fields: &ModuleSearchFields,
    score: f32,
) -> Option<SearchHitDto> {
    let hit_id = stored_string(document, fields.hit_id)?;
    let page_id = stored_string(document, fields.page_id)?;
    let module_id = non_empty_stored_string(document, fields.module_id);
    let module_type = non_empty_stored_string(document, fields.module_type)
        .unwrap_or_else(|| "unknown".to_string());
    let snippet = stored_string(document, fields.snippet).unwrap_or_default();
    let bbox = non_empty_stored_string(document, fields.bbox_json)
        .and_then(|value| serde_json::from_str::<NormalizedBbox>(&value).ok())
        .filter(valid_bbox);
    let module_json = non_empty_stored_string(document, fields.module_json);
    Some(SearchHitDto {
        hit_id,
        page_id,
        module_id,
        module_type,
        snippet,
        bbox,
        module_json,
        document_id: non_empty_stored_string(document, fields.document_id),
        page_number: document
            .get_first(fields.page_number)
            .and_then(|value| value.as_i64()),
        image_path: non_empty_stored_string(document, fields.image_path),
        original_filename: non_empty_stored_string(document, fields.original_filename),
        score,
    })
}

fn legacy_hit(
    document: &TantivyDocument,
    fields: &LegacySearchFields,
    score: f32,
) -> Option<SearchHitDto> {
    let page_id = stored_string(document, fields.page_id)?;
    Some(SearchHitDto {
        hit_id: legacy_page_hit_id(&page_id),
        page_id,
        module_id: None,
        module_type: "page".to_string(),
        snippet: String::new(),
        bbox: None,
        module_json: None,
        document_id: document
            .get_first(fields.document_id)
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string),
        page_number: document
            .get_first(fields.page_number)
            .and_then(|value| value.as_i64()),
        image_path: document
            .get_first(fields.image_path)
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string),
        original_filename: document
            .get_first(fields.original_filename)
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string),
        score,
    })
}

fn stored_string(document: &TantivyDocument, field: Field) -> Option<String> {
    document
        .get_first(field)
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn non_empty_stored_string(document: &TantivyDocument, field: Field) -> Option<String> {
    stored_string(document, field).filter(|value| !value.trim().is_empty())
}

fn valid_bbox(bbox: &NormalizedBbox) -> bool {
    const EPSILON: f64 = 1e-9;
    bbox.x.is_finite()
        && bbox.y.is_finite()
        && bbox.width.is_finite()
        && bbox.height.is_finite()
        && bbox.x >= 0.0
        && bbox.y >= 0.0
        && bbox.width > 0.0
        && bbox.height > 0.0
        && bbox.x + bbox.width <= 1.0 + EPSILON
        && bbox.y + bbox.height <= 1.0 + EPSILON
}

#[cfg(test)]
mod tests {
    use super::TantivyBm25SearchProvider;
    use crate::domain::index::{module_hit_id, SearchIndexDocument};
    use crate::domain::pdf_structure::NormalizedBbox;
    use crate::providers::search::chinese_analyzer::cjk_bigram_analyzer;
    use crate::providers::search::search_provider::SearchProvider;
    use std::fs;
    use tantivy::{doc, Index};

    fn sample_doc(
        module_id: &str,
        page_id: &str,
        title: &str,
        body: &str,
        filename: &str,
    ) -> SearchIndexDocument {
        SearchIndexDocument {
            hit_id: module_hit_id(module_id),
            page_id: page_id.to_string(),
            module_id: Some(module_id.to_string()),
            module_type: "paragraph".to_string(),
            snippet: body.to_string(),
            bbox: Some(NormalizedBbox {
                x: 0.1,
                y: 0.2,
                width: 0.3,
                height: 0.1,
            }),
            module_json: Some(format!(r#"{{"block_id":"{module_id}"}}"#)),
            document_id: "doc-1".to_string(),
            page_number: 1,
            image_path: format!("pages/doc-1/{page_id}.png"),
            original_filename: Some(filename.to_string()),
            title: Some(title.to_string()),
            summary: Some("summary".to_string()),
            visible_text: Some(body.to_string()),
            topics: vec!["topic".to_string()],
            keywords: vec!["keyword".to_string()],
            bm25_text: body.to_string(),
        }
    }

    fn temp_index_root(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("slicer-tantivy-{label}-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn chinese_query_ranks_relevant_module_higher() {
        let root = temp_index_root("module-search");
        fs::create_dir_all(&root).expect("temp dir");
        let index_path = root.join("index");
        let provider = TantivyBm25SearchProvider;
        let docs = vec![
            sample_doc(
                "block-a",
                "page-a",
                "purchase contract",
                "purchase contract terms",
                "contract.pdf",
            ),
            sample_doc(
                "block-b",
                "page-a",
                "meeting notes",
                "weekly team meeting",
                "meeting.pdf",
            ),
        ];
        provider
            .build_index(&index_path, &docs)
            .expect("build index");

        let hits = provider
            .search(&index_path, "purchase contract", 5)
            .expect("search");
        assert!(!hits.is_empty());
        assert_eq!(hits[0].hit_id, "module:block-a");
        assert_eq!(hits[0].module_id.as_deref(), Some("block-a"));
        assert_eq!(hits[0].page_id, "page-a");
        assert!(hits[0].bbox.is_some());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn two_modules_on_one_page_keep_distinct_hit_ids() {
        let root = temp_index_root("same-page");
        fs::create_dir_all(&root).expect("temp dir");
        let index_path = root.join("index");
        let provider = TantivyBm25SearchProvider;
        let docs = vec![
            sample_doc(
                "block-a",
                "page-a",
                "alpha",
                "shared phrase alpha",
                "same.pdf",
            ),
            sample_doc(
                "block-b",
                "page-a",
                "beta",
                "shared phrase beta",
                "same.pdf",
            ),
        ];
        provider
            .build_index(&index_path, &docs)
            .expect("build index");

        let hits = provider
            .search(&index_path, "shared phrase", 5)
            .expect("search");
        assert_eq!(hits.len(), 2);
        assert_ne!(hits[0].hit_id, hits[1].hit_id);
        assert!(hits.iter().all(|hit| hit.page_id == "page-a"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_page_index_remains_searchable() {
        let root = temp_index_root("legacy-page");
        fs::create_dir_all(&root).expect("temp dir");
        let index_path = root.join("index");
        fs::create_dir_all(&index_path).expect("index dir");
        let (schema, fields) = TantivyBm25SearchProvider::build_legacy_schema();
        let index = Index::create_in_dir(&index_path, schema).expect("create legacy index");
        index
            .tokenizers()
            .register("cjk_bigram", cjk_bigram_analyzer());
        let mut writer = index.writer(15_000_000).expect("writer");
        writer
            .add_document(doc!(
                fields.page_id => "legacy-page",
                fields.document_id => "legacy-document",
                fields.page_number => 3_i64,
                fields.image_path => "pages/legacy/page.png",
                fields.original_filename => "legacy.pdf",
                fields.body => "legacy searchable text"
            ))
            .expect("add legacy doc");
        writer.commit().expect("commit");

        let hits = TantivyBm25SearchProvider
            .search(&index_path, "legacy searchable", 5)
            .expect("search legacy index");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].hit_id, "page:legacy-page");
        assert!(hits[0].module_id.is_none());
        assert!(hits[0].bbox.is_none());
        assert_eq!(hits[0].document_id.as_deref(), Some("legacy-document"));
        assert_eq!(hits[0].page_number, Some(3));
        assert_eq!(hits[0].image_path.as_deref(), Some("pages/legacy/page.png"));
        assert_eq!(hits[0].original_filename.as_deref(), Some("legacy.pdf"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn duplicate_hit_ids_fail_before_index_creation() {
        let root = temp_index_root("duplicate-hit");
        fs::create_dir_all(&root).expect("temp dir");
        let index_path = root.join("index");
        let provider = TantivyBm25SearchProvider;
        let duplicate = sample_doc("block-a", "page-b", "two", "two", "same.pdf");
        let err = provider
            .build_index(
                &index_path,
                &[
                    sample_doc("block-a", "page-a", "one", "one", "same.pdf"),
                    duplicate,
                ],
            )
            .expect_err("duplicate hit id must fail");
        assert_eq!(err.code, "index_duplicate_hit_id");
        assert!(!index_path.exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_bbox_fails_before_index_creation() {
        let root = temp_index_root("invalid-bbox");
        fs::create_dir_all(&root).expect("temp dir");
        let index_path = root.join("index");
        let provider = TantivyBm25SearchProvider;
        let mut document = sample_doc("block-a", "page-a", "one", "one", "same.pdf");
        document.bbox = Some(NormalizedBbox {
            x: 0.9,
            y: 0.1,
            width: 0.2,
            height: 0.2,
        });

        let err = provider
            .build_index(&index_path, &[document])
            .expect_err("out-of-range bbox must fail");
        assert_eq!(err.code, "index_document_bbox_invalid");
        assert!(!index_path.exists());

        let _ = fs::remove_dir_all(root);
    }
}
