use crate::api::endpoints::{
    get_document_handler, get_page_handler, rebuild_index_handler, search_handler,
};
use crate::api::health::health_handler;
use crate::api::state::ApiAppState;
use crate::errors::{AppError, AppResult};
use axum::routing::{get, post};
use axum::Router;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

pub fn build_router(state: ApiAppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/search", get(search_handler))
        .route("/pages/{page_id}", get(get_page_handler))
        .route("/documents/{document_id}", get(get_document_handler))
        .route("/indexes/rebuild", post(rebuild_index_handler))
        .with_state(state)
}

pub async fn serve(
    addr: SocketAddr,
    state: ApiAppState,
    shutdown: oneshot::Receiver<()>,
) -> AppResult<()> {
    let listener = TcpListener::bind(addr).await.map_err(|err| {
        let code = match err.kind() {
            std::io::ErrorKind::AddrInUse => "api_server_port_in_use",
            _ => "api_server_bind_failed",
        };
        let message = if matches!(err.kind(), std::io::ErrorKind::AddrInUse) {
            "localhost API 端口已被占用，请更换端口或释放该端口后重试。"
        } else {
            "localhost API 启动失败，请检查端口与权限设置。"
        };
        AppError::new(code, message, "api", true).with_details(err.to_string())
    })?;

    let router = build_router(state);
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            let _ = shutdown.await;
        })
        .await
        .map_err(|err| {
            AppError::new(
                "api_server_serve_failed",
                "localhost API 运行过程中发生错误。",
                "api",
                true,
            )
            .with_details(err.to_string())
        })?;
    Ok(())
}
