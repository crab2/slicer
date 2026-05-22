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

pub struct SiliconFlowProvider;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(90);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_PROVIDER_RESPONSE_BYTES: u64 = 1_000_000;
const SILICONFLOW_TEMPERATURE: f64 = 0.7;
const SILICONFLOW_MAX_TOKENS: u16 = 1000;

impl SiliconFlowProvider {
    pub fn request_endpoint(settings: &AppSettingsDto) -> AppResult<String> {
        let base_url = settings.base_url.trim().trim_end_matches('/');
        let custom_endpoint = settings.custom_endpoint.trim();

        if custom_endpoint.starts_with("http://") || custom_endpoint.starts_with("https://") {
            return validate_endpoint(custom_endpoint);
        }

        if base_url.is_empty() {
            return Ok("https://api.siliconflow.cn/v1/chat/completions".to_string());
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

    fn request_body(model_name: &str, prompt: &str, image_url: &str) -> Value {
        json!({
            "model": model_name,
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
                            "text": prompt
                        }
                    ]
                }
            ],
            "temperature": SILICONFLOW_TEMPERATURE,
            "max_tokens": SILICONFLOW_MAX_TOKENS
        })
    }
}

impl ModelProvider for SiliconFlowProvider {
    fn analyze_page(&self, request: &ModelAnalysisRequest) -> AppResult<ModelAnalysisResponse> {
        let api_key = security::read_api_key()?.ok_or_else(|| {
            AppError::new(
                "api_key_missing",
                "API key 未配置，无法调用硅基流动模型。",
                "analysis_provider",
                true,
            )
        })?;

        let image_base64 = base64::engine::general_purpose::STANDARD.encode(&request.image_bytes);
        let image_url = format!("data:{};base64,{}", request.image_mime_type, image_base64);

        let body = Self::request_body(&request.model_name, &request.prompt, &image_url);

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
                    "response_bytes={content_length}; max_response_bytes={MAX_PROVIDER_RESPONSE_BYTES}; endpoint_kind=siliconflow"
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
                "硅基流动 provider 返回非成功状态。",
                "analysis_provider",
                true,
            )
            .with_details(format!(
                "status={}; response_bytes={}; response_preview={}; endpoint_kind=siliconflow",
                status.as_u16(),
                response_text.len(),
                error_preview
            )));
        }

        Ok(ModelAnalysisResponse {
            raw_json: extract_siliconflow_content(&response_text)?,
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
        .with_details("endpoint_kind=siliconflow")
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
        .with_details("scheme=http; endpoint_kind=siliconflow")),
        _ => Err(AppError::new(
            "model_endpoint_invalid_scheme",
            "模型 endpoint 仅支持 HTTPS，或本机调试用 HTTP。",
            "analysis_provider",
            true,
        )
        .with_details("endpoint_kind=siliconflow")),
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
            "response_bytes>{MAX_PROVIDER_RESPONSE_BYTES}; max_response_bytes={MAX_PROVIDER_RESPONSE_BYTES}; endpoint_kind=siliconflow"
        )));
    }
    String::from_utf8(bytes).map_err(|_| {
        AppError::new(
            "model_response_not_utf8",
            "模型 provider 响应不是 UTF-8 文本。",
            "analysis_provider",
            true,
        )
        .with_details("endpoint_kind=siliconflow")
    })
}

fn extract_siliconflow_content(response_text: &str) -> AppResult<String> {
    let parsed: Value = serde_json::from_str(response_text).map_err(|err| {
        let preview = if response_text.len() > 500 {
            format!("{}...", &response_text[..500])
        } else {
            response_text.to_string()
        };
        AppError::new(
            "model_response_json_invalid",
            "硅基流动 provider 响应不是可解析 JSON。",
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
                "硅基流动响应缺少 choices[0].message.content 字段。",
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
        "硅基流动 provider 调用失败。",
        "analysis_provider",
        retryable,
    )
    .with_details(format!("summary={summary}; endpoint_kind=siliconflow"))
}

#[cfg(test)]
mod tests {
    use super::{extract_siliconflow_content, SiliconFlowProvider};
    use crate::domain::settings::AppSettingsDto;

    #[test]
    fn uses_official_siliconflow_endpoint_when_base_url_empty() {
        let settings = AppSettingsDto::default();
        assert_eq!(
            SiliconFlowProvider::request_endpoint(&settings).expect("endpoint"),
            "https://api.siliconflow.cn/v1/chat/completions"
        );
    }

    #[test]
    fn combines_base_url_and_default_path() {
        let mut settings = AppSettingsDto::default();
        settings.base_url = "https://custom.siliconflow.proxy.com".to_string();
        assert_eq!(
            SiliconFlowProvider::request_endpoint(&settings).expect("endpoint"),
            "https://custom.siliconflow.proxy.com/v1/chat/completions"
        );
    }

    #[test]
    fn accepts_openai_style_v1_base_url_without_duplicate_v1() {
        let mut settings = AppSettingsDto::default();
        settings.base_url = "https://api.siliconflow.cn/v1".to_string();
        assert_eq!(
            SiliconFlowProvider::request_endpoint(&settings).expect("endpoint"),
            "https://api.siliconflow.cn/v1/chat/completions"
        );
    }

    #[test]
    fn accepts_full_chat_completions_base_url() {
        let mut settings = AppSettingsDto::default();
        settings.base_url = "https://api.siliconflow.cn/v1/chat/completions".to_string();
        assert_eq!(
            SiliconFlowProvider::request_endpoint(&settings).expect("endpoint"),
            "https://api.siliconflow.cn/v1/chat/completions"
        );
    }

    #[test]
    fn extracts_content_from_siliconflow_response() {
        let raw =
            r#"{"choices":[{"message":{"content":"{\"schema_version\":\"page_analysis_v1\"}"}}]}"#;
        assert_eq!(
            extract_siliconflow_content(raw).expect("content"),
            r#"{"schema_version":"page_analysis_v1"}"#
        );
    }

    #[test]
    fn rejects_response_missing_content_field() {
        let raw = r#"{"choices":[{"message":{}}]}"#;
        let err = extract_siliconflow_content(raw).expect_err("missing content");
        assert_eq!(err.code, "model_response_format_invalid");
    }

    #[test]
    fn siliconflow_request_body_matches_reference_chat_completions_vision_shape() {
        let body = SiliconFlowProvider::request_body(
            "zai-org/GLM-4.6V",
            "analyze this page",
            "data:image/png;base64,abc",
        );

        assert_eq!(body["model"], "zai-org/GLM-4.6V");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"].as_array().expect("messages").len(), 1);
        assert_eq!(body["messages"][0]["content"][0]["type"], "image_url");
        assert_eq!(
            body["messages"][0]["content"][0]["image_url"]["url"],
            "data:image/png;base64,abc"
        );
        assert_eq!(body["messages"][0]["content"][1]["type"], "text");
        assert_eq!(
            body["messages"][0]["content"][1]["text"],
            "analyze this page"
        );
        assert_eq!(body["temperature"], 0.7);
        assert_eq!(body["max_tokens"], 1000);
        assert!(body.get("response_format").is_none());
    }
}
