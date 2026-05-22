use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::errors::AppError;

/// Unified success response wrapper.
///
/// All successful API responses are serialized as `{ "data": T }`.
#[derive(Debug, Clone, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub data: T,
}

impl<T: Serialize> ApiResponse<T> {
    /// 200 OK with JSON body `{ "data": T }`.
    pub fn ok(data: T) -> Self {
        Self { data }
    }

    /// 201 Created with JSON body `{ "data": T }`.
    pub fn created(data: T) -> ApiCreatedResponse<T> {
        ApiCreatedResponse { data }
    }
}

/// Wrapper that forces 201 status code.
#[derive(Debug, Clone, Serialize)]
pub struct ApiCreatedResponse<T: Serialize> {
    pub data: T,
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> Response {
        let mut resp = axum::Json(self).into_response();
        *resp.status_mut() = StatusCode::OK;
        resp
    }
}

impl<T: Serialize> IntoResponse for ApiCreatedResponse<T> {
    fn into_response(self) -> Response {
        let mut resp = axum::Json(self).into_response();
        *resp.status_mut() = StatusCode::CREATED;
        resp
    }
}

/// Error response body serialized as `{ "error": { ... } }`.
#[derive(Debug, Clone, Serialize)]
pub struct ApiErrorBody {
    pub error: ApiErrorDetail,
}

/// Inner error detail — mirrors `AppError` fields.
#[derive(Debug, Clone, Serialize)]
pub struct ApiErrorDetail {
    pub code: String,
    pub message: String,
    pub stage: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    pub correlation_id: String,
}

impl From<&AppError> for ApiErrorBody {
    fn from(err: &AppError) -> Self {
        Self {
            error: ApiErrorDetail {
                code: err.code.clone(),
                message: err.message.clone(),
                stage: err.stage.clone(),
                retryable: err.retryable,
                details: err.details.clone(),
                correlation_id: err.correlation_id.clone(),
            },
        }
    }
}

/// Map `AppError.code` prefix to an HTTP status code.
fn status_code_for_error(err: &AppError) -> StatusCode {
    if err.code.starts_with("api_server_") {
        return StatusCode::SERVICE_UNAVAILABLE;
    }
    if err.code.starts_with("not_found") || err.code.contains("_not_found") {
        return StatusCode::NOT_FOUND;
    }
    if err.code.starts_with("validation_") {
        return StatusCode::BAD_REQUEST;
    }
    StatusCode::INTERNAL_SERVER_ERROR
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = status_code_for_error(&self);
        let body = ApiErrorBody::from(&self);
        (status, axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_response_ok_serializes_with_data_field() {
        let resp = ApiResponse::ok(serde_json::json!({"key": "value"}));
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json, serde_json::json!({"data": {"key": "value"}}));
    }

    #[test]
    fn api_response_created_serializes_with_data_field() {
        let resp = ApiResponse::created(serde_json::json!({"id": 42}));
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json, serde_json::json!({"data": {"id": 42}}));
    }

    #[test]
    fn api_error_body_serializes_with_error_field() {
        let err = AppError::new("test_code", "test message", "test_stage", true);
        let body = ApiErrorBody::from(&err);
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["error"]["code"], "test_code");
        assert_eq!(json["error"]["message"], "test message");
        assert_eq!(json["error"]["stage"], "test_stage");
        assert_eq!(json["error"]["retryable"], true);
        assert!(json["error"]["correlation_id"].is_string());
        // details should be absent when None
        assert!(json["error"]["details"].is_null());
    }

    #[test]
    fn api_error_body_serializes_details_when_present() {
        let err = AppError::new("test_code", "msg", "stage", false).with_details("extra info");
        let body = ApiErrorBody::from(&err);
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["error"]["details"], "extra info");
    }

    #[test]
    fn app_error_maps_api_server_codes_to_503() {
        let err = AppError::new("api_server_port_in_use", "msg", "api", true);
        assert_eq!(status_code_for_error(&err), StatusCode::SERVICE_UNAVAILABLE);

        let err2 = AppError::new("api_server_bind_failed", "msg", "api", true);
        assert_eq!(
            status_code_for_error(&err2),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[test]
    fn app_error_maps_not_found_codes_to_404() {
        let err = AppError::new("not_found", "msg", "search", false);
        assert_eq!(status_code_for_error(&err), StatusCode::NOT_FOUND);

        let err2 = AppError::new("page_not_found", "msg", "api", false);
        assert_eq!(status_code_for_error(&err2), StatusCode::NOT_FOUND);
    }

    #[test]
    fn app_error_maps_validation_codes_to_400() {
        let err = AppError::new("validation_failed", "msg", "api", false);
        assert_eq!(status_code_for_error(&err), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn app_error_maps_unknown_codes_to_500() {
        let err = AppError::new("something_went_wrong", "msg", "internal", false);
        assert_eq!(
            status_code_for_error(&err),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
