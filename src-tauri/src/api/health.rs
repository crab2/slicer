use axum::extract::State;
use serde::Serialize;

use crate::api::dto::ApiResponse;
use crate::api::state::ApiAppState;
use crate::domain::index::IndexStatusDto;
use crate::domain::workspace::WorkspaceStatusDto;
use crate::errors::AppError;
use crate::services::search_service::SearchService;

/// Response payload for `GET /health`.
#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    pub api_version: String,
    pub workspace: WorkspaceStatusDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<IndexStatusDto>,
}

pub async fn health_handler(
    State(state): State<ApiAppState>,
) -> Result<ApiResponse<HealthResponse>, AppError> {
    let ws = state.workspace.clone();
    tokio::task::spawn_blocking(move || {
        let workspace_status = ws.get_workspace_status();

        let index = if workspace_status.status == "ready" {
            match SearchService::get_index_status(&ws) {
                Ok(status) => Some(status),
                Err(err) => {
                    tracing::warn!(
                        target: "api",
                        code = %err.code,
                        "health check skipped index status"
                    );
                    None
                }
            }
        } else {
            None
        };

        Ok(ApiResponse::ok(HealthResponse {
            api_version: env!("CARGO_PKG_VERSION").to_string(),
            workspace: workspace_status,
            index,
        }))
    })
    .await
    .map_err(|err| {
        AppError::new(
            "health_task_failed",
            "localhost API 健康检查执行失败。",
            "api",
            true,
        )
        .with_details(err.to_string())
    })?
}
