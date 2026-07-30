use crate::domain::analysis::PAGE_ANALYSIS_SCHEMA_VERSION;
use crate::domain::pdf_structure::VISUAL_MODULE_ANALYSIS_SCHEMA_VERSION;
use crate::errors::{AppError, AppResult};
use crate::providers::model::provider::{
    ModelAnalysisRequest, ModelAnalysisResponse, ModelProvider,
};

pub struct MockModelProvider;

impl ModelProvider for MockModelProvider {
    fn analyze_page(&self, request: &ModelAnalysisRequest) -> AppResult<ModelAnalysisResponse> {
        if request
            .prompt
            .contains(VISUAL_MODULE_ANALYSIS_SCHEMA_VERSION)
        {
            let marker = "\"block_id\": \"";
            let block_id = request
                .prompt
                .split_once(marker)
                .and_then(|(_, rest)| rest.split_once('"'))
                .map(|(block_id, _)| block_id)
                .ok_or_else(|| {
                    AppError::new(
                        "mock_visual_block_id_missing",
                        "visual-module prompt does not contain a block id",
                        "analysis_provider",
                        false,
                    )
                })?;
            let raw_json = serde_json::json!({
                "schema_version": VISUAL_MODULE_ANALYSIS_SCHEMA_VERSION,
                "block_id": block_id,
                "description": "Deterministic mock visual-module description.",
                "visible_text": "mock visual module text",
                "keywords": ["mock", "visual", "module"],
                "model": {
                    "provider": request.provider,
                    "model_name": request.model_name
                }
            })
            .to_string();
            return Ok(ModelAnalysisResponse {
                raw_json,
                provider: request.provider.clone(),
                model_name: request.model_name.clone(),
                provider_response_json: None,
            });
        }

        let expected = &request.expected_page;
        let raw_json = serde_json::json!({
            "schema_version": PAGE_ANALYSIS_SCHEMA_VERSION,
            "page_id": expected.page_id,
            "image_hash": expected.image_hash,
            "image_path": expected.image_path,
            "source": {
                "document_id": expected.document_id,
                "page_number": expected.page_number,
                "original_filename": null
            },
            "analysis": {
                "title": format!("第 {} 页", expected.page_number),
                "summary": "本地 mock provider 生成的确定性页面分析摘要。",
                "visible_text": "mock analysis text",
                "topics": ["mock", "analysis"],
                "keywords": ["mock"]
            },
            "retrieval": {
                "bm25_text": "mock analysis text"
            },
            "model": {
                "provider": request.provider,
                "model_name": request.model_name
            }
        })
        .to_string();

        Ok(ModelAnalysisResponse {
            raw_json,
            provider: request.provider.clone(),
            model_name: request.model_name.clone(),
            provider_response_json: None,
        })
    }
}
