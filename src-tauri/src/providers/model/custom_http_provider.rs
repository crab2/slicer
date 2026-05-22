use crate::domain::settings::AppSettingsDto;
use crate::errors::{AppError, AppResult};
use crate::providers::model::provider::{
    ModelAnalysisRequest, ModelAnalysisResponse, ModelProvider,
};
use crate::security;
use base64::Engine;
use serde_json::Value;
use std::io::Read;
use std::time::Duration;

pub struct CustomHttpModelProvider;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(90);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_PROVIDER_RESPONSE_BYTES: u64 = 1_000_000;

impl CustomHttpModelProvider {
    pub fn request_endpoint(settings: &AppSettingsDto) -> AppResult<String> {
        let base_url = settings.base_url.trim().trim_end_matches('/');
        let custom_endpoint = settings.custom_endpoint.trim();

        if custom_endpoint.starts_with("http://") || custom_endpoint.starts_with("https://") {
            return validate_endpoint(custom_endpoint);
        }

        if base_url.is_empty() {
            return Err(AppError::new(
                "model_endpoint_missing",
                "模型 endpoint 未配置。",
                "analysis_provider",
                true,
            ));
        }

        if custom_endpoint.is_empty() {
            return validate_endpoint(base_url);
        }

        validate_endpoint(&format!(
            "{base_url}/{}",
            custom_endpoint.trim_start_matches('/')
        ))
    }
}

impl ModelProvider for CustomHttpModelProvider {
    fn analyze_page(&self, request: &ModelAnalysisRequest) -> AppResult<ModelAnalysisResponse> {
        let api_key = security::read_api_key()?.ok_or_else(|| {
            AppError::new(
                "api_key_missing",
                "API key 未配置，无法调用远程模型。",
                "analysis_provider",
                true,
            )
        })?;

        let image_base64 = base64::engine::general_purpose::STANDARD.encode(&request.image_bytes);
        let body = serde_json::json!({
            "model": request.model_name,
            "prompt": request.prompt,
            "image_base64": image_base64,
            "image_mime_type": request.image_mime_type,
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
                    "response_bytes={content_length}; max_response_bytes={MAX_PROVIDER_RESPONSE_BYTES}; endpoint_kind=custom_http"
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
                "模型 provider 返回非成功状态。",
                "analysis_provider",
                true,
            )
            .with_details(format!(
                "status={}; response_bytes={}; response_preview={}; endpoint_kind=custom_http",
                status.as_u16(),
                response_text.len(),
                error_preview
            )));
        }

        Ok(ModelAnalysisResponse {
            raw_json: extract_model_json(&response_text)?,
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
        .with_details("endpoint_kind=custom_http")
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
        .with_details("scheme=http; endpoint_kind=custom_http")),
        _ => Err(AppError::new(
            "model_endpoint_invalid_scheme",
            "模型 endpoint 仅支持 HTTPS，或本机调试用 HTTP。",
            "analysis_provider",
            true,
        )
        .with_details("endpoint_kind=custom_http")),
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
            "response_bytes>{MAX_PROVIDER_RESPONSE_BYTES}; max_response_bytes={MAX_PROVIDER_RESPONSE_BYTES}; endpoint_kind=custom_http"
        )));
    }
    String::from_utf8(bytes).map_err(|_| {
        AppError::new(
            "model_response_not_utf8",
            "模型 provider 响应不是 UTF-8 文本。",
            "analysis_provider",
            true,
        )
        .with_details("endpoint_kind=custom_http")
    })
}

fn extract_model_json(response_text: &str) -> AppResult<String> {
    match serde_json::from_str::<Value>(response_text) {
        Ok(Value::Object(map)) => {
            if let Some(Value::String(content)) = map.get("content") {
                return Ok(content.clone());
            }
            Ok(Value::Object(map).to_string())
        }
        Ok(value) => Ok(value.to_string()),
        Err(err) => Err(AppError::new(
            "model_response_json_invalid",
            "模型 provider 响应不是可解析 JSON。",
            "analysis_provider",
            true,
        )
        .with_details(format!(
            "summary=json parse failed at line {} column {}; response_bytes={}",
            err.line(),
            err.column(),
            response_text.len()
        ))),
    }
}

fn provider_error(code: &str, retryable: bool, summary: &str) -> AppError {
    AppError::new(
        code,
        "模型 provider 调用失败。",
        "analysis_provider",
        retryable,
    )
    .with_details(format!("summary={summary}; endpoint_kind=custom_http"))
}

#[cfg(test)]
mod tests {
    use super::{extract_model_json, CustomHttpModelProvider};
    use crate::domain::settings::AppSettingsDto;

    #[test]
    fn combines_base_url_and_custom_endpoint() {
        let mut settings = AppSettingsDto::default();
        settings.base_url = "https://models.example.com/".to_string();
        settings.custom_endpoint = "/v1/analyze".to_string();

        assert_eq!(
            CustomHttpModelProvider::request_endpoint(&settings).expect("endpoint"),
            "https://models.example.com/v1/analyze"
        );
    }

    #[test]
    fn rejects_non_tls_remote_endpoint() {
        let mut settings = AppSettingsDto::default();
        settings.custom_endpoint = "http://models.example.com/v1/analyze".to_string();

        let err = CustomHttpModelProvider::request_endpoint(&settings).expect_err("insecure");

        assert_eq!(err.code, "model_endpoint_insecure");
    }

    #[test]
    fn permits_loopback_http_endpoint_for_local_testing() {
        let mut settings = AppSettingsDto::default();
        settings.custom_endpoint = "http://127.0.0.1:9000/v1/analyze".to_string();

        assert_eq!(
            CustomHttpModelProvider::request_endpoint(&settings).expect("loopback"),
            "http://127.0.0.1:9000/v1/analyze"
        );
    }

    #[test]
    fn extracts_content_string_without_wrapping_provider_response() {
        let raw = r#"{"content":"{\"schema_version\":\"page_analysis_v1\"}"}"#;
        assert_eq!(
            extract_model_json(raw).expect("content"),
            r#"{"schema_version":"page_analysis_v1"}"#
        );
    }

    #[test]
    fn invalid_response_details_do_not_include_body_or_secrets() {
        let err = extract_model_json("Authorization: Bearer sk-secret").expect_err("invalid");
        let details = err.details.unwrap();
        assert!(details.contains("response_bytes="));
        assert!(!details.contains("Authorization"));
        assert!(!details.contains("sk-secret"));
    }
}
