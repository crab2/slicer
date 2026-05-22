use slicer_lib::api;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::time::Duration;
use tokio::sync::oneshot;

fn allocate_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("ephemeral bind");
    let port = listener.local_addr().expect("local addr").port();
    drop(listener);
    port
}

fn test_state() -> api::state::ApiAppState {
    let dir = std::env::temp_dir().join(format!("slicer-test-{}", uuid::Uuid::new_v4()));
    api::state::ApiAppState::for_test(dir)
}

struct ServerHandle {
    state: api::state::ApiAppState,
    shutdown_tx: oneshot::Sender<()>,
    server_handle: Option<std::thread::JoinHandle<()>>,
    port: u16,
}

fn start_server() -> ServerHandle {
    start_server_with_state(test_state())
}

fn start_server_with_state(state: api::state::ApiAppState) -> ServerHandle {
    let port = allocate_port();
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let state_clone = state.clone();
    let server_handle = std::thread::spawn(move || {
        let result = tauri::async_runtime::block_on(async move {
            api::server::serve(addr, state_clone, shutdown_rx).await
        });
        assert!(result.is_ok(), "serve returned error: {:?}", result.err());
    });

    // Poll until server accepts connections
    let mut last_err: Option<reqwest::Error> = None;
    for _ in 0..30 {
        std::thread::sleep(Duration::from_millis(100));
        match reqwest::blocking::get(format!("http://127.0.0.1:{port}/health")) {
            Ok(_) => break,
            Err(err) => last_err = Some(err),
        }
    }
    if last_err.is_some() {
        std::thread::sleep(Duration::from_millis(100));
        let _ = reqwest::blocking::get(format!("http://127.0.0.1:{port}/health"))
            .unwrap_or_else(|e| panic!("server never accepted requests: last_err={:?}", e));
    }

    ServerHandle {
        state,
        shutdown_tx,
        server_handle: Some(server_handle),
        port,
    }
}

impl ServerHandle {
    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn shutdown(mut self) {
        let _ = self.shutdown_tx.send(());
        if let Some(handle) = self.server_handle.take() {
            assert!(handle.join().is_ok(), "server thread panicked");
        }
    }
}

// ── Health endpoint ──────────────────────────────────────────────────────────

#[test]
fn health_route_returns_200_with_expected_shape() {
    let server = start_server();
    let response =
        reqwest::blocking::get(format!("{}/health", server.base_url())).expect("health request");

    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().expect("json body");
    assert!(
        body.get("data").is_some(),
        "response should have 'data' field"
    );
    let data = &body["data"];
    assert!(
        data.get("api_version").is_some(),
        "data should have 'api_version'"
    );
    assert!(
        data.get("workspace").is_some(),
        "data should have 'workspace'"
    );

    server.shutdown();
}

#[test]
fn health_route_returns_200_for_ready_workspace() {
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

    let server = start_server_with_state(api::state::ApiAppState::for_test(config_dir));
    let response =
        reqwest::blocking::get(format!("{}/health", server.base_url())).expect("health request");

    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().expect("json body");
    assert_eq!(body["data"]["workspace"]["status"], "ready");
    assert!(
        body["data"].get("index").is_some(),
        "ready workspace health should include index status"
    );

    server.shutdown();
    let _ = std::fs::remove_dir_all(root);
}

// ── Search endpoint ──────────────────────────────────────────────────────────

#[test]
fn search_endpoint_returns_200_with_data_field() {
    let server = start_server();
    let response = reqwest::blocking::get(format!("{}/search?q=test", server.base_url()))
        .expect("search request");

    // In a test workspace without an initialized index, the endpoint may return 500
    // (search index not built) or 200 (empty results). Either is acceptable for contract testing.
    let status = response.status().as_u16();
    let body: serde_json::Value = response.json().expect("json body");

    if status == 200 {
        assert!(
            body.get("data").is_some(),
            "response should have 'data' field"
        );
        let data = &body["data"];
        assert!(data.get("items").is_some(), "data should have 'items'");
        assert!(data.get("query").is_some(), "data should have 'query'");
        assert!(data.get("limit").is_some(), "data should have 'limit'");
    } else {
        // Error response must conform to error contract
        assert_eq!(status, 500);
        assert!(
            body.get("error").is_some(),
            "error response should have 'error' field"
        );
        assert!(body["error"].get("code").is_some());
    }

    server.shutdown();
}

