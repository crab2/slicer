---
title: 'Story 5.2: 统一 HTTP DTO 成功响应与 AppError 错误映射'
type: 'feature'
created: '2026-05-20'
status: 'done'
baseline_commit: 'b901e8f'
context: []
---

# Story 5.2: 统一 HTTP DTO 成功响应与 AppError 错误映射

**Status:** done

## Story

As a 本地自动化用户,
I want HTTP API 返回统一结构的成功和错误响应,
so that 我的脚本可以用一致的方式解析所有 API 返回。

## Scope Boundaries

**In scope (本故事必须完成):**

- 新建 `api/dto.rs`，定义 `ApiResponse<T>` 统一成功包装 `{ "data": T }`。
- 定义 `ApiErrorBody` 结构体，序列化为 `{ "error": { "code", "message", "stage", "retryable", "details", "correlation_id" } }`。
- 实现 `AppError` → `axum::http::StatusCode` 映射（按 error code 映射 HTTP 状态码）。
- 实现 `axum::IntoResponse` for `AppError`，使 handler 可以直接 `?` 返回 `AppError`。
- 实现 `axum::extract::FromRequestParts` 或 `axum::response::IntoResponse` for `ApiResponse<T>`，使 handler 可以返回 `ApiResponse::ok(data)`。
- 更新 `api/server.rs` 的 `build_router()`，将 `ApiAppState` 注入为 axum state。
- 更新占位路由 `GET /__api_alive` 使用新的 DTO（或保留 204，由 5.3 替换）。
- 新增 `api/mod.rs` 导出 `dto` 模块。
- 单元测试：`ApiResponse` 序列化、`AppError` → StatusCode 映射、`ApiErrorBody` 序列化。

**Out of scope (本故事禁止实现 — 留给后续 Story):**

- ❌ `GET /health`、`GET /search` 等业务 endpoint（留给 5.3、5.4）。
- ❌ token 认证中间件（留给 5.5）。
- ❌ API contract tests 全套覆盖（留给 5.6）。
- ❌ 前端 DTO 类型同步（留给 5.6）。

## Intent

**Problem:** Story 5.1 建立了 axum server 基础，但 router 只有一个返回 204 的占位路由，没有统一的响应格式。后续 5.3–5.5 的每个 handler 都需要返回一致的 JSON 结构，且 `AppError` 需要自动映射为合适的 HTTP 状态码和 JSON 错误体。

**Approach:**

1. 新建 `api/dto.rs`，定义 `ApiResponse<T: Serialize>` 包装类型，提供 `ApiResponse::ok(data)` 和 `ApiResponse::created(data)` 工厂方法。
2. 定义 `ApiErrorBody` 包装类型，序列化为 `{ "error": { ... } }`，字段直接来自 `AppError`。
3. 为 `AppError` 实现 `axum::response::IntoResponse`：根据 `code` 映射 HTTP 状态码（`api_server_*` → 503，`not_found` → 404，`validation_*` → 400，默认 500），body 为 `ApiErrorBody` JSON。
4. 为 `ApiResponse<T>` 实现 `IntoResponse`：状态码由构造时指定（默认 200），body 为 `{ "data": T }` JSON。
5. 更新 `build_router()` 将 `ApiAppState` 注入 `.with_state()`，为 5.3+ 的 handler 铺路。
6. 添加单元测试覆盖序列化格式和状态码映射。

## Boundaries & Constraints

**Always:**

- 成功响应必须是 `{ "data": ... }`，错误响应必须是 `{ "error": { ... } }`。
- `AppError` 的 `correlation_id` 必须出现在错误响应中。
- `details` 字段如果为 `None`，在 JSON 中必须省略（`#[serde(skip_serializing_if = "Option::is_none")]`）。
- 状态码映射必须可扩展（用 `match` 而非硬编码数字）。
- `ApiResponse` 和 `ApiErrorBody` 必须是纯数据类型，不持有引用（`'static` bounds）。

**Ask First:**

