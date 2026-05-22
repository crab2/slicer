use crate::domain::analysis::{
    PageAnalysisContent, PageAnalysisModelInfo, PageAnalysisSource, PageAnalysisV1,
    PageRetrievalFields, ProviderResponseRecord, PAGE_ANALYSIS_SCHEMA_VERSION,
};
use crate::errors::{AppError, AppResult};
use serde::Deserialize;
use serde_json::Value;

const ANALYSIS_VALIDATION_STAGE: &str = "analysis_validation";
const MAX_ANALYSIS_TEXT_CHARS: usize = 50_000;
const MAX_TOTAL_TEXT_CHARS: usize = 250_000;
const MAX_RAW_JSON_BYTES: usize = 1_000_000;
const MAX_ARRAY_ITEMS: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedPageContext {
    pub page_id: String,
    pub document_id: String,
    pub page_number: i64,
    pub image_hash: String,
    pub image_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPageAnalysisV1 {
    schema_version: Option<String>,
    page_id: Option<String>,
    image_hash: Option<String>,
    image_path: Option<String>,
    source: Option<PageAnalysisSource>,
    analysis: Option<PageAnalysisContent>,
    retrieval: Option<RawPageRetrievalFields>,
    model: Option<PageAnalysisModelInfo>,
    provider_response: Option<ProviderResponseRecord>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPageRetrievalFields {
    bm25_text: Option<String>,
}

pub fn validate_page_analysis_v1(
    raw_json: &str,
    expected_page: &ExpectedPageContext,
) -> AppResult<PageAnalysisV1> {
    let normalized_json = extract_json_object(raw_json)?;

    if normalized_json.len() > MAX_RAW_JSON_BYTES {
        return Err(validation_error(
            "analysis_payload_too_large",
            "Model response exceeds the safe payload size limit.",
            true,
        )
        .with_details(format!(
            "path=$; limit_bytes={MAX_RAW_JSON_BYTES}; actual_bytes={}",
            normalized_json.len()
        )));
    }

    let value: Value = serde_json::from_str(&normalized_json).map_err(|err| {
        validation_error(
            "analysis_json_invalid",
            "Model response JSON could not be parsed.",
            true,
        )
        .with_details(format!(
            "path=$; summary=json parse failed at line {} column {}; bytes={}",
            err.line(),
            err.column(),
            normalized_json.len()
        ))
    })?;

    let mut total_text_chars = 0;
    validate_value_bounds(&value, "$", &mut total_text_chars)?;

    let raw: RawPageAnalysisV1 = serde_path_to_error::deserialize(value).map_err(|err| {
        validation_error(
            "analysis_field_invalid",
            "Model response fields do not match the analysis schema.",
            true,
        )
        .with_details(format!("path={}; summary={}", err.path(), err.inner()))
    })?;

    let schema_version = require_string(raw.schema_version, "schema_version")?;
    if schema_version != PAGE_ANALYSIS_SCHEMA_VERSION {
        return Err(validation_error(
            "analysis_schema_version_unsupported",
            "Model response uses an unsupported analysis schema version.",
            false,
        )
        .with_details(format!(
            "path=schema_version; expected={PAGE_ANALYSIS_SCHEMA_VERSION}; actual={schema_version}"
        )));
    }

    let page_id = require_string(raw.page_id, "page_id")?;
    if page_id != expected_page.page_id {
        return Err(validation_error(
            "analysis_page_id_mismatch",
            "Model response page identity does not match the expected page.",
            true,
        )
        .with_details(format!(
            "path=page_id; expected={}; actual={page_id}",
            expected_page.page_id
        )));
    }

    let image_hash = require_string(raw.image_hash, "image_hash")?;
    if image_hash != expected_page.image_hash {
        return Err(validation_error(
            "analysis_image_hash_mismatch",
            "Model response image identity does not match the expected page image.",
            true,
        )
        .with_details(format!(
            "path=image_hash; expected={}; actual={image_hash}",
            expected_page.image_hash
        )));
    }

    let image_path = require_string(raw.image_path, "image_path")?;
    if image_path != expected_page.image_path {
        return Err(validation_error(
            "analysis_image_path_mismatch",
            "Model response image path does not match the expected page image path.",
            true,
        )
        .with_details(format!(
            "path=image_path; expected={}; actual={image_path}",
            expected_page.image_path
        )));
    }

    let source = raw.source.ok_or_else(|| missing_field_error("source"))?;
    validate_source(&source, expected_page)?;

    let analysis = raw
        .analysis
        .ok_or_else(|| missing_field_error("analysis"))?;
    let model = raw.model.ok_or_else(|| missing_field_error("model"))?;
    let retrieval = raw
        .retrieval
        .ok_or_else(|| missing_field_error("retrieval"))?;
    let bm25_text = normalized_bm25_text(retrieval.bm25_text, &analysis)?;

    Ok(PageAnalysisV1 {
        schema_version,
        page_id,
        image_hash,
        image_path,
        source,
        analysis,
        retrieval: PageRetrievalFields { bm25_text },
        model,
        provider_response: raw.provider_response,
    })
}

fn extract_json_object(raw: &str) -> AppResult<String> {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Ok(trimmed.to_string());
    }

    if let Some(fenced) = extract_fenced_json(trimmed) {
        return Ok(fenced);
    }

    extract_balanced_json_object(trimmed).ok_or_else(|| {
        validation_error(
            "analysis_json_invalid",
            "model response did not contain a complete JSON object",
            true,
        )
        .with_details(format!(
            "path=$; summary=no complete JSON object found; bytes={}",
            raw.len()
        ))
    })
}

fn extract_fenced_json(raw: &str) -> Option<String> {
    let fence_start = raw.find("```")?;
    let after_start = &raw[fence_start + 3..];
    let content_start = after_start.find('\n').map(|idx| idx + 1).unwrap_or(0);
    let after_language = &after_start[content_start..];
    let fence_end = after_language.find("```")?;
    let fenced = after_language[..fence_end].trim();
    if fenced.starts_with('{') && fenced.ends_with('}') {
        Some(fenced.to_string())
    } else {
        None
    }
}

fn extract_balanced_json_object(raw: &str) -> Option<String> {
    let mut start = None;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (idx, ch) in raw.char_indices() {
        if start.is_none() {
            if ch == '{' {
                start = Some(idx);
                depth = 1;
            }
            continue;
        }

        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let begin = start.expect("start exists when depth is positive");
                    return Some(raw[begin..=idx].trim().to_string());
                }
            }
            _ => {}
        }
    }

    None
}

