use crate::domain::settings::AppSettingsDto;
use crate::domain::settings::{ModelInfoDto, ModelListDto};
use crate::errors::{AppError, AppResult};
use crate::providers::model::provider::{
    ModelAnalysisRequest, ModelAnalysisResponse, ModelProvider,
};
use crate::services::settings_service::SettingsService;
use base64::Engine;
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Read;
use std::time::Duration;

pub struct OpenAIProvider;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(90);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_PROVIDER_RESPONSE_BYTES: u64 = 1_000_000;
const MAX_COMPLETION_TOKENS: u16 = 1200;

impl OpenAIProvider {
    pub fn request_endpoint(settings: &AppSettingsDto) -> AppResult<String> {
        let base_url = settings.base_url.trim().trim_end_matches('/');
        let custom_endpoint = settings.custom_endpoint.trim();

        if custom_endpoint.starts_with("http://") || custom_endpoint.starts_with("https://") {
            return validate_endpoint(custom_endpoint);
        }

        if base_url.is_empty() {
            return Ok("https://api.openai.com/v1/chat/completions".to_string());
        }

        if custom_endpoint.is_empty() {
            validate_endpoint(&default_chat_completions_endpoint(base_url))
        } else {
            validate_endpoint(&format!(
                "{base_url}/{}",
                custom_endpoint.trim_start_matches('/')
            ))
        }
    }

    fn request_body(request: &ModelAnalysisRequest, image_url: &str) -> Value {
        json!({
            "model": request.model_name,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": request.prompt
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": image_url
                            }
                        }
                    ]
                }
            ],
            "max_tokens": MAX_COMPLETION_TOKENS,
            "response_format": {
                "type": "json_object"
            }
        })
    }

    pub fn models_endpoint(settings: &AppSettingsDto) -> AppResult<String> {
        let base_url = settings.base_url.trim().trim_end_matches('/');
        let custom_endpoint = settings.custom_endpoint.trim();

        if custom_endpoint.starts_with("http://") || custom_endpoint.starts_with("https://") {
            return validate_endpoint(&default_models_endpoint(
                custom_endpoint.trim_end_matches('/'),
            ));
        }

        if base_url.is_empty() {
            return Ok("https://api.openai.com/v1/models".to_string());
        }

        if custom_endpoint.is_empty() {
            validate_endpoint(&default_models_endpoint(base_url))
        } else {
            validate_endpoint(&default_models_endpoint(&format!(
                "{base_url}/{}",
                custom_endpoint.trim_start_matches('/')
            )))
        }
    }

    pub fn list_models_with_api_key(
        settings: &AppSettingsDto,
        api_key: &str,
    ) -> AppResult<ModelListDto> {
        let endpoint = Self::models_endpoint(settings)?;
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| provider_error("model_client_build_failed", true, "client_build_failed"))?;

        let mut response = client
            .get(endpoint)
            .bearer_auth(api_key)
            .header(ACCEPT, "application/json")
            .header(CONTENT_TYPE, "application/json")
            .send()
            .map_err(|_| provider_error("model_list_request_failed", true, "request_failed"))?;

        let status = response.status();
        if let Some(content_length) = response.content_length() {
            if content_length > MAX_PROVIDER_RESPONSE_BYTES {
                return Err(provider_error(
                    "model_response_too_large",
                    true,
                    "content_length_exceeded",
                ));
            }
        }
        let response_text = read_limited_response(&mut response)?;

        if !status.is_success() {
            let error_preview = response_preview(&response_text);
            let status_code = status.as_u16();
            let key_fingerprint = api_key_fingerprint(api_key);
            return Err(AppError::new(
                "model_list_http_status_failed",
                format!("OpenAI 模型列表返回非成功状态（HTTP {status_code}）。"),
                "settings",
                true,
            )
            .with_details(format!(
                "status={}; response_bytes={}; response_preview={}; key_fingerprint={}; endpoint={}; endpoint_kind=openai_models",
                status_code,
                response_text.len(),
                error_preview,
                key_fingerprint,
                Self::models_endpoint(settings)?
            )));
        }

        parse_model_list_response(&response_text)
    }
}

impl ModelProvider for OpenAIProvider {
    fn analyze_page(&self, request: &ModelAnalysisRequest) -> AppResult<ModelAnalysisResponse> {
        let api_key =
            SettingsService::read_active_api_key_for_provider("openai")?.ok_or_else(|| {
                AppError::new(
                    "api_key_missing",
                    "API key 未配置，无法调用 OpenAI 模型。",
                    "analysis_provider",
                    true,
                )
            })?;

        let image_base64 = base64::engine::general_purpose::STANDARD.encode(&request.image_bytes);
        let image_url = format!("data:{};base64,{}", request.image_mime_type, image_base64);

        let body = Self::request_body(request, &image_url);

        let client = reqwest::blocking::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|_| {
                provider_error("model_client_build_failed", true, "client_build_failed")
            })?;

