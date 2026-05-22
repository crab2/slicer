use crate::api::state::ApiAppState;
use crate::domain::settings::{ApiServerStatusDto, AppSettingsDto};
use crate::errors::{AppError, AppResult};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Mutex;
use tauri::async_runtime;
use tokio::sync::oneshot;

const ALLOWED_BIND: &str = "127.0.0.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApiRuntimeStatus {
    Disabled,
    Stopped,
    Running,
    Failed,
}

impl ApiRuntimeStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Stopped => "stopped",
            Self::Running => "running",
            Self::Failed => "failed",
        }
    }
}

struct ApiServerInner {
    status: ApiRuntimeStatus,
    bind_address: String,
    port: u16,
    enabled: bool,
    last_error: Option<AppError>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    join_handle: Option<async_runtime::JoinHandle<()>>,
    state: ApiAppState,
}

pub struct ApiServerService {
    inner: Mutex<ApiServerInner>,
}

impl ApiServerService {
    pub fn new(state: ApiAppState) -> Self {
        Self {
            inner: Mutex::new(ApiServerInner {
                status: ApiRuntimeStatus::Disabled,
                bind_address: ALLOWED_BIND.to_string(),
                port: 17321,
                enabled: false,
                last_error: None,
                shutdown_tx: None,
                join_handle: None,
                state,
            }),
        }
    }