fn validate_source(
    source: &PageAnalysisSource,
    expected_page: &ExpectedPageContext,
) -> AppResult<()> {
    if source.document_id != expected_page.document_id {
        return Err(validation_error(
            "analysis_document_id_mismatch",
            "Model response document identity does not match the expected document.",
            true,
        )
        .with_details(format!(
            "path=source.document_id; expected={}; actual={}",
            expected_page.document_id, source.document_id
        )));
    }

    if source.page_number != expected_page.page_number {
        return Err(validation_error(
            "analysis_page_number_mismatch",
            "Model response page number does not match the expected page.",
            true,
        )
        .with_details(format!(
            "path=source.page_number; expected={}; actual={}",
            expected_page.page_number, source.page_number
        )));
    }

    Ok(())
}

fn normalized_bm25_text(
    provided_text: Option<String>,
    analysis: &PageAnalysisContent,
) -> AppResult<String> {
    if let Some(text) = provided_text {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    let mut parts = Vec::new();
    push_optional_text(&mut parts, analysis.title.as_deref());
    push_optional_text(&mut parts, analysis.summary.as_deref());
    push_optional_text(&mut parts, analysis.visible_text.as_deref());
    push_text_list(&mut parts, &analysis.topics);
    push_text_list(&mut parts, &analysis.keywords);

    if parts.is_empty() {
        return Err(validation_error(
            "analysis_retrieval_text_missing",
            "Model response lacks retrievable page text.",
            true,
        )
        .with_details("path=retrieval.bm25_text; summary=no analysis text available"));
    }

    Ok(parts.join("\n"))
}

fn push_optional_text(parts: &mut Vec<String>, value: Option<&str>) {
    if let Some(value) = value {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed.to_string());
        }
    }
}