        let mut response = client
            .post(&request.endpoint)
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .map_err(|_| provider_error("model_request_failed", true, "request_failed"))?;

        let status = response.status();
        if let Some(content_length) = response.content_length() {
            if content_length > MAX_PROVIDER_RESPONSE_BYTES {
                return Err(provider_error(
                    "model_response_too_large",
                    true,
                    "content_length_exceeded",
                )
                .with_details(format!(
                    "response_bytes={content_length}; max_response_bytes={MAX_PROVIDER_RESPONSE_BYTES}; endpoint_kind=openai"
                )));
            }
        }
        let response_text = read_limited_response(&mut response)?;

        if !status.is_success() {
            let error_preview = if response_text.len() > 500 {
                format!("{}...", &response_text[..500])
            } else {
                response_text.clone()
            };
            return Err(AppError::new(
                "model_http_status_failed",
                "OpenAI provider 返回非成功状态。",
                "analysis_provider",
                true,
            )
            .with_details(format!(
                "status={}; response_bytes={}; response_preview={}; endpoint_kind=openai",
                status.as_u16(),
                response_text.len(),
                error_preview
            )));
        }

        Ok(ModelAnalysisResponse {
            raw_json: extract_openai_content(&response_text)?,
            provider: "openai".to_string(),
            model_name: request.model_name.clone(),
            provider_response_json: Some(response_text),
        })
    }
}

fn validate_endpoint(endpoint: &str) -> AppResult<String> {
    let url = reqwest::Url::parse(endpoint).map_err(|_| {
        AppError::new(
            "model_endpoint_invalid",
            "模型 endpoint 必须是有效 URL。",
            "analysis_provider",
            true,
        )
        .with_details("endpoint_kind=openai")
    })?;

    match url.scheme() {
        "https" => Ok(endpoint.to_string()),
        "http" if is_loopback_host(url.host_str()) => Ok(endpoint.to_string()),
        "http" => Err(AppError::new(
            "model_endpoint_insecure",
            "远程模型 endpoint 必须使用 HTTPS。",
            "analysis_provider",
            true,
        )
        .with_details("scheme=http; endpoint_kind=openai")),
        _ => Err(AppError::new(
            "model_endpoint_invalid_scheme",
            "模型 endpoint 仅支持 HTTPS，或本机调试用 HTTP。",
            "analysis_provider",
            true,
        )
        .with_details("endpoint_kind=openai")),
    }
}

fn default_chat_completions_endpoint(base_url: &str) -> String {
    let lower = base_url.to_ascii_lowercase();
    if lower.ends_with("/chat/completions") {
        base_url.to_string()
    } else if lower.ends_with("/v1") {
        format!("{base_url}/chat/completions")
    } else {
        format!("{base_url}/v1/chat/completions")
    }
}

fn is_loopback_host(host: Option<&str>) -> bool {
    matches!(host, Some("localhost") | Some("127.0.0.1") | Some("::1"))
}

fn read_limited_response(response: &mut reqwest::blocking::Response) -> AppResult<String> {
    let mut limited = response.take(MAX_PROVIDER_RESPONSE_BYTES + 1);
    let mut bytes = Vec::new();
    limited
        .read_to_end(&mut bytes)
        .map_err(|_| provider_error("model_response_read_failed", true, "response_read_failed"))?;
    if bytes.len() as u64 > MAX_PROVIDER_RESPONSE_BYTES {
        return Err(provider_error(
            "model_response_too_large",
            true,
            "response_limit_exceeded",
        )
        .with_details(format!(
            "response_bytes>{MAX_PROVIDER_RESPONSE_BYTES}; max_response_bytes={MAX_PROVIDER_RESPONSE_BYTES}; endpoint_kind=openai"
        )));
    }
    String::from_utf8(bytes).map_err(|_| {
        AppError::new(
            "model_response_not_utf8",
            "模型 provider 响应不是 UTF-8 文本。",
            "analysis_provider",
            true,
        )
        .with_details("endpoint_kind=openai")
    })
}

fn default_models_endpoint(base_url: &str) -> String {
    let lower = base_url.to_ascii_lowercase();
    if lower.ends_with("/models") {
        base_url.to_string()
    } else if lower.ends_with("/chat/completions") {
        let suffix_len = "/chat/completions".len();
        format!("{}{}", &base_url[..base_url.len() - suffix_len], "/models")
    } else if lower.ends_with("/v1") {
        format!("{base_url}/models")
    } else {
        format!("{base_url}/v1/models")
    }
}

