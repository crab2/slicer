---
title: 'Story 5.1: 嵌入式 Axum Localhost API 服务基础与设置控制'
type: 'feature'
created: '2026-05-20'
status: 'review'
baseline_commit: 'b901e8f'
context: []
---

# Story 5.1: 嵌入式 Axum Localhost API 服务基础与设置控制

**Status:** review

## Story

As a 本地自动化用户,
I want slicer 提供默认仅监听本机地址的 HTTP API 服务,
so that 我可以从本机脚本或工具安全地访问页面、搜索和索引能力。

## Scope Boundaries

**In scope (本故事必须完成):**

- 引入 `axum` 与 `tokio` runtime,在应用进程内嵌入 HTTP server。
- 新建 Rust 模块 `api/`(`api/mod.rs`、`api/server.rs`、`api/state.rs`),以及 `services/api_server_service.rs`。
- `ApiServerService` 提供 `start`、`stop`、`get_runtime_status` 三个能力,负责按 settings 启动/停止 server。
- 启动失败(端口占用、bind 不可用、绑定地址非 `127.0.0.1`)统一映射为 `AppError`。
- 新增 1 个临时占位路由 `GET /__api_alive`(返回 204 No Content,**不属于** Epic 5 业务 endpoints,仅用于验证 server 起停)。
- `tauri::Builder::default().setup(...)` 中根据当前工作区与 settings 决定是否启动 API。
- 设置页 `ApiServerSettings` 区域:展示 enabled/bind/port、保存设置后联动启停、显示运行时实际状态(running / stopped / failed)。
- 添加 `get_api_server_status` Tauri command。
- token reset action **保留按钮占位**(disabled),不实现 token 生成逻辑(留给 Story 5.5)。
- 切换工作区或禁用 API 时优雅关闭已运行的 server。

**Out of scope (本故事禁止实现 — 留给后续 Story):**

- ❌ `GET /health`、`GET /search`、`GET /pages/{page_id}`、`GET /documents/{document_id}` 任何**业务返回**(留给 5.3、5.4)。
- ❌ 统一 `{ "data": ... }` / `{ "error": ... }` DTO 包装层(留给 5.2)。
- ❌ `POST /indexes/rebuild` 与 token 校验中间件(留给 5.5)。
- ❌ API contract tests 全套覆盖(留给 5.6)。
- ❌ 共享 `services/` 调用(本故事 server 几乎不调任何 service,只验证起停与配置流)。

## Intent

**Problem:** Epic 1–4 已建立完整的 GUI、SQLite ledger、Job Orchestrator、Search/Index 能力。Story 5.1 需要为后续 4 个 HTTP endpoint(5.3–5.5)与统一 DTO 契约(5.2)铺一层"可启动、可停止、可配置、默认安全"的 axum server 基础。当前代码完全没有 `api/` 目录、`axum` 依赖或 HTTP runtime,设置页 ApiServer 区域也只是表单字段,保存后并不真的启动服务。

**Approach:**

1. 在 `Cargo.toml` 增加 `axum = "0.8"` 与 `tokio = { version = "1", features = ["macros", "rt-multi-thread", "net", "signal"] }`,沿用项目现有 `runtime-tokio-rustls` 后端。
2. 新建 `src-tauri/src/api/` 模块(`mod.rs`、`server.rs`、`state.rs`),`server.rs` 负责构造 `axum::Router`(只有占位 `GET /__api_alive`),`state.rs` 暴露未来 routes 共享的 `ApiAppState { workspace: WorkspaceService }` 类型(为 5.2+ 预备,本故事不会被 router 实际使用)。
3. 新建 `services/api_server_service.rs`,内部用 `Mutex<Option<RunningServer>>` 持有 tokio runtime handle 与 shutdown channel,提供 `start(workspace, settings)`、`stop()`、`get_runtime_status()` 同步接口。
4. `lib.rs` 用 `tauri::Builder::default().setup(...)` 在应用启动后读取 settings,如 `api_enabled = true` 则调用 `ApiServerService::start`;否则保持停止。注册新 command `get_api_server_status`。
5. `SettingsService::save_settings` 完成后,触发 `ApiServerService::reconcile(...)`(根据新旧 settings 决定 start/stop/restart);`select_workspace` 切换工作区时也走同一 reconcile 路径(切换时一律先 stop 旧 server)。
6. 前端新建 `src/features/settings/components/ApiServerSettings.tsx`(从 `SettingsPage.tsx` 现有"localhost API"区块抽出),增加 `runtime_status` chip,绑定 `useApiServerRuntimeStatus` hook 轮询 `get_api_server_status`。
7. `validate_settings` 中已有的 "API 当前仅允许监听 127.0.0.1" 规则保留,不放宽。