- 无。

**Never:**

- ❌ 不在 `ApiResponse` 中包含 `status` 或 `success` 字段 — 外层 HTTP status code 足够。
- ❌ 不在错误响应中暴露内部细节（如 stack trace、SQL 语句）。
- ❌ 不修改 `errors.rs` 中 `AppError` 的现有结构或行为。
- ❌ 不让 `__api_alive` 占位路由返回 DTO 包装（保留 204 No Content，由 5.3 替换）。

## Code Map

### Files that will be CREATED

- `src-tauri/src/api/dto.rs` — `ApiResponse<T>`、`ApiErrorBody`、`AppErrorStatusCode` 映射

### Files that will be UPDATED (must read fully before editing)

- `src-tauri/src/api/mod.rs` — 添加 `pub mod dto;`
- `src-tauri/src/api/server.rs` — `build_router()` 改为 `Router::new().with_state(ApiAppState::new(workspace))`（或仅 `Router::new()` 如果 state 由调用方注入）；添加 `use tower-http` 如需 CORS

### Files that MUST NOT be modified

- `src-tauri/src/errors.rs` — `AppError` 结构不变
- `src-tauri/src/services/api_server_service.rs` — server lifecycle 不变
- `src-tauri/migrations/*.sql` — 无新表

## Tasks & Acceptance

### Tasks (顺序执行 — 后一步依赖前一步)

- [x] **T1: 新建 api/dto.rs** (AC #1, #2, #3)
  - [x] 定义 `ApiResponse<T: Serialize>`，字段 `data: T`，提供 `ok(data)` → 200 和 `created(data)` → 201
  - [x] 定义 `ApiErrorBody`，字段 `error: AppErrorDetail`，`AppErrorDetail` 包含 `code`, `message`, `stage`, `retryable`, `details?`, `correlation_id`
  - [x] 实现 `IntoResponse for ApiResponse<T>`：状态码 + JSON body
  - [x] 实现 `IntoResponse for AppError`：按 code 映射 StatusCode + `ApiErrorBody` JSON
  - [x] 状态码映射：`api_server_*` → 503, `not_found*` → 404, `validation_*` → 400, 默认 500

- [x] **T2: 更新 api/mod.rs** (AC #4)
  - [x] 添加 `pub mod dto;`

- [x] **T3: 更新 api/server.rs** (AC #5)
  - [x] `build_router()` 保持无状态（5.3 引入业务 handler 时再注入 `ApiAppState`）
  - [x] 占位路由 `__api_alive` 保留 204 No Content，不受 DTO 影响

- [x] **T4: 单元测试** (AC #6)
  - [x] `ApiResponse::ok` 序列化为 `{"data": ...}`
  - [x] `ApiResponse::created` 返回 201
  - [x] `AppError` → 503 for `api_server_*` codes
  - [x] `AppError` → 404 for `not_found` codes
  - [x] `AppError` → 400 for `validation_*` codes
  - [x] `AppError` → 500 for unknown codes
  - [x] `ApiErrorBody` 序列化中 `details: None` 被省略

- [x] **T5: cargo build + cargo test** (AC #7)
  - [x] 所有现有测试仍然通过 (105 tests)
  - [x] 新增测试全部通过 (8 tests)

## Acceptance Criteria

1. `GET /__api_alive` 仍然返回 204 No Content（不受本故事影响）。
2. 新增 `ApiResponse<T>` 和 `ApiErrorBody` 类型可编译且序列化格式正确。
3. `AppError` 可直接在 axum handler 中通过 `?` 返回，自动映射为正确 HTTP 状态码和 JSON 错误体。
4. 错误响应格式为 `{ "error": { "code": "...", "message": "...", "stage": "...", "retryable": true/false, "correlation_id": "..." } }`，`details` 为 `null` 时省略。
5. 成功响应格式为 `{ "data": ... }`。
6. 所有新增代码有单元测试覆盖。
7. `cargo build` 和 `cargo test` 全部通过。
