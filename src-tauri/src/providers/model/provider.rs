use crate::providers::model::schema_validator::ExpectedPageContext;

#[derive(Debug, Clone)]
pub struct ModelAnalysisRequest {
    pub image_bytes: Vec<u8>,
    pub image_mime_type: String,
    pub prompt: String,
    pub model_name: String,
    pub provider: String,
    pub endpoint: String,
    pub expected_page: ExpectedPageContext,
}

#[derive(Debug, Clone)]
pub struct ModelAnalysisResponse {
    pub raw_json: String,
    pub provider: String,
    pub model_name: String,
    pub provider_response_json: Option<String>,
}

pub trait ModelProvider: Send + Sync {
    fn analyze_page(
        &self,
        request: &ModelAnalysisRequest,
    ) -> crate::errors::AppResult<ModelAnalysisResponse>;
}