## Boundaries & Constraints

**Always:**

- 默认 bind `127.0.0.1`,`validate_settings` 已禁止其他地址(包括 `0.0.0.0`、`::`、`localhost` 以外的字面量);本故事**不得放宽**该校验。
- 业务逻辑只能在 `services/`,本故事新建的 axum handler `__api_alive` 是唯一例外(仅返回静态 204,不调用任何 service)。
- 启停由 `ApiServerService` 集中管理,不得在 `commands/`、`SettingsService::save_settings` 内直接 `tokio::spawn` server task。
- 端口冲突、bind 失败必须映射为 `AppError`,`code` 形如 `api_server_bind_failed`、`api_server_port_in_use`、`api_server_already_running`。
- DTO 字段使用 `snake_case`,`runtime_status` 取值集合:`running`、`stopped`、`failed`、`disabled`(`disabled` 表示 settings 未启用)。
- 切换工作区或修改 API settings 时,server 必须能优雅关闭(用 `tokio::sync::oneshot` 触发 `axum::serve(...).with_graceful_shutdown(...)`)。
- `tracing::info!` 必须记录起停事件,**不得**记录任何 token、API key 或完整 URL 中的 query string;只允许打印 `bind_address` 与 `port`。

**Ask First:**

- axum 0.8 vs 0.7 选择(决定:0.8,与 tokio 1.x 一致,且 0.8 已稳定数月)。
- 是否需要 `tauri::async_runtime::spawn` 还是独立 tokio runtime(决定:复用 `tauri::async_runtime`,Tauri 2 默认就是 tokio,避免双 runtime)。

**Never:**

- ❌ 不实现 token 生成、校验或受保护 endpoint(Story 5.5)。
- ❌ 不实现统一 `{ "data": ... }` 响应包装(Story 5.2)。
- ❌ 不让任何 endpoint 返回真实业务数据 — 占位路由必须返回 `204 No Content`,空 body。
- ❌ 不在 `app-settings.json`(全局)写入 API 启停状态;API enable/bind/port 已经是 **workspace-scoped** settings(见 `WorkspaceSettingsRecord`),保持不变。
- ❌ 不引入新的 HTTP DTO 包装类型(Story 5.2 才负责)。
- ❌ 不允许在测试以外绕过 `validate_settings` 直接创建 `0.0.0.0` 配置。

## Code Map

### Files that will be CREATED

- `src-tauri/src/api/mod.rs` — 模块入口,`pub mod server; pub mod state;`
- `src-tauri/src/api/server.rs` — `build_router()`、`serve_with_shutdown(addr, shutdown_rx)` 函数
- `src-tauri/src/api/state.rs` — `ApiAppState { workspace: WorkspaceService }`(预备给 5.2+,本故事仅定义类型)
- `src-tauri/src/services/api_server_service.rs` — `ApiServerService` 单例,封装起停状态机
- `src-tauri/src/commands/api_commands.rs` — Tauri command `get_api_server_status`
- `src/features/settings/components/ApiServerSettings.tsx` — 从 `SettingsPage.tsx` 抽出的 API 配置区块组件
- `src-tauri/tests/api_server_lifecycle_tests.rs` — 集成测试:start → __api_alive 200 → stop → 端口释放

### Files that will be UPDATED (must read fully before editing)

