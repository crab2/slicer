use crate::domain::settings::AppSettingsDto;
use crate::errors::{AppError, AppResult};
use crate::providers::model::provider::{
    ModelAnalysisRequest, ModelAnalysisResponse, ModelProvider,
};
use crate::security;
use base64::Engine;
use serde_json::{json, Value};
use std::io::Read;
use std::time::Duration;

pub struct OpenAIProvider;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(90);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_PROVIDER_RESPONSE_BYTES: u64 = 1_000_000;

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
            validate_endpoint(&format!("{base_url}/v1/chat/completions"))
        } else {
            validate_endpoint(&format!(
                "{base_url}/{}",
                custom_endpoint.trim_start_matches('/')
            ))
        }
    }
}

impl ModelProvider for OpenAIProvider {
    fn analyze_page(&self, request: &ModelAnalysisRequest) -> AppResult<ModelAnalysisResponse> {
        let api_key = security::read_api_key()?.ok_or_else(|| {
            AppError::new(
                "api_key_missing",
                "API key 未配置，无法调用 OpenAI 模型。",
                "analysis_provider",
                true,
            )
        })?;

        let image_base64 = base64::engine::general_purpose::STANDARD.encode(&request.image_bytes);
        let image_url = format!("data:{};base64,{}", request.image_mime_type, image_base64);

        let body = json!({
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
            "max_tokens": 4096
        });

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
            provider: request.provider.clone(),
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
    use super::{extract_openai_content, OpenAIProvider};
    use crate::domain::settings::AppSettingsDto;

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
    fn extracts_content_from_openai_response() {
        let raw =
            r#"{"choices":[{"message":{"content":"{\"schema_version\":\"page_analysis_v1\"}"}}]}"#;
        assert_eq!(
            extract_openai_content(raw).expect("content"),
            r#"{"schema_version":"page_analysis_v1"}"#
        );
    }

    #[test]
    fn rejects_response_missing_content_field() {
        let raw = r#"{"choices":[{"message":{}}]}"#;
        let err = extract_openai_content(raw).expect_err("missing content");
        assert_eq!(err.code, "model_response_format_invalid");
    }
}