#[derive(Debug, Deserialize)]
struct OpenAIModelListResponse {
    data: Vec<OpenAIModelRecord>,
}

#[derive(Debug, Deserialize)]
struct OpenAIModelRecord {
    id: String,
    display_name: Option<String>,
    owned_by: Option<String>,
}

fn parse_model_list_response(response_text: &str) -> AppResult<ModelListDto> {
    let parsed: OpenAIModelListResponse = serde_json::from_str(response_text).map_err(|err| {
        AppError::new(
            "model_list_response_json_invalid",
            "OpenAI 模型列表响应不是可解析 JSON。",
            "settings",
            true,
        )
        .with_details(format!(
            "summary=json parse failed at line {} column {}; response_bytes={}; endpoint_kind=openai_models",
            err.line(),
            err.column(),
            response_text.len()
        ))
    })?;

    let mut models: Vec<ModelInfoDto> = parsed
        .data
        .into_iter()
        .filter(|model| !model.id.trim().is_empty())
        .map(|model| ModelInfoDto {
            id: model.id,
            display_name: model.display_name,
            owned_by: model.owned_by,
        })
        .collect();
    models.sort_by(|a, b| a.id.cmp(&b.id));
    models.dedup_by(|a, b| a.id == b.id);

    Ok(ModelListDto {
        provider: "openai".to_string(),
        models,
    })
}

fn response_preview(response_text: &str) -> String {
    response_text.chars().take(500).collect()
}

fn api_key_fingerprint(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 8 {
        return format!("len={}", chars.len());
    }
    let prefix: String = chars.iter().take(4).collect();
    let suffix: String = chars.iter().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    format!("len={}; prefix={prefix}; suffix={suffix}", chars.len())
}

fn extract_openai_content(response_text: &str) -> AppResult<String> {
    let parsed: Value = serde_json::from_str(response_text).map_err(|err| {
        let preview = if response_text.len() > 500 {
            format!("{}...", &response_text[..500])
        } else {
            response_text.to_string()
        };
        AppError::new(
            "model_response_json_invalid",
            "OpenAI provider 响应不是可解析 JSON。",
            "analysis_provider",
            true,
        )
        .with_details(format!(
            "summary=json parse failed at line {} column {}; response_bytes={}; response_preview={}",
            err.line(),
            err.column(),
            response_text.len(),
            preview
        ))
    })?;

    parsed
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|msg| msg.get("content"))
        .and_then(|content| content.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            let preview = if response_text.len() > 500 {
                format!("{}...", &response_text[..500])
            } else {
                response_text.to_string()
            };
            AppError::new(
                "model_response_format_invalid",
                "OpenAI 响应缺少 choices[0].message.content 字段。",
                "analysis_provider",
                true,
            )
            .with_details(format!(
                "response_bytes={}; response_preview={}",
                response_text.len(),
                preview
            ))
        })
}

fn provider_error(code: &str, retryable: bool, summary: &str) -> AppError {
    AppError::new(
        code,
        "OpenAI provider 调用失败。",
        "analysis_provider",
        retryable,
    )
    .with_details(format!("summary={summary}; endpoint_kind=openai"))
}

#[cfg(test)]
mod tests {
    use super::{
        api_key_fingerprint, extract_openai_content, parse_model_list_response, OpenAIProvider,
        MAX_COMPLETION_TOKENS,
    };
    use crate::domain::settings::AppSettingsDto;
    use crate::providers::model::provider::ModelAnalysisRequest;
    use crate::providers::model::schema_validator::ExpectedPageContext;

    fn expected_page() -> ExpectedPageContext {
        ExpectedPageContext {
            page_id: "doc-1_1".to_string(),
            document_id: "doc-1".to_string(),
            page_number: 1,
            image_hash: "hash-1".to_string(),
            image_path: "pages/doc-1/hash-1.png".to_string(),
        }
    }

    #[test]
    fn uses_official_openai_endpoint_when_base_url_empty() {
        let settings = AppSettingsDto::default();
        assert_eq!(
            OpenAIProvider::request_endpoint(&settings).expect("endpoint"),
            "https://api.openai.com/v1/chat/completions"
        );
    }

    #[test]
    fn combines_base_url_and_default_path() {
        let mut settings = AppSettingsDto::default();
        settings.base_url = "https://custom.openai.azure.com".to_string();
        assert_eq!(
            OpenAIProvider::request_endpoint(&settings).expect("endpoint"),
            "https://custom.openai.azure.com/v1/chat/completions"
        );
    }

