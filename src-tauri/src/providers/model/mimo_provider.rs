use crate::domain::settings::AppSettingsDto;
use crate::errors::{AppError, AppResult};
use crate::providers::model::provider::{
    ModelAnalysisRequest, ModelAnalysisResponse, ModelProvider,
};
use crate::services::settings_service::SettingsService;
use base64::Engine;
use serde_json::{json, Value};
use std::io::Read;
use std::time::Duration;

pub struct MimoProvider;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(90);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_PROVIDER_RESPONSE_BYTES: u64 = 1_000_000;
const MAX_COMPLETION_TOKENS: u16 = 1200;

impl MimoProvider {
    pub fn request_endpoint(settings: &AppSettingsDto) -> AppResult<String> {
        let base_url = settings.base_url.trim().trim_end_matches('/');
        let custom_endpoint = settings.custom_endpoint.trim();

        if custom_endpoint.starts_with("http://") || custom_endpoint.starts_with("https://") {
            return validate_endpoint(custom_endpoint);
        }

        if base_url.is_empty() {
            return Ok("https://token-plan-cn.xiaomimimo.com/v1/chat/completions".to_string());
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
            "model": normalized_model_name(&request.model_name),
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": image_url
                            }
                        },
                        {
                            "type": "text",
                            "text": request.prompt
                        }
                    ]
                }
            ],
            "max_completion_tokens": MAX_COMPLETION_TOKENS,
            "response_format": {
                "type": "json_object"
            }
        })
    }
}

impl ModelProvider for MimoProvider {
    fn analyze_page(&self, request: &ModelAnalysisRequest) -> AppResult<ModelAnalysisResponse> {
        let api_key =
            SettingsService::read_active_api_key_for_provider("mimo")?.ok_or_else(|| {
                AppError::new(
                    "api_key_missing",
                    "API key 未配置，无法调用 MiMo 模型。",
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
                    "response_bytes={content_length}; max_response_bytes={MAX_PROVIDER_RESPONSE_BYTES}; endpoint_kind=mimo"
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
                "MiMo provider 返回非成功状态。",
                "analysis_provider",
                true,
            )
            .with_details(format!(
                "status={}; response_bytes={}; response_preview={}; endpoint_kind=mimo",
                status.as_u16(),
                response_text.len(),
                error_preview
            )));
        }

        Ok(ModelAnalysisResponse {
            raw_json: extract_mimo_content(&response_text)?,
            provider: request.provider.clone(),
            model_name: request.model_name.clone(),
            provider_response_json: Some(response_text),
        })
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

fn normalized_model_name(model_name: &str) -> String {
    let trimmed = model_name.trim();
    if trimmed.to_ascii_lowercase().starts_with("mimo-") {
        trimmed.to_ascii_lowercase()
    } else {
        trimmed.to_string()
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
        .with_details("endpoint_kind=mimo")
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
        .with_details("scheme=http; endpoint_kind=mimo")),
        _ => Err(AppError::new(
            "model_endpoint_invalid_scheme",
            "模型 endpoint 仅支持 HTTPS，或本机调试用 HTTP。",
            "analysis_provider",
            true,
        )
        .with_details("endpoint_kind=mimo")),
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
            "response_bytes>{MAX_PROVIDER_RESPONSE_BYTES}; max_response_bytes={MAX_PROVIDER_RESPONSE_BYTES}; endpoint_kind=mimo"
        )));
    }
    String::from_utf8(bytes).map_err(|_| {
        AppError::new(
            "model_response_not_utf8",
            "模型 provider 响应不是 UTF-8 文本。",
            "analysis_provider",
            true,
        )
        .with_details("endpoint_kind=mimo")
    })
}

fn extract_mimo_content(response_text: &str) -> AppResult<String> {
    let parsed: Value = serde_json::from_str(response_text).map_err(|err| {
        AppError::new(
            "model_response_json_invalid",
            "MiMo provider 响应不是可解析 JSON。",
            "analysis_provider",
            true,
        )
        .with_details(format!(
            "summary=json parse failed at line {} column {}; response_bytes={}",
            err.line(),
            err.column(),
            response_text.len()
        ))
    })?;

    parsed
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
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
                "MiMo 响应缺少 choices[0].message.content 字段。",
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
        "MiMo provider 调用失败。",
        "analysis_provider",
        retryable,
    )
    .with_details(format!("summary={summary}; endpoint_kind=mimo"))
}

#[cfg(test)]
mod tests {
    use super::{extract_mimo_content, MimoProvider, MAX_COMPLETION_TOKENS};
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
    fn uses_official_mimo_endpoint_when_base_url_empty() {
        let settings = AppSettingsDto::default();
        assert_eq!(
            MimoProvider::request_endpoint(&settings).expect("endpoint"),
            "https://token-plan-cn.xiaomimimo.com/v1/chat/completions"
        );
    }

    #[test]
    fn accepts_mimo_v1_base_url_without_duplicate_v1() {
        let mut settings = AppSettingsDto::default();
        settings.base_url = "https://token-plan-cn.xiaomimimo.com/v1".to_string();
        assert_eq!(
            MimoProvider::request_endpoint(&settings).expect("endpoint"),
            "https://token-plan-cn.xiaomimimo.com/v1/chat/completions"
        );
    }

    #[test]
    fn mimo_request_body_uses_official_model_id_and_completion_tokens() {
        let request = ModelAnalysisRequest {
            image_bytes: Vec::new(),
            image_mime_type: "image/png".to_string(),
            prompt: "分析页面".to_string(),
            model_name: "MiMo-V2.5".to_string(),
            provider: "mimo".to_string(),
            endpoint: "https://token-plan-cn.xiaomimimo.com/v1/chat/completions".to_string(),
            expected_page: expected_page(),
        };

        let body = MimoProvider::request_body(&request, "data:image/png;base64,abc");

        assert_eq!(body["model"], "mimo-v2.5");
        assert_eq!(body["max_completion_tokens"], MAX_COMPLETION_TOKENS);
        assert!(body.get("max_tokens").is_none());
        assert_eq!(body["response_format"]["type"], "json_object");
        assert_eq!(body["messages"][0]["content"][0]["type"], "image_url");
        assert_eq!(body["messages"][0]["content"][1]["type"], "text");
    }

    #[test]
    fn extracts_content_from_mimo_response() {
        let raw =
            r#"{"choices":[{"message":{"content":"{\"schema_version\":\"page_analysis_v1\"}"}}]}"#;
        assert_eq!(
            extract_mimo_content(raw).expect("content"),
            r#"{"schema_version":"page_analysis_v1"}"#
        );
    }

    #[test]
    fn rejects_response_missing_content_field() {
        let raw = r#"{"choices":[{"message":{}}]}"#;
        let err = extract_mimo_content(raw).expect_err("missing content");
        assert_eq!(err.code, "model_response_format_invalid");
    }
}
