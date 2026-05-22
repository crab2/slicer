---
title: 'Story 5.3: GET /health 返回应用、工作区与索引状态'
type: 'feature'
created: '2026-05-20'
status: 'review'
baseline_commit: 'b901e8f'
context: []
---

# Story 5.3: GET /health 返回应用、工作区与索引状态

**Status:** review

## Story

As a 本地自动化用户,
I want 通过 GET /health 查询应用、工作区和索引的当前状态,
so that 我的脚本可以判断 slicer 是否就绪并了解当前工作区状态。

## Scope Boundaries

**In scope (本故事必须完成):**

- 实现 `GET /health` endpoint，返回 200 OK + `{ "data": { "api_version", "workspace", "index" } }`。
- `workspace` 字段来自 `WorkspaceService::get_workspace_status()`。
- `index` 字段来自 `SearchService::get_index_status()`，仅在 workspace ready 时填充，否则为 `null`。
- `api_version` 来自 `env!("CARGO_PKG_VERSION")`。
- 更新 `api/server.rs` 的 `build_router()` 接受 `ApiAppState` 并注入 axum state。
- 更新 `ApiServerService` 存储 `ApiAppState` 并传递给 router。
- 移除 `__api_alive` 占位路由（由 `/health` 替代）。
- `SearchService::get_index_status` 是同步 DB 操作，handler 中使用 `tokio::task::spawn_blocking`。
- 集成测试验证 `/health` 返回 200 和正确的 JSON 结构。

**Out of scope (本故事禁止实现 — 留给后续 Story):**

- ❌ `GET /search`、`GET /pages/{page_id}` 等业务 endpoint（留给 5.4）。
- ❌ token 认证中间件（留给 5.5）。
- ❌ API contract tests 全套覆盖（留给 5.6）。

## Intent

**Problem:** Story 5.2 建立了统一 DTO 契约，但 router 只有返回 204 的占位路由。用户和自动化工具需要一个端点来检查 slicer 是否就绪、工作区是否可用、索引是否已建。

**Approach:**

1. 新建 `api/health.rs`，定义 `HealthResponse` 结构体和 `health_handler` async 函数。
2. `health_handler` 从 `State<ApiAppState>` 获取 `WorkspaceService`，调用 `get_workspace_status()`。
3. 如果 workspace ready，用 `tokio::task::spawn_blocking` 调用 `SearchService::get_index_status()`。
4. 更新 `build_router(state: ApiAppState)` 注入 state，注册 `/health` 路由。
5. `ApiServerService` 内部存储 `ApiAppState`，`start()` 时传递给 `build_router()`。
6. 移除 `__api_alive` 占位路由和 handler。
7. 更新集成测试验证 `/health` 响应。

## Tasks & Acceptance

### Tasks

- [x] **T1: 新建 api/health.rs** (AC #1, #2, #3)
  - [x] 定义 `HealthResponse { api_version, workspace, index }`
  - [x] 实现 `health_handler(State<ApiAppState>) -> Result<ApiResponse<HealthResponse>, AppError>`
  - [x] workspace ready 时调用 `SearchService::get_index_status`（spawn_blocking）
  - [x] workspace 非 ready 时 index 为 None

- [x] **T2: 更新 api/server.rs** (AC #4, #5)
  - [x] `build_router(state: ApiAppState) -> Router`，注册 `/health` 路由
  - [x] 移除 `__api_alive` 占位路由
  - [x] `serve()` 接受 state 参数

- [x] **T3: 更新 ApiServerService** (AC #6)
  - [x] `ApiServerInner` 存储 `ApiAppState`
  - [x] `new(state: ApiAppState)` 构造函数
  - [x] `start()` 传递 state 给 `build_router()`

- [x] **T4: 更新 lib.rs 和调用方** (AC #7)
  - [x] 构造 `ApiAppState` 并传递给 `ApiServerService::new()`
  - [x] 更新所有测试中的 `ApiServerService::new()` 调用

- [x] **T5: 集成测试** (AC #8)
  - [x] 验证 `/health` 返回 200 + `{ "data": { "api_version", "workspace" } }`

- [x] **T6: cargo build + cargo test** (AC #9)
  - [x] 所有现有测试仍然通过 (105 tests)
  - [x] 集成测试通过 (1 test)

## Acceptance Criteria

1. `GET /health` 返回 200 OK。
2. 响应格式为 `{ "data": { "api_version": "0.1.0", "workspace": { "status": "..." }, "index": { ... } | null } }`。
3. workspace 未选中时，`index` 为 `null`，`workspace.status` 为 `"not_selected"`。
4. workspace ready 时，`index` 包含完整的 `IndexStatusDto`。
5. `__api_alive` 占位路由已移除。
6. `ApiServerService` 内部持有 `ApiAppState`，start/stop/reconcile 签名不变。
7. 所有现有测试和新增测试全部通过。