    pub fn start(&self, settings: &AppSettingsDto) -> AppResult<()> {
        let mut guard = self.lock_inner()?;

        guard.bind_address = settings.api_bind_address.clone();
        guard.port = settings.api_port;
        guard.enabled = settings.api_enabled;

        if !settings.api_enabled {
            guard.status = ApiRuntimeStatus::Disabled;
            let err = AppError::new(
                "api_server_disabled",
                "localhost API 当前未启用，无法启动。",
                "api",
                true,
            );
            guard.last_error = Some(err.clone());
            return Err(err);
        }

        if guard.shutdown_tx.is_some() {
            let err = AppError::new(
                "api_server_already_running",
                "localhost API 已在运行，请先停止再启动。",
                "api",
                true,
            );
            guard.last_error = Some(err.clone());
            return Err(err);
        }

        if settings.api_bind_address.trim() != ALLOWED_BIND {
            let err = AppError::new(
                "api_server_bind_failed",
                "localhost API 仅允许监听 127.0.0.1。",
                "api",
                true,
            )
            .with_details(format!("bind_address={}", settings.api_bind_address));
            guard.status = ApiRuntimeStatus::Failed;
            guard.last_error = Some(err.clone());
            return Err(err);
        }

        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), settings.api_port);

        let listener =
            match async_runtime::block_on(async { tokio::net::TcpListener::bind(addr).await }) {
                Ok(listener) => listener,
                Err(err) => {
                    let code = match err.kind() {
                        std::io::ErrorKind::AddrInUse => "api_server_port_in_use",
                        _ => "api_server_bind_failed",
                    };
                    let message = if matches!(err.kind(), std::io::ErrorKind::AddrInUse) {
                        "localhost API 端口已被占用，请更换端口或释放该端口后重试。"
                    } else {
                        "localhost API 启动失败，请检查端口与权限设置。"
                    };
                    let mapped =
                        AppError::new(code, message, "api", true).with_details(err.to_string());
                    guard.status = ApiRuntimeStatus::Failed;
                    guard.last_error = Some(mapped.clone());
                    return Err(mapped);
                }
            };

        let actual_addr = listener.local_addr().map_err(|err| {
            AppError::new(
                "api_server_bind_failed",
                "localhost API 启动失败，请检查端口与权限设置。",
                "api",
                true,
            )
            .with_details(err.to_string())
        })?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let router = crate::api::server::build_router(guard.state.clone());

        let handle = async_runtime::spawn(async move {
            let serve_future = axum::serve(listener, router).with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });
            if let Err(err) = serve_future.await {
                tracing::error!(
                    target: "api",
                    error = %err,
                    "API server task exited with error"
                );
            }
        });

        guard.bind_address = ALLOWED_BIND.to_string();
        guard.port = actual_addr.port();
        guard.enabled = true;
        guard.status = ApiRuntimeStatus::Running;
        guard.last_error = None;
        guard.shutdown_tx = Some(shutdown_tx);
        guard.join_handle = Some(handle);

        tracing::info!(
            target: "api",
            bind_address = %guard.bind_address,
            port = guard.port,
            "localhost API server 已启动"
        );

        Ok(())
    }

    pub fn stop(&self) -> AppResult<()> {
        let mut guard = self.lock_inner()?;
        let shutdown_tx = guard.shutdown_tx.take();
        let join_handle = guard.join_handle.take();

        let bind_address = guard.bind_address.clone();
        let port = guard.port;

        if shutdown_tx.is_none() && join_handle.is_none() {
            guard.status = if guard.enabled {
                ApiRuntimeStatus::Stopped
            } else {
                ApiRuntimeStatus::Disabled
            };
            return Ok(());
        }

        drop(guard);

        if let Some(tx) = shutdown_tx {
            let _ = tx.send(());
        }
        if let Some(handle) = join_handle {
            let _ = async_runtime::block_on(async { handle.await });
        }

        let mut guard = self.lock_inner()?;
        guard.status = if guard.enabled {
            ApiRuntimeStatus::Stopped
        } else {
            ApiRuntimeStatus::Disabled
        };
        guard.last_error = None;

        tracing::info!(
            target: "api",
            bind_address = %bind_address,
            port = port,
            "localhost API server 已停止"
        );

        Ok(())
    }

    pub fn reconcile(&self, settings: &AppSettingsDto) -> AppResult<()> {
        let snapshot = {
            let guard = self.lock_inner()?;
            (
                guard.status,
                guard.bind_address.clone(),
                guard.port,
                guard.enabled,
                guard.shutdown_tx.is_some(),
            )
        };
        let (status, current_bind, current_port, current_enabled, has_handle) = snapshot;

        if !settings.api_enabled {
            if has_handle {
                self.stop()?;
            }
            let mut guard = self.lock_inner()?;
            guard.enabled = false;
            guard.bind_address = settings.api_bind_address.clone();
            guard.port = settings.api_port;
            guard.status = ApiRuntimeStatus::Disabled;
            guard.last_error = None;
            return Ok(());
        }

        let same_target = settings.api_bind_address.trim() == current_bind.trim()
            && settings.api_port == current_port
            && current_enabled
            && status == ApiRuntimeStatus::Running
            && has_handle;

        if same_target {
            return Ok(());
        }

        if has_handle {
            self.stop()?;
        }
        self.start(settings)
    }

    pub fn reconcile_for_new_workspace(&self, settings: &AppSettingsDto) -> AppResult<()> {
        if let Err(err) = self.stop() {
            tracing::warn!(
                target: "api",
                code = %err.code,
                "切换工作区时停止 localhost API 失败"
            );
        }
        if !settings.api_enabled {
            let mut guard = self.lock_inner()?;
            guard.enabled = false;
            guard.bind_address = settings.api_bind_address.clone();
            guard.port = settings.api_port;
            guard.status = ApiRuntimeStatus::Disabled;
            guard.last_error = None;
            return Ok(());
        }
        self.start(settings)
    }

    pub fn get_runtime_status(&self) -> ApiServerStatusDto {
        match self.inner.lock() {
            Ok(guard) => ApiServerStatusDto {
                runtime_status: guard.status.as_str().to_string(),
                bind_address: guard.bind_address.clone(),
                port: guard.port,
                enabled: guard.enabled,
                last_error: guard.last_error.clone(),
            },
            Err(_) => ApiServerStatusDto {
                runtime_status: ApiRuntimeStatus::Failed.as_str().to_string(),
                bind_address: ALLOWED_BIND.to_string(),
                port: 0,
                enabled: false,
                last_error: Some(state_poisoned_error()),
            },
        }
    }

    fn lock_inner(&self) -> AppResult<std::sync::MutexGuard<'_, ApiServerInner>> {
        self.inner.lock().map_err(|_| state_poisoned_error())
    }
}