fn push_text_list(parts: &mut Vec<String>, values: &[String]) {
    for value in values {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed.to_string());
        }
    }
}

fn validate_value_bounds(value: &Value, path: &str, total_text_chars: &mut usize) -> AppResult<()> {
    match value {
        Value::String(text) if text.chars().count() > MAX_ANALYSIS_TEXT_CHARS => {
            Err(validation_error(
                "analysis_text_too_long",
                "Model response text field exceeds the safe length limit.",
                true,
            )
            .with_details(format!(
                "path={path}; limit_chars={MAX_ANALYSIS_TEXT_CHARS}; actual_chars={}",
                text.chars().count()
            )))
        }
        Value::String(text) => {
            *total_text_chars += text.chars().count();
            if *total_text_chars > MAX_TOTAL_TEXT_CHARS {
                return Err(validation_error(
                    "analysis_text_too_long",
                    "Model response total text exceeds the safe length limit.",
                    true,
                )
                .with_details(format!(
                    "path={path}; limit_total_chars={MAX_TOTAL_TEXT_CHARS}; actual_total_chars={}",
                    *total_text_chars
                )));
            }
            Ok(())
        }
        Value::Array(items) => {
            if items.len() > MAX_ARRAY_ITEMS {
                return Err(validation_error(
                    "analysis_array_too_large",
                    "Model response array field exceeds the safe item limit.",
                    true,
                )
                .with_details(format!(
                    "path={path}; limit_items={MAX_ARRAY_ITEMS}; actual_items={}",
                    items.len()
                )));
            }
            for (index, item) in items.iter().enumerate() {
                validate_value_bounds(item, &format!("{path}[{index}]"), total_text_chars)?;
            }
            Ok(())
        }
        Value::Object(map) => {
            for (key, child) in map {
                validate_value_bounds(child, &format!("{path}.{key}"), total_text_chars)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn require_string(value: Option<String>, path: &str) -> AppResult<String> {
    value.ok_or_else(|| missing_field_error(path))
}

fn missing_field_error(path: &str) -> AppError {
    validation_error(
        "analysis_field_missing",
        "Model response is missing a required page_analysis_v1 field.",
        true,
    )
    .with_details(format!("path={path}; summary=required field missing"))
}

fn validation_error(code: &str, message: &str, retryable: bool) -> AppError {
    AppError::new(code, message, ANALYSIS_VALIDATION_STAGE, retryable)
}

#[cfg(test)]
mod tests {
    use super::{validate_page_analysis_v1, ExpectedPageContext};
    use crate::domain::analysis::PAGE_ANALYSIS_SCHEMA_VERSION;

    fn expected_page() -> ExpectedPageContext {
        ExpectedPageContext {
            page_id: "doc-1_1".to_string(),
            document_id: "doc-1".to_string(),
            page_number: 1,
            image_hash: "hash-1".to_string(),
            image_path: "pages/doc-1/hash-1.png".to_string(),
        }
    }

    fn valid_json_without_bm25() -> String {
        format!(
            r#"{{
  "schema_version": "{PAGE_ANALYSIS_SCHEMA_VERSION}",
  "page_id": "doc-1_1",
  "image_hash": "hash-1",
  "image_path": "pages/doc-1/hash-1.png",
  "source": {{
    "document_id": "doc-1",
    "page_number": 1,
    "original_filename": "sample.pdf"
  }},
  "analysis": {{
    "title": "Title",
    "summary": "Summary",
    "visible_text": "Body",
    "topics": ["Topic A", "Topic B"],
    "keywords": ["Keyword A"]
  }},
  "retrieval": {{}},
  "model": {{
    "provider": "custom",
    "model_name": "configured-model"
  }}
}}"#
        )
    }

    fn valid_json_without_retrieval() -> String {
        format!(
            r#"{{
  "schema_version": "{PAGE_ANALYSIS_SCHEMA_VERSION}",
  "page_id": "doc-1_1",
  "image_hash": "hash-1",
  "image_path": "pages/doc-1/hash-1.png",
  "source": {{
    "document_id": "doc-1",
    "page_number": 1,
    "original_filename": "sample.pdf"
  }},
  "analysis": {{
    "title": "Title",
    "summary": "Summary",
    "visible_text": "Body",
    "topics": ["Topic A", "Topic B"],
    "keywords": ["Keyword A"]
  }},
  "model": {{
    "provider": "custom",
    "model_name": "configured-model"
  }}
}}"#
        )
    }

    #[test]
    fn validates_valid_fixture_round_trip() {
        let raw = include_str!("../../../fixtures/sample_analysis/valid_page_analysis_v1.json");
        let analysis =
            validate_page_analysis_v1(raw, &expected_page()).expect("valid fixture should pass");

        let serialized =
            serde_json::to_string(&analysis).expect("normalized analysis should serialize");
        let reparsed = validate_page_analysis_v1(&serialized, &expected_page())
            .expect("serialized normalized analysis should pass");

        assert_eq!(reparsed, analysis);
    }

    #[test]
    fn validates_valid_json_and_fills_bm25_text() {
        let analysis = validate_page_analysis_v1(&valid_json_without_bm25(), &expected_page())
            .expect("valid analysis should pass");

        assert_eq!(analysis.schema_version, PAGE_ANALYSIS_SCHEMA_VERSION);
        assert_eq!(analysis.page_id, "doc-1_1");
        assert_eq!(analysis.image_hash, "hash-1");
        assert_eq!(
            analysis.retrieval.bm25_text,
            "Title\nSummary\nBody\nTopic A\nTopic B\nKeyword A"
        );
    }

    #[test]
    fn extracts_json_from_markdown_fence() {
        let raw = format!("```json\n{}\n```", valid_json_without_bm25());
        let analysis =
            validate_page_analysis_v1(&raw, &expected_page()).expect("fenced json should pass");

        assert_eq!(analysis.page_id, "doc-1_1");
    }

    #[test]
    fn extracts_first_balanced_json_object_from_model_prose() {
        let raw = format!("Here is the JSON:\n{}\nDone.", valid_json_without_bm25());
        let analysis =
            validate_page_analysis_v1(&raw, &expected_page()).expect("wrapped json should pass");

        assert_eq!(analysis.image_hash, "hash-1");
    }

    #[test]
    fn rejects_page_id_mismatch_without_using_image_hash_as_identity() {
        let raw = include_str!("../../../fixtures/sample_analysis/page_id_mismatch.json");
        let err =
            validate_page_analysis_v1(&raw, &expected_page()).expect_err("page_id must mismatch");

        assert_eq!(err.code, "analysis_page_id_mismatch");
        assert_eq!(err.stage, "analysis_validation");
        assert!(err.details.unwrap().contains("page_id"));
    }

    #[test]
    fn rejects_image_hash_mismatch_independently() {
        let raw = include_str!("../../../fixtures/sample_analysis/image_hash_mismatch.json");
        let err = validate_page_analysis_v1(&raw, &expected_page())
            .expect_err("image_hash must mismatch");

        assert_eq!(err.code, "analysis_image_hash_mismatch");
        assert_eq!(err.stage, "analysis_validation");
        assert!(err.details.unwrap().contains("image_hash"));
    }

    #[test]
    fn reports_unsupported_schema_version() {
        let raw = include_str!("../../../fixtures/sample_analysis/unknown_schema_version.json");
        let err =
            validate_page_analysis_v1(&raw, &expected_page()).expect_err("version should fail");

        assert_eq!(err.code, "analysis_schema_version_unsupported");
        assert_eq!(err.stage, "analysis_validation");
    }

    #[test]
    fn reports_invalid_json_with_safe_redacted_details() {
        let raw = include_str!("../../../fixtures/sample_analysis/invalid_json.json");
        let err = validate_page_analysis_v1(raw, &expected_page()).expect_err("json should fail");

        assert_eq!(err.code, "analysis_json_invalid");
        assert_eq!(err.stage, "analysis_validation");
        let details = err.details.unwrap();
        assert!(details.contains("path=$"));
        assert!(details.contains("bytes="));
        assert!(!details.contains("raw="));
        assert!(!details.contains("sk-secret"));
    }

    #[test]
    fn reports_missing_schema_version() {
        let raw = include_str!("../../../fixtures/sample_analysis/missing_schema_version.json");
        let err =
            validate_page_analysis_v1(raw, &expected_page()).expect_err("schema version required");

        assert_eq!(err.code, "analysis_field_missing");
        assert_eq!(err.stage, "analysis_validation");
        assert!(err.details.unwrap().contains("schema_version"));
    }

    #[test]
    fn reports_wrong_field_type() {
        let raw = include_str!("../../../fixtures/sample_analysis/wrong_field_type.json");
        let err = validate_page_analysis_v1(raw, &expected_page()).expect_err("type should fail");

        assert_eq!(err.code, "analysis_field_invalid");
        assert_eq!(err.stage, "analysis_validation");
        let details = err.details.unwrap();
        assert!(details.contains("source.page_number"));
        assert!(!details.contains("sample.pdf"));
    }

    #[test]
    fn rejects_missing_retrieval_object_before_fallback() {
        let err = validate_page_analysis_v1(&valid_json_without_retrieval(), &expected_page())
            .expect_err("retrieval object is required");

        assert_eq!(err.code, "analysis_field_missing");
        assert_eq!(err.stage, "analysis_validation");
        assert!(err.details.unwrap().contains("retrieval"));
    }

    #[test]
    fn rejects_unknown_fields_in_valid_json() {
        let raw = valid_json_without_bm25().replace(
            "\"schema_version\"",
            "\"unexpected_field\": \"not allowed\", \"schema_version\"",
        );
        let err = validate_page_analysis_v1(&raw, &expected_page())
            .expect_err("unknown fields should be rejected");

        assert_eq!(err.code, "analysis_field_invalid");
        assert_eq!(err.stage, "analysis_validation");
        assert!(err.details.unwrap().contains("unexpected_field"));
    }

    #[test]
    fn rejects_large_arrays() {
        let topics = (0..513)
            .map(|index| format!("\"topic-{index}\""))
            .collect::<Vec<_>>()
            .join(",");
        let raw = valid_json_without_bm25().replace("\"Topic A\", \"Topic B\"", &topics);
        let err =
            validate_page_analysis_v1(&raw, &expected_page()).expect_err("array should be too big");

        assert_eq!(err.code, "analysis_array_too_large");
        assert_eq!(err.stage, "analysis_validation");
        assert!(err.details.unwrap().contains("analysis.topics"));
    }

    #[test]
    fn rejects_overlong_text_with_safe_summary() {
        let raw =
            valid_json_without_bm25().replace("\"Body\"", &format!("\"{}\"", "a".repeat(50_001)));
        let err =
            validate_page_analysis_v1(&raw, &expected_page()).expect_err("text should be too long");

        assert_eq!(err.code, "analysis_text_too_long");
        assert_eq!(err.stage, "analysis_validation");
        let details = err.details.unwrap();
        assert!(details.contains("analysis.visible_text"));
        assert!(details.len() < 200);
    }
}