- `src-tauri/Cargo.toml` — 添加 `axum`、`tokio` 显式 features
- `src-tauri/src/lib.rs` — 注册新模块 `mod api;`、`mod commands::api_commands;`、`.manage(ApiServerService::new())`、`.setup(...)` 启动钩子、`invoke_handler` 注册 `get_api_server_status`
- `src-tauri/src/services/mod.rs` — `pub mod api_server_service;`
- `src-tauri/src/commands/mod.rs` — `pub mod api_commands;`
- `src-tauri/src/services/settings_service.rs` — `save_settings` 末尾追加 `ApiServerService::reconcile(workspace, &normalized_settings)`(参数签名需要新接收 `&WorkspaceService`,目前已有);**注意**:reconcile 的参数应是新保存的 `WorkspaceSettingsRecord` 派生的 `AppSettingsDto`,不能传未校验值
- `src-tauri/src/services/workspace_service.rs` — `select_workspace` 成功后调用 `ApiServerService::reconcile_for_new_workspace(...)`(先 stop,再视新工作区 settings 决定是否 start)
- `src-tauri/src/domain/settings.rs` — 新增 `ApiServerStatusDto { runtime_status: String, bind_address: String, port: u16, enabled: bool, last_error: Option<AppError> }`
- `src-tauri/src/repositories/db.rs` — 无变化(无新表)
- `src-tauri/src/commands/settings_commands.rs` — 无变化
- `src/features/settings/SettingsPage.tsx` — 移除内联的 "localhost API" `<section>`,改为 `<ApiServerSettings ... />`;接收 `runtimeStatus` prop
- `src/types/app.ts` — 添加 `ApiServerStatusDto` 与 `ApiServerRuntimeStatus` 类型
- `src/lib/tauriClient.ts` — 添加 `getApiServerStatus`
- `src/styles/globals.css` — 如需要 `runtime-status-chip` 配色(running 绿,failed 红,disabled 灰),复用 `StatusBadge` tone 即可,**不必**新建 CSS

### Files that MUST NOT be modified

- `src-tauri/migrations/*.sql` — 本故事不引入新表/新列
- `src-tauri/src/services/search_service.rs`、`analysis_service.rs`、`import_service.rs` — 业务 service 不变
- 任何 Epic 1–4 已 done 的故事文件

## Tasks & Acceptance

### Tasks (顺序执行 — 后一步依赖前一步)