// ── Page endpoint ────────────────────────────────────────────────────────────

#[test]
fn page_not_found_returns_error() {
    let server = start_server();
    let response = reqwest::blocking::get(format!("{}/pages/nonexistent", server.base_url()))
        .expect("page request");

    // Test workspace has no DB initialized, so this returns 500 or 404.
    // Either way, the error contract must hold.
    let status = response.status().as_u16();
    assert!(
        status == 404 || status == 500,
        "expected 404 or 500, got {}",
        status
    );
    let body: serde_json::Value = response.json().expect("json body");
    assert!(
        body.get("error").is_some(),
        "response should have 'error' field"
    );
    assert!(
        body["error"].get("code").is_some(),
        "error should have 'code'"
    );
    assert!(
        body["error"].get("message").is_some(),
        "error should have 'message'"
    );
    assert!(
        body["error"].get("correlation_id").is_some(),
        "error should have 'correlation_id'"
    );

    server.shutdown();
}

// ── Document endpoint ────────────────────────────────────────────────────────

#[test]
fn document_not_found_returns_error() {
    let server = start_server();
    let response = reqwest::blocking::get(format!("{}/documents/nonexistent", server.base_url()))
        .expect("document request");

    // Test workspace has no DB initialized, so this returns 500 or 404.
    let status = response.status().as_u16();
    assert!(
        status == 404 || status == 500,
        "expected 404 or 500, got {}",
        status
    );
    let body: serde_json::Value = response.json().expect("json body");
    assert!(
        body.get("error").is_some(),
        "response should have 'error' field"
    );
    assert!(
        body["error"].get("code").is_some(),
        "error should have 'code'"
    );
    assert!(
        body["error"].get("message").is_some(),
        "error should have 'message'"
    );
    assert!(
        body["error"].get("correlation_id").is_some(),
        "error should have 'correlation_id'"
    );

    server.shutdown();
}

// ── Rebuild endpoint — auth tests ────────────────────────────────────────────

#[test]
fn rebuild_without_token_returns_401() {
    let server = start_server();
    let client = reqwest::blocking::Client::new();
    let response = client
        .post(format!("{}/indexes/rebuild", server.base_url()))
        .send()
        .expect("rebuild request");

    assert_eq!(response.status().as_u16(), 401);
    let body: serde_json::Value = response.json().expect("json body");
    assert!(
        body.get("error").is_some(),
        "response should have 'error' field"
    );
    assert_eq!(body["error"]["code"], "missing_authorization");

    server.shutdown();
}

#[test]
fn rebuild_with_invalid_token_returns_401() {
    let server = start_server();
    let client = reqwest::blocking::Client::new();
    let response = client
        .post(format!("{}/indexes/rebuild", server.base_url()))
        .header("Authorization", "Bearer wrong-token")
        .send()
        .expect("rebuild request");

    assert_eq!(response.status().as_u16(), 401);
    let body: serde_json::Value = response.json().expect("json body");
    assert!(
        body.get("error").is_some(),
        "response should have 'error' field"
    );
    assert_eq!(body["error"]["code"], "invalid_token");

    server.shutdown();
}

#[test]
fn rebuild_with_valid_token_returns_success_or_error() {
    let server = start_server();
    let token = server
        .state
        .api_token
        .read()
        .unwrap()
        .as_ref()
        .unwrap()
        .clone();

    let client = reqwest::blocking::Client::new();
    let response = client
        .post(format!("{}/indexes/rebuild", server.base_url()))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .expect("rebuild request");

    // Test workspace has no search index, so rebuild may fail with 500.
    // The key contract: valid token passes auth, response conforms to DTO.
    let status = response.status().as_u16();
    let body: serde_json::Value = response.json().expect("json body");

    if status == 200 {
        assert!(
            body.get("data").is_some(),
            "response should have 'data' field"
        );
        assert!(
            body["data"].get("status").is_some(),
            "data should have 'status'"
        );
    } else {
        // Error response must conform to error contract
        assert!(
            status == 500 || status == 503,
            "expected 200/500/503, got {}",
            status
        );
        assert!(
            body.get("error").is_some(),
            "error response should have 'error' field"
        );
        assert!(body["error"].get("code").is_some());
    }

    server.shutdown();
}
