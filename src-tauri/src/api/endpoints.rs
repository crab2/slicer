use axum::extract::{Path, Query, State};
use serde::Deserialize;

use crate::api::auth::BearerAuth;
use crate::api::dto::ApiResponse;
use crate::api::state::ApiAppState;
use crate::domain::document::DocumentDto;
use crate::domain::index::{IndexRebuildStartDto, SearchResponseDto};
use crate::domain::page::PageRecordDto;
use crate::errors::AppError;
use crate::repositories::document_repository::DocumentRepository;
use crate::services::search_service::SearchService;

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub limit: Option<usize>,
}

pub async fn search_handler(
    State(state): State<ApiAppState>,
    Query(params): Query<SearchQuery>,
) -> Result<ApiResponse<SearchResponseDto>, AppError> {
    let ws = state.workspace.clone();
    let query = params.q;
    let limit = params.limit.unwrap_or(20);
    tokio::task::spawn_blocking(move || {
        let result = SearchService::search(&ws, &query, limit)?;
        Ok(ApiResponse::ok(result))
    })
    .await
    .map_err(|err| {
        AppError::new("search_task_failed", "搜索任务执行失败。", "api", true)
            .with_details(err.to_string())
    })?
}

pub async fn get_page_handler(
    State(state): State<ApiAppState>,
    Path(page_id): Path<String>,
) -> Result<ApiResponse<PageRecordDto>, AppError> {
    let ws = state.workspace.clone();
    tokio::task::spawn_blocking(move || {
        let mut conn = ws.get_db_connection()?;
        match DocumentRepository::find_page_by_id(&mut conn, &page_id)? {
            Some(page) => Ok(ApiResponse::ok(page)),
            None => Err(AppError::new(
                "page_not_found",
                "未找到指定的页面记录。",
                "api",
                false,
            )),
        }
    })
    .await
    .map_err(|err| {
        AppError::new("page_task_failed", "页面查询任务执行失败。", "api", true)
            .with_details(err.to_string())
    })?
}

pub async fn get_document_handler(
    State(state): State<ApiAppState>,
    Path(document_id): Path<String>,
) -> Result<ApiResponse<DocumentDto>, AppError> {
    let ws = state.workspace.clone();
    tokio::task::spawn_blocking(move || {
        let mut conn = ws.get_db_connection()?;
        match DocumentRepository::find_document_by_id(&mut conn, &document_id)? {
            Some(doc) => Ok(ApiResponse::ok(doc)),
            None => Err(AppError::new(
                "document_not_found",
                "未找到指定的文档记录。",
                "api",
                false,
            )),
        }
    })
    .await
    .map_err(|err| {
        AppError::new(
            "document_task_failed",
            "文档查询任务执行失败。",
            "api",
            true,
        )
        .with_details(err.to_string())
    })?
}

pub async fn rebuild_index_handler(
    State(state): State<ApiAppState>,
    _auth: BearerAuth,
) -> Result<ApiResponse<IndexRebuildStartDto>, AppError> {
    let ws = state.workspace.clone();
    tokio::task::spawn_blocking(move || {
        let result = SearchService::start_index_rebuild(&ws)?;
        Ok(ApiResponse::ok(result))
    })
    .await
    .map_err(|err| {
        AppError::new("rebuild_task_failed", "索引重建任务启动失败。", "api", true)
            .with_details(err.to_string())
    })?
}
