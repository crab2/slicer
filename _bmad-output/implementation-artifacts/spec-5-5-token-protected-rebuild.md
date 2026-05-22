---
title: 'Story 5.5: POST /indexes/rebuild — token 保护与后台重建任务'
type: 'feature'
created: '2026-05-20'
status: 'done'
baseline_commit: 'b901e8f'
context: []
---

# Story 5.5: POST /indexes/rebuild — token 保护与后台重建任务

**Status:** done

## Story

As a 本地自动化用户,
I want 通过受 token 保护的 HTTP API 触发索引重建,
so that 我的脚本可以在安全控制下重建搜索索引。

## Scope Boundaries

**In scope (本故事必须完成):**

- `POST /indexes/rebuild` — 调用 `SearchService::rebuild_index`，返回 `{ "data": { "status": "rebuilding" } }`。
- Bearer token 认证：`Authorization: Bearer <token>` 请求头。
- `BearerAuth` extractor — 自定义 `FromRequestParts` 实现，验证 token 与 `ApiAppState` 中存储的 token 一致。
- 认证失败返回 401 + 详细错误码（`auth_state_poisoned`、`api_token_not_configured`、`missing_authorization`、`invalid_token`）。
- `reset_api_token` Tauri 命令 — 生成新 token 并返回。
- 应用启动时自动生成 UUID token 并存入 `ApiAppState`。

**Out of scope:**

- ❌ API contract tests（留给 5.6）。
- ❌ settings UI 可见性（留给 5.6）。

## Tasks & Acceptance

- [x] **T1: 新建 api/auth.rs** — `BearerAuth` extractor + `AuthErrorBody`/`AuthErrorDetail` DTO
- [x] **T2: 更新 api/state.rs** — 添加 `api_token: Arc<RwLock<Option<String>>>` 字段、`reset_token()` 方法
- [x] **T3: 新建 api/endpoints.rs** — 添加 `rebuild_index_handler`，使用 `BearerAuth` 保护
- [x] **T4: 更新 api/server.rs** — 注册 `POST /indexes/rebuild` 路由
- [x] **T5: 新建 commands/api_commands.rs** — 添加 `reset_api_token` Tauri 命令
- [x] **T6: 更新 lib.rs** — 启动时生成 token，管理 `ApiAppState` 为 Tauri state
- [x] **T7: 更新 api/mod.rs** — 导出 auth 模块
- [x] **T8: cargo build + cargo test** — 105 lib + 1 integration test pass

## Acceptance Criteria

1. `POST /indexes/rebuild` 无 token 返回 401 + `{ "error": { "code": "missing_authorization", ... } }`。
2. `POST /indexes/rebuild` 无效 token 返回 401 + `{ "error": { "code": "invalid_token", ... } }`。
3. `POST /indexes/rebuild` 有效 token 返回 200 + `{ "data": { "status": "rebuilding" } }`。
4. `reset_api_token` 命令返回新 token 字符串。
5. 应用启动时自动生成 UUID token 存入 `ApiAppState.api_token`。
6. `BearerAuth` 的 `AuthErrorBody`/`AuthErrorDetail` 类型为 `pub`（避免私有类型泄漏到公共接口）。

## Implementation Details

### 新增文件

**`src-tauri/src/api/auth.rs`** — Bearer token 认证 extractor
- `BearerAuth` — 空结构体，实现 `FromRequestParts<S>` where `ApiAppState: FromRef<S>`
- `AuthErrorBody` / `AuthErrorDetail` — 401 响应 DTO，实现 `IntoResponse`
- 从 `ApiAppState.api_token` 读取存储的 token 进行比较

**`src-tauri/src/api/endpoints.rs`** — 读取端点 + 索引重建
- `search_handler` — `GET /search?q=...&limit=...`
- `get_page_handler` — `GET /pages/{page_id}`
- `get_document_handler` — `GET /documents/{document_id}`
- `rebuild_index_handler` — `POST /indexes/rebuild`（需要 `BearerAuth`）

### 修改文件

**`src-tauri/src/api/state.rs`** — 添加 token 存储
- `api_token: Arc<RwLock<Option<String>>>` — 内存中的 token
- `reset_token()` — 生成 UUID v4 token 并写入
- `for_test()` — 测试辅助方法，创建临时工作区并生成 token

**`src-tauri/src/api/server.rs`** — 注册 rebuild 路由
- `POST /indexes/rebuild` → `rebuild_index_handler`

**`src-tauri/src/api/mod.rs`** — 添加 `pub mod auth;`

**`src-tauri/src/commands/api_commands.rs`** — 新增 `reset_api_token` 命令

**`src-tauri/src/lib.rs`** — 启动时初始化 token
- 创建 `ApiAppState`，调用 `reset_token()`
- 将 `ApiAppState` 和 `ApiServerService` 都 manage 为 Tauri state

### 关键设计决策

1. **Token 存储位置**：`ApiAppState` 同时作为 axum router state 和 Tauri managed state，确保 token 在 HTTP handler 和 Tauri 命令间共享。
2. **Token 生成**：使用 `uuid::Uuid::new_v4()` 生成，每次应用启动或调用 `reset_api_token` 时重新生成。
3. **Auth extractor**：使用 axum 的 `FromRequestParts` trait，从 `ApiAppState` 中读取 token 进行验证。
4. **错误类型可见性**：`AuthErrorBody`/`AuthErrorDetail` 必须为 `pub`，否则 `FromRequestParts` 的 `type Rejection` 会暴露私有类型。