    #[test]
    fn combines_base_url_for_models_endpoint() {
        let mut settings = AppSettingsDto::default();
        settings.base_url = "https://custom.openai.azure.com".to_string();
        assert_eq!(
            OpenAIProvider::models_endpoint(&settings).expect("models endpoint"),
            "https://custom.openai.azure.com/v1/models"
        );

        settings.base_url = "https://proxy.example.com/v1".to_string();
        assert_eq!(
            OpenAIProvider::models_endpoint(&settings).expect("models endpoint"),
            "https://proxy.example.com/v1/models"
        );

        settings.base_url = "https://www.su8.codes/codex/v1".to_string();
        assert_eq!(
            OpenAIProvider::models_endpoint(&settings).expect("models endpoint"),
            "https://www.su8.codes/codex/v1/models"
        );
    }

    #[test]
    fn custom_chat_endpoint_preserves_proxy_prefix_for_models_endpoint() {
        let mut settings = AppSettingsDto::default();
        settings.custom_endpoint = "https://proxy.example.com/codex/v1/chat/completions".to_string();

        assert_eq!(
            OpenAIProvider::models_endpoint(&settings).expect("models endpoint"),
            "https://proxy.example.com/codex/v1/models"
        );
    }

    #[test]
    fn relative_custom_chat_endpoint_preserves_proxy_prefix_for_models_endpoint() {
        let mut settings = AppSettingsDto::default();
        settings.base_url = "https://www.su8.codes".to_string();
        settings.custom_endpoint = "/codex/v1/chat/completions".to_string();

        assert_eq!(
            OpenAIProvider::models_endpoint(&settings).expect("models endpoint"),
            "https://www.su8.codes/codex/v1/models"
        );
    }

    #[test]
    fn openai_request_body_uses_standard_chat_completions_shape() {
        let request = ModelAnalysisRequest {
            image_bytes: Vec::new(),
            image_mime_type: "image/png".to_string(),
            prompt: "分析页面".to_string(),
            model_name: "Custom-Vision-Model".to_string(),
            provider: "openai".to_string(),
            endpoint: "https://models.example.com/v1/chat/completions".to_string(),
            expected_page: expected_page(),
        };

        let body = OpenAIProvider::request_body(&request, "data:image/png;base64,abc");

        assert_eq!(body["model"], "Custom-Vision-Model");
        assert_eq!(body["max_tokens"], MAX_COMPLETION_TOKENS);
        assert!(body.get("max_completion_tokens").is_none());
        assert!(body.get("thinking").is_none());
        assert_eq!(body["response_format"]["type"], "json_object");
        assert_eq!(body["messages"][0]["content"][0]["type"], "text");
        assert_eq!(body["messages"][0]["content"][1]["type"], "image_url");
    }

    #[test]
    fn extracts_content_from_openai_response() {
        let raw =
            r#"{"choices":[{"message":{"content":"{\"schema_version\":\"page_analysis_v1\"}"}}]}"#;
        assert_eq!(
            extract_openai_content(raw).expect("content"),
            r#"{"schema_version":"page_analysis_v1"}"#
        );
    }

    #[test]
    fn parses_and_sorts_model_list_response() {
        let raw = r#"{
            "object": "list",
            "data": [
                {"id": "gpt-5.5", "object": "model", "display_name": "GPT-5.5", "owned_by": "openai"},
                {"id": "gpt-4.1-mini", "object": "model", "display_name": "GPT-4.1 Mini", "owned_by": "openai"},
                {"id": "gpt-5.5", "object": "model", "owned_by": "openai"}
            ]
        }"#;

        let list = parse_model_list_response(raw).expect("model list");

        assert_eq!(list.provider, "openai");
        assert_eq!(
            list.models.iter().map(|model| model.id.as_str()).collect::<Vec<_>>(),
            vec!["gpt-4.1-mini", "gpt-5.5"]
        );
        assert_eq!(list.models[0].display_name.as_deref(), Some("GPT-4.1 Mini"));
    }

    #[test]
    fn api_key_fingerprint_does_not_expose_full_secret() {
        assert_eq!(api_key_fingerprint("sk-1234567890abcd"), "len=17; prefix=sk-1; suffix=abcd");
        assert_eq!(api_key_fingerprint("short"), "len=5");
    }

    #[test]
    fn rejects_response_missing_content_field() {
        let raw = r#"{"choices":[{"message":{}}]}"#;
        let err = extract_openai_content(raw).expect_err("missing content");
        assert_eq!(err.code, "model_response_format_invalid");
    }
}