fn state_poisoned_error() -> AppError {
    AppError::new(
        "api_server_state_poisoned",
        "localhost API 状态暂时不可用，请重启应用后重试。",
        "api",
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::workspace_service::WorkspaceService;
    use std::net::TcpStream;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    fn test_state() -> ApiAppState {
        let dir = std::env::temp_dir().join(format!("slicer-test-{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&dir);
        ApiAppState::new(Arc::new(WorkspaceService::new(dir)))
    }

    fn ready_workspace_state() -> (ApiAppState, PathBuf) {
        let root = std::env::temp_dir().join(format!("slicer-health-{}", uuid::Uuid::new_v4()));
        let config_dir = root.join("config");
        let workspace_dir = root.join("workspace");
        std::fs::create_dir_all(&config_dir).expect("config dir");
        std::fs::create_dir_all(&workspace_dir).expect("workspace dir");
        std::fs::write(
            config_dir.join("bootstrap-workspace.json"),
            serde_json::to_string(&serde_json::json!({
                "last_workspace_path": workspace_dir.to_string_lossy()
            }))
            .expect("bootstrap json"),
        )
        .expect("bootstrap file");
        (
            ApiAppState::new(Arc::new(WorkspaceService::new(config_dir))),
            root,
        )
    }

    fn settings_with(port: u16, enabled: bool) -> AppSettingsDto {
        let mut s = AppSettingsDto::default();
        s.api_enabled = enabled;
        s.api_bind_address = "127.0.0.1".to_string();
        s.api_port = port;
        s
    }

    fn allocate_port() -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("ephemeral bind");
        let port = listener.local_addr().expect("local addr").port();
        drop(listener);
        port
    }

    #[test]
    fn start_then_stop_releases_port() {
        let port = allocate_port();
        let service = ApiServerService::new(test_state());
        let settings = settings_with(port, true);
        service.start(&settings).expect("start");

        let connect = TcpStream::connect_timeout(
            &format!("127.0.0.1:{port}").parse().unwrap(),
            Duration::from_secs(2),
        );
        assert!(
            connect.is_ok(),
            "expected port {port} to be listening after start"
        );

        service.stop().expect("stop");

        std::thread::sleep(Duration::from_millis(150));
        let connect_after = TcpStream::connect_timeout(
            &format!("127.0.0.1:{port}").parse().unwrap(),
            Duration::from_millis(500),
        );
        assert!(
            connect_after.is_err(),
            "expected port {port} to be free after stop"
        );

        let status = service.get_runtime_status();
        assert_eq!(status.runtime_status, "stopped");
    }

    #[test]
    fn start_serves_health_for_ready_workspace() {
        let port = allocate_port();
        let (state, root) = ready_workspace_state();
        let service = ApiServerService::new(state);
        let settings = settings_with(port, true);
        service.start(&settings).expect("start");

        let mut last_err: Option<reqwest::Error> = None;
        let mut response = None;
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(100));
            match reqwest::blocking::get(format!("http://127.0.0.1:{port}/health")) {
                Ok(resp) => {
                    response = Some(resp);
                    break;
                }
                Err(err) => last_err = Some(err),
            }
        }
        let response =
            response.unwrap_or_else(|| panic!("health request never succeeded: {last_err:?}"));
        assert_eq!(response.status().as_u16(), 200);
        let body: serde_json::Value = response.json().expect("json body");
        assert_eq!(body["data"]["workspace"]["status"], "ready");
        assert!(
            body["data"].get("index").is_some(),
            "ready workspace health should include index status"
        );

        service.stop().expect("stop");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn start_when_already_running_returns_error() {
        let port = allocate_port();
        let service = ApiServerService::new(test_state());
        let settings = settings_with(port, true);
        service.start(&settings).expect("first start");

        let err = service
            .start(&settings)
            .expect_err("second start should fail");
        assert_eq!(err.code, "api_server_already_running");

        service.stop().expect("stop");
    }

    #[test]
    fn start_with_disabled_returns_error() {
        let service = ApiServerService::new(test_state());
        let settings = settings_with(allocate_port(), false);
        let err = service.start(&settings).expect_err("start should fail");
        assert_eq!(err.code, "api_server_disabled");
        let status = service.get_runtime_status();
        assert_eq!(status.runtime_status, "disabled");
        assert!(!status.enabled);
    }

    #[test]
    fn reconcile_is_idempotent() {
        let port = allocate_port();
        let service = ApiServerService::new(test_state());
        let settings = settings_with(port, true);
        service.reconcile(&settings).expect("first reconcile");
        let status_a = service.get_runtime_status();
        assert_eq!(status_a.runtime_status, "running");

        service.reconcile(&settings).expect("second reconcile");
        let status_b = service.get_runtime_status();
        assert_eq!(status_b.runtime_status, "running");
        assert_eq!(status_a.port, status_b.port);

        service.stop().expect("stop");
    }

    #[test]
    fn reconcile_changes_port_restarts_server() {
        let first_port = allocate_port();
        let service = ApiServerService::new(test_state());
        service
            .reconcile(&settings_with(first_port, true))
            .expect("initial start");
        let connect_first = TcpStream::connect_timeout(
            &format!("127.0.0.1:{first_port}").parse().unwrap(),
            Duration::from_secs(2),
        );
        assert!(connect_first.is_ok());

        let new_port = allocate_port();
        service
            .reconcile(&settings_with(new_port, true))
            .expect("port change reconcile");

        std::thread::sleep(Duration::from_millis(150));

        let connect_old = TcpStream::connect_timeout(
            &format!("127.0.0.1:{first_port}").parse().unwrap(),
            Duration::from_millis(500),
        );
        assert!(
            connect_old.is_err(),
            "old port {first_port} should be released"
        );

        let connect_new = TcpStream::connect_timeout(
            &format!("127.0.0.1:{new_port}").parse().unwrap(),
            Duration::from_secs(2),
        );
        assert!(
            connect_new.is_ok(),
            "new port {new_port} should be listening"
        );

        service.stop().expect("stop");
    }

    #[test]
    fn port_in_use_maps_to_specific_error_code() {
        let port = allocate_port();
        let blocker = std::net::TcpListener::bind(format!("127.0.0.1:{port}")).expect("blocker");
        let service = ApiServerService::new(test_state());
        let err = service
            .start(&settings_with(port, true))
            .expect_err("start should fail when port is held");
        assert_eq!(err.code, "api_server_port_in_use");
        let status = service.get_runtime_status();
        assert_eq!(status.runtime_status, "failed");
        assert_eq!(
            status.last_error.as_ref().expect("last error").code,
            "api_server_port_in_use"
        );

        drop(blocker);
    }

    #[test]
    fn reconcile_disable_stops_server() {
        let port = allocate_port();
        let service = ApiServerService::new(test_state());
        service
            .reconcile(&settings_with(port, true))
            .expect("start");
        service
            .reconcile(&settings_with(port, false))
            .expect("disable");
        let status = service.get_runtime_status();
        assert_eq!(status.runtime_status, "disabled");
        assert!(!status.enabled);
    }

    #[test]
    fn rejects_non_loopback_bind_address() {
        let port = allocate_port();
        let service = ApiServerService::new(test_state());
        let mut settings = settings_with(port, true);
        settings.api_bind_address = "0.0.0.0".to_string();
        let err = service
            .start(&settings)
            .expect_err("non-loopback should fail");
        assert_eq!(err.code, "api_server_bind_failed");
    }
}