- [x] **T1: 引入依赖** (AC #1)
  - [x] `src-tauri/Cargo.toml` 添加 `axum = "0.8"`,显式扩充 `tokio = { version = "1", features = ["macros", "rt-multi-thread", "net", "sync", "signal"] }`(项目目前未直接声明 tokio,而是通过 sqlx 与 tauri 间接依赖;此处显式声明以使用 `tokio::sync::oneshot`)
  - [x] `cargo build` 必须通过

- [x] **T2: api 模块骨架** (AC #1, #6)
  - [x] 新建 `src-tauri/src/api/mod.rs`、`api/server.rs`、`api/state.rs`
  - [x] `server.rs` 暴露 `pub fn build_router() -> axum::Router`,内部仅注册 `GET /__api_alive` → 返回 `axum::http::StatusCode::NO_CONTENT`
  - [x] `server.rs` 暴露 `pub async fn serve(addr: SocketAddr, shutdown: oneshot::Receiver<()>) -> AppResult<()>`,使用 `tokio::net::TcpListener::bind(addr).await` + `axum::serve(listener, router).with_graceful_shutdown(async move { let _ = shutdown.await; })`
  - [x] `state.rs` 定义 `pub struct ApiAppState { pub workspace: WorkspaceService }`(本故事不被 router 使用,但需在编译期可见,为 5.2 预备)

- [x] **T3: ApiServerService 状态机** (AC #1, #4, #5, #6)
  - [x] 新建 `services/api_server_service.rs`,持有 `Arc<Mutex<ApiServerInner>>`
  - [x] `ApiServerInner` 字段:`shutdown_tx: Option<oneshot::Sender<()>>`、`bind_address: String`、`port: u16`、`last_error: Option<AppError>`、`status: ApiRuntimeStatus`
  - [x] `pub fn start(&self, settings: &AppSettingsDto) -> AppResult<()>`:校验 `api_enabled=true`,bind `127.0.0.1:<port>`,失败时填充 `last_error` 并设 `status=failed` 返回 `AppError`
  - [x] `pub fn stop(&self) -> AppResult<()>`:发送 shutdown 信号,join task,设 `status=stopped`
  - [x] `pub fn reconcile(&self, settings: &AppSettingsDto)`:幂等 — 期望状态与当前一致直接返回;`enabled` 变化或 `port`/`bind_address` 变化触发 stop+start
  - [x] `pub fn get_runtime_status(&self) -> ApiServerStatusDto`
  - [x] **错误码集合(必须全部实现)**:
    - `api_server_already_running`(`start` 时已经在跑)
    - `api_server_port_in_use`(`io::ErrorKind::AddrInUse`)
    - `api_server_bind_failed`(其他 bind 错误)
    - `api_server_disabled`(`enabled=false` 时却调 `start`)
    - `api_server_state_poisoned`(Mutex poison)

- [x] **T4: settings_service 与 workspace_service 集成** (AC #3, #5)
  - [x] `SettingsService::save_settings`:在持久化成功后调 `ApiServerService::reconcile(&normalized_settings)`;若 reconcile 失败,**不回滚** settings 持久化,但返回 `AppError`(用户在前端会看到设置已保存但 API 启动失败)
  - [x] `WorkspaceService::select_workspace`:在 `current` 更新成功后,从新工作区加载 settings 并调 `ApiServerService::reconcile`;失败仅 log warn,不阻断工作区切换
  - [x] `ApiServerService` 通过 Tauri `State` 注入,`SettingsService::save_settings` 与 `select_workspace` 都需要新参数 `api_server: &ApiServerService`(更新所有调用点)

- [x] **T5: Tauri command 与 lib.rs 注册** (AC #1, #2, #5)
  - [x] `commands/api_commands.rs`:`#[tauri::command] pub fn get_api_server_status(api_server: State<'_, ApiServerService>) -> ApiServerStatusDto`
  - [x] `lib.rs`:
    - `mod api;`
    - `use commands::api_commands::get_api_server_status;`
    - `.manage(ApiServerService::new())`
    - `.setup(|app| { ... })` 中:从 `app.state::<WorkspaceService>()` 读 settings,若 `api_enabled` 则调 `api_server.start(&settings)`,失败仅 `tracing::warn!`(不阻断应用启动)
    - `invoke_handler` 增加 `get_api_server_status`
  - [x] **注意**:`ApiServerService::new()` 必须不启动 server — 启动决策只在 `setup` 钩子或 `reconcile` 中

- [x] **T6: 前端类型与客户端** (AC #2)
  - [x] `src/types/app.ts` 添加:
    ```ts
    export type ApiServerRuntimeStatus = "running" | "stopped" | "failed" | "disabled";
    export interface ApiServerStatusDto {
      runtime_status: ApiServerRuntimeStatus;
      bind_address: string;
      port: number;
      enabled: boolean;
      last_error: AppErrorDto | null;
    }
    ```
  - [x] `src/lib/tauriClient.ts` 添加 `getApiServerStatus: () => callTauriCommand<ApiServerStatusDto>("get_api_server_status")`

- [x] **T7: 前端组件抽离与 runtime status 展示** (AC #2)
  - [x] 新建 `src/features/settings/components/ApiServerSettings.tsx`,接收 props:`{ settings, onUpdateField, runtimeStatus, isLoading }`
  - [x] 渲染当前内联区块的全部字段(enabled/bind/port),**额外**展示 `runtime_status` chip 与 last_error message + correlation_id(若有)
  - [x] 添加 `token reset` 按钮 — `disabled` 状态,`title="将在 Story 5.5 中启用"`
  - [x] `SettingsPage.tsx` 用 `useEffect` + `setInterval(2000ms)` 轮询 `tauriClient.getApiServerStatus()`,组件卸载或工作区切换时清理
  - [x] 替换原 "localhost API" 内联 section 为 `<ApiServerSettings ... runtimeStatus={runtimeStatus} />`

- [x] **T8: 测试覆盖** (AC #1, #4, #5)
  - [x] **单元测试** (`#[cfg(test)]` in `api_server_service.rs`):
    - `start_then_stop_releases_port`:启动 → 用 `TcpStream::connect` 验证端口在监听 → stop → 验证 connect 失败
    - `start_when_already_running_returns_error`:连续两次 start 第二次返回 `api_server_already_running`
    - `start_with_disabled_returns_error`:`api_enabled=false` 时 start 返回 `api_server_disabled`
    - `reconcile_is_idempotent`:同一 settings 连续 reconcile 不应重启 server(通过观察 PID/start_time 验证)
    - `reconcile_changes_port_restarts_server`:改 port 后旧端口释放,新端口可连接
    - `port_in_use_maps_to_specific_error_code`:占用端口后 start 返回 `api_server_port_in_use`(用 `tokio::net::TcpListener::bind` 提前占住测试端口)
  - [x] **集成测试** (`src-tauri/tests/api_server_lifecycle_tests.rs`):
    - 启动 server → `reqwest::blocking::get("http://127.0.0.1:<port>/__api_alive")` 返回 204 → stop → 同一 GET 返回连接错误
  - [x] **运行**:`cargo test --lib`(应保持 73+ 通过 + 新增 ~7 个)、`cargo test --test api_server_lifecycle_tests`、`npx tsc --noEmit`

- [x] **T9: 占位路由验证与文档化** (AC #6)
  - [x] 在 `api/server.rs` 顶部加 1 行注释:`// Placeholder until Story 5.2 introduces the unified DTO contract.`(这是允许的注释,因为它解释了为什么文件几乎是空的)
  - [x] 不要写"用法说明"或"未来如何扩展"的多行 doc comment

### Acceptance Criteria

#### AC #1 — 用户启用 API 后嵌入式 axum 在 127.0.0.1 启动

- **Given** 用户已选择可用工作区且 SQLite 账本已初始化
- **When** 用户在设置页将 `api_enabled` 切到 true 并保存
- **Then** `ApiServerService::reconcile` 调用 `start`,axum server 在 `127.0.0.1:<api_port>` 上监听
- **And** `GET http://127.0.0.1:<port>/__api_alive` 返回 `204 No Content`
- **And** `validate_settings` 拒绝 `api_bind_address != "127.0.0.1"` 的保存请求

#### AC #2 — 设置页展示运行时状态

- **Given** API 已启用且正在运行
- **When** 用户查看设置页 ApiServerSettings 区域
- **Then** 区域显示 `runtime_status=running` chip(success tone)、bind address `127.0.0.1`、port `<api_port>`
- **And** token reset 按钮可见但 disabled,鼠标悬停提示"将在 Story 5.5 中启用"
- **And** 当 API 禁用时显示 `disabled` chip(neutral tone),启动失败时显示 `failed` chip(warning tone)与 `last_error.message + correlation_id`
- **And** 不展示完整 token、API key 或其他 secret

#### AC #3 — 设置变化优雅重启 server

- **Given** API 当前在 port 17321 运行
- **When** 用户修改 `api_port` 到 17322 并保存
- **Then** `ApiServerService::reconcile` 检测到 port 变化,先 stop(发送 shutdown,等 task 退出),再用新 port start
- **And** 旧端口 17321 不再可连接,新端口 17322 可连接 `__api_alive`
- **And** 同一 settings 重复保存不应触发 stop+start(reconcile 必须幂等)

#### AC #4 — 启动失败映射为 AppError

- **Given** 端口 17321 已被另一进程占用
- **When** 用户启用 API 并保存设置
- **Then** `start` 返回 `AppError { code: "api_server_port_in_use", retryable: true, stage: "api", correlation_id: ... }`
- **And** `runtime_status=failed`,`last_error` 暴露给前端
- **And** 用户在设置页看到中文摘要"localhost API 端口已被占用,请更换端口或释放该端口后重试。"
- **And** `correlation_id` 不出现在错误以外的位置(无泄露)

#### AC #5 — 切换工作区或禁用 API 时 server 优雅关闭

- **Given** API 已在 workspace A 上运行
- **When** 用户调用 `select_workspace(B)` 切到 workspace B
- **Then** 旧 server 收到 shutdown,task 退出,端口释放
- **And** 若 workspace B settings 中 `api_enabled=true`,server 在新 settings 的 port 上重启;若 false,保持 stopped
- **And** 工作区切换不会因为 API stop 失败而被阻断(stop 失败仅 `tracing::warn!`)

#### AC #6 — 模块边界与单一职责

- **Given** 开发者审阅 `src-tauri/src/api/`、`src-tauri/src/services/api_server_service.rs`
- **Then** `api/server.rs` 只构造 router 与 listener,不调任何业务 service
- **And** `api/server.rs` 内**不存在**业务 endpoint(`/health`、`/search`、`/pages/...`、`/documents/...`、`/indexes/rebuild`)
- **And** `ApiServerService` 是唯一持有 server lifecycle 状态的位置(命令层与 settings 层都通过它)
- **And** `ApiServerService::new()` 不启动 server

## Dev Notes

### Architecture Compliance (架构强约束)

来自 `_bmad-output/planning-artifacts/architecture.md` §"API & Communication Patterns":

> **API Boundaries**:
> - `src-tauri/src/api/` 拥有 localhost HTTP API
> - HTTP handlers 必须调用 `services/`;不得直接访问 repositories、providers、artifact store、SQLite 或文件系统 helper
> - API authentication/token handling 位于 `api/auth.rs` 与 `security/api_token.rs`(本故事**不创建** auth.rs,留给 5.5)
> - HTTP DTOs 位于 `api/dto.rs`,与数据库 row struct 分离(本故事**不创建** dto.rs,留给 5.2)

来自 architecture.md §"Tauri Command Boundaries":

> Commands 不得 spawn LibreOffice、写 SQLite、修改索引文件、调用 model APIs。

→ 本故事的 `get_api_server_status` command 严格遵循:它只调 `ApiServerService::get_runtime_status()`。

### Previous Story Intelligence (Story 1.4 / 1.5 / 4.7 经验)

- **Story 1.4 经验**:`recover_interrupted_jobs` 单循环非原子。本故事的 `ApiServerService` 也用 Mutex,但只承载单个 server 实例,**不会**有"部分恢复"问题 — 但要小心 Mutex poison(`PoisonError`),需映射为 `api_server_state_poisoned`。
- **Story 1.5 经验**:`AppError::with_details` 内部已自动调 `redact_secrets`。本故事写 `last_error` 时直接构造 `AppError` 即可,redaction 自动完成。
- **Story 4.7 经验**:索引重建任务前端用 2 秒轮询 + 工作区切换时清理。本故事 ApiServerSettings 复用同一模式。

### Async Runtime 集成

Tauri 2 默认使用 `tauri::async_runtime`,底层就是 tokio。本故事**不要**新建独立 tokio runtime — 用 `tauri::async_runtime::spawn` 在 Tauri runtime 上启动 server task:

```rust
// 伪代码,实际写在 ApiServerService::start 内
let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
let addr = format!("{}:{}", settings.api_bind_address, settings.api_port).parse::<SocketAddr>()?;
tauri::async_runtime::spawn(async move {
    if let Err(err) = api::server::serve(addr, shutdown_rx).await {
        tracing::error!(target: "api", code = %err.code, "API server task exited with error");
    }
});
```

### redact_secrets 已经覆盖的字段

`src-tauri/src/errors.rs::ASSIGNMENT_SECRET_KEYS` 已经覆盖 `api_key`、`authorization`、`token`、`secret` 等。**不要**重新发明 redaction — 直接用 `AppError::with_details`。

### Settings 持久化路径

`api_enabled`、`api_bind_address`、`api_port` 是 **workspace-scoped** — 走 `WorkspaceSettingsRepository`(SQLite `settings` 表 key=`app_settings`),不进 `app-settings.json`。本故事不改这个分布,只读它。

### 关键不变量

1. **bind 安全**:任何代码路径都不能让 axum 监听非 `127.0.0.1` 地址。`validate_settings` 是入口拦截;`ApiServerService::start` 内部**额外**断言 `bind_address == "127.0.0.1"`(纵深防御)。
2. **状态单一来源**:`ApiServerService` 是 runtime status 的唯一源;前端轮询它,不要从 settings DTO 推断 `runtime_status`(settings 只表达"用户期望"是否启用,server 可能因 bind 失败处于 `failed`)。
3. **`__api_alive` 路由仅本故事临时存在**:Story 5.3 实现 `GET /health` 后,`__api_alive` 应被替换或删除。本故事不删,留作过渡。

### File Naming 与 Module Layout 强约束

- `api/server.rs`、`api/state.rs`、`api/mod.rs` — 与 architecture.md `项目目录结构` 完全一致
- `services/api_server_service.rs` — 与 architecture.md 一致
- ❌ 不要用 `api/setup.rs`、`api/lifecycle.rs`、`api_service.rs` 等替代命名

### 测试策略

- **不要**让单元测试硬编码端口 17321 — 用 OS 分配端口(`bind 127.0.0.1:0` 后读取实际端口)避免 CI 端口冲突
- 集成测试(`src-tauri/tests/`)允许使用动态端口或固定测试端口(如 17399),但要在 setup/teardown 释放
- mock 不需要:`ApiServerService` 直接对 axum + tokio 集成测试比 mock 更可靠

## Verification

**Commands (按顺序运行):**

- `cargo build` — 期望:axum/tokio 依赖正确解析,无 unused warning
- `cargo test --lib` — 期望:Story 1.4–4.7 现有 73 个测试 + 新增 ~6 个 ApiServerService 测试,全部通过
- `cargo test --test api_server_lifecycle_tests` — 期望:集成测试通过
- `npx tsc --noEmit` — 期望:`ApiServerStatusDto`、`ApiServerRuntimeStatus`、`getApiServerStatus` 类型检查通过
- `npx vite build` — 期望:前端构建通过

**Manual smoke (开发完成后必须人工验证):**

- 启动 `npm run tauri dev`,选择工作区,在设置页启用 API → 浏览器或 curl `http://127.0.0.1:17321/__api_alive` → 应返回 204
- 关闭 API → 同一 URL 应连接拒绝
- 修改 port → 旧 port 不通,新 port 通
- 占用 port 后启用 → 设置页显示 `failed` chip + 中文错误摘要 + correlation_id

## Suggested Review Order

**Dependencies & Boundaries**

- 新增 axum/tokio 依赖
  [`Cargo.toml`](../../src-tauri/Cargo.toml#L20)

**API Module Skeleton**

- Router 与占位路由
  [`api/server.rs`](../../src-tauri/src/api/server.rs)

- Future-state placeholder
  [`api/state.rs`](../../src-tauri/src/api/state.rs)

**Lifecycle State Machine**

- start/stop/reconcile/get_runtime_status,错误码完整性
  [`services/api_server_service.rs`](../../src-tauri/src/services/api_server_service.rs)

**Integration Points**

- settings 保存后 reconcile
  [`services/settings_service.rs`](../../src-tauri/src/services/settings_service.rs#L36)

- 工作区切换后 reconcile
  [`services/workspace_service.rs`](../../src-tauri/src/services/workspace_service.rs#L68)

- Tauri setup 钩子启动逻辑
  [`lib.rs`](../../src-tauri/src/lib.rs#L31)

**Frontend Surface**

- ApiServerSettings 组件抽离与 runtime_status chip
  [`features/settings/components/ApiServerSettings.tsx`](../../src/features/settings/components/ApiServerSettings.tsx)

- SettingsPage 接入轮询
  [`features/settings/SettingsPage.tsx`](../../src/features/settings/SettingsPage.tsx#L326)

- DTO 类型与 client 方法
  [`types/app.ts`](../../src/types/app.ts#L218)

**Verification**

- 单元测试:start/stop/reconcile/error-mapping
  [`services/api_server_service.rs`](../../src-tauri/src/services/api_server_service.rs)

- 集成测试:lifecycle + 真实 HTTP 客户端
  [`tests/api_server_lifecycle_tests.rs`](../../src-tauri/tests/api_server_lifecycle_tests.rs)

## Dev Agent Record

### Agent Model Used

claude-opus-4-7[1m] (Claude Code dev agent)

### Debug Log References

- 全程无 HALT。`cargo test`(同时跑 lib + integration)首次跑出现 `failed_analysis_exports_baseline_not_raw_result` 偶发失败,根因是该 pre-existing 测试在 `test_connection()` 用 `SystemTime::now().as_nanos()` 拼临时路径,在并行进程下可能撞名导致 `schema_migrations.version UNIQUE` 冲突;重跑通过。该问题在本故事改动范围之外,不在 5.1 修复。
- `cargo test --lib` 单独跑:81/81 通过(基线 73 + 本故事新增 8)。
- `cargo test --test api_server_lifecycle_tests`:1/1 通过。
- `npx tsc --noEmit`:无报错。
- `npx vite build`:53 modules transformed,success。

### Completion Notes List

- 引入 `axum 0.8` 与显式 `tokio 1`(`macros, rt-multi-thread, net, sync, signal`),不动 sqlx 的 `runtime-tokio-rustls`。
- 新建 `api/` 模块:`server.rs` 仅注册 `GET /__api_alive` → 204,`state.rs` 预备 `ApiAppState` 类型(标记 `#[allow(dead_code)]`,本故事不接 router)。`server.rs` 顶部加上 placeholder 注释。
- `ApiServerService`:`Mutex<ApiServerInner>` 单实例,通过 `tauri::async_runtime::block_on` 同步预绑定 `tokio::net::TcpListener` 检测 `AddrInUse` → 映射为 `api_server_port_in_use`,然后 `tauri::async_runtime::spawn` 把 `axum::serve(...).with_graceful_shutdown(...)` 跑在 Tauri runtime 上(避免双 runtime)。
- 错误码全部覆盖:`api_server_already_running`、`api_server_port_in_use`、`api_server_bind_failed`、`api_server_disabled`、`api_server_state_poisoned`,以及额外 `api_server_serve_failed`(serve 任务异常退出兜底,不在 spec 强制集合内但属于纵深防御)。
- 纵深防御:`ApiServerService::start` 内部除 `validate_settings` 外**额外**断言 `bind_address.trim() == "127.0.0.1"`,不通过则返回 `api_server_bind_failed`。
- `reconcile` 幂等判断:`enabled=true` 且 bind/port 与当前 running 状态完全一致 → 直接返回 `Ok(())`;否则 stop+start。
- `reconcile_for_new_workspace` 在切换工作区时调用 — 一律先 stop(失败仅 `tracing::warn!`),再视新 settings 决定 start。
- `SettingsService::save_settings` 与 `WorkspaceService::select_workspace` 增加新参数 `&ApiServerService`;同步更新 `commands/`、各 service 单测调用点。`select_workspace` 内 reconcile 失败仅 `tracing::warn!` 不阻断切换。
- `lib.rs`:`pub mod api;`(为 integration test 暴露)、`.manage(ApiServerService::new())`、`.setup(...)` 钩子读取 settings 并按需 `start`(失败仅 warn,不阻断应用启动)、`invoke_handler` 注册 `get_api_server_status`。
- 前端:抽出 `ApiServerSettings.tsx`,展示 4 种 runtime_status chip(running 绿 / failed 警 / stopped 灰 / disabled 灰),token reset 按钮 `disabled` + tooltip "将在 Story 5.5 中启用",`SettingsPage` 用 2 秒 `setInterval` 轮询 `getApiServerStatus`,组件卸载与工作区切换时清理。展示 `last_error.message + correlation_id`,不展示 token/secret。
- 单测使用 `127.0.0.1:0` 让 OS 分配端口,避免 CI 端口冲突;集成测试同样用动态端口。
- `tracing::info!` 仅记录 `bind_address` 与 `port`,无 token/URL query string。

### File List

**Created:**

- `src-tauri/src/api/mod.rs`
- `src-tauri/src/api/server.rs`
- `src-tauri/src/api/state.rs`
- `src-tauri/src/services/api_server_service.rs`
- `src-tauri/src/commands/api_commands.rs`
- `src-tauri/tests/api_server_lifecycle_tests.rs`
- `src/features/settings/components/ApiServerSettings.tsx`

**Updated:**

- `src-tauri/Cargo.toml`
- `src-tauri/src/lib.rs`
- `src-tauri/src/services/mod.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/services/settings_service.rs`
- `src-tauri/src/services/workspace_service.rs`
- `src-tauri/src/services/analysis_service.rs`(测试调用点同步 `select_workspace` 新签名)
- `src-tauri/src/commands/settings_commands.rs`
- `src-tauri/src/commands/workspace_commands.rs`
- `src-tauri/src/domain/settings.rs`
- `src/features/settings/SettingsPage.tsx`
- `src/types/app.ts`
- `src/lib/tauriClient.ts`

## Change Log

- 2026-05-20 — Story 5.1 初版实现:嵌入式 axum localhost API 服务基础(占位 `__api_alive`)、`ApiServerService` 起停 reconcile 状态机、settings/workspace 集成、设置页 runtime_status 展示。新增 8 个 lib 单测 + 1 个集成测试。
