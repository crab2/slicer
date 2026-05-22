use crate::domain::analysis::PAGE_ANALYSIS_SCHEMA_VERSION;
use crate::errors::AppResult;
use crate::providers::model::provider::{
    ModelAnalysisRequest, ModelAnalysisResponse, ModelProvider,
};

pub struct MockModelProvider;

impl ModelProvider for MockModelProvider {
    fn analyze_page(&self, request: &ModelAnalysisRequest) -> AppResult<ModelAnalysisResponse> {
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
