use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// Bearer token extractor for protected endpoints.
///
/// Reads `Authorization: Bearer <token>` from request headers and validates
/// against the token stored in `ApiAppState`.
pub struct BearerAuth;

#[derive(Debug, Serialize)]
pub struct AuthErrorBody {
    pub error: AuthErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct AuthErrorDetail {
    pub code: &'static str,
    pub message: &'static str,
}

impl IntoResponse for AuthErrorBody {
    fn into_response(self) -> Response {
        (StatusCode::UNAUTHORIZED, axum::Json(self)).into_response()
    }
}

impl<S> FromRequestParts<S> for BearerAuth
where
    S: Send + Sync,
    crate::api::state::ApiAppState: FromRef<S>,
{
    type Rejection = AuthErrorBody;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = crate::api::state::ApiAppState::from_ref(state);

        let stored_token = app_state.api_token.read().map_err(|_| AuthErrorBody {
            error: AuthErrorDetail {
                code: "auth_state_poisoned",
                message: "认证状态不可用。",
            },
        })?;

        let stored = match stored_token.as_ref() {
            Some(t) => t.clone(),
            None => {
                return Err(AuthErrorBody {
                    error: AuthErrorDetail {
                        code: "api_token_not_configured",
                        message: "API token 未配置，请在设置中重置 token。",
                    },
                });
            }
        };

        let header_value = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok());

        let bearer = match header_value {
            Some(val) if val.starts_with("Bearer ") => &val[7..],
            _ => {
                return Err(AuthErrorBody {
                    error: AuthErrorDetail {
                        code: "missing_authorization",
                        message: "缺少 Authorization: Bearer <token> 请求头。",
                    },
                });
            }
        };

        if bearer == stored {
            Ok(BearerAuth)
        } else {
            Err(AuthErrorBody {
                error: AuthErrorDetail {
                    code: "invalid_token",
                    message: "API token 无效。",
                },
            })
        }
    }
}

use axum::extract::FromRef;
