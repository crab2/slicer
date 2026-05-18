---
stepsCompleted:
  - step-01-init
  - step-02-context
  - step-03-starter
  - step-04-decisions
  - step-05-patterns
  - step-06-structure
  - step-07-validation
  - step-08-complete
inputDocuments:
  - D:\AIProject\slicer\_bmad-output\planning-artifacts\prd.md
workflowType: 'architecture'
lastStep: 8
project_name: 'slicer'
user_name: 'xq'
date: '2026-05-14'
status: complete
completedAt: '2026-05-14'
documentCounts:
  prd: 1
  productBriefs: 0
  uxDesign: 0
  research: 0
  projectDocs: 0
  projectContext: 0
---

# 架构决策文档 - slicer

_本文档将通过逐步协作的方式构建。每一步都会围绕关键架构决策追加内容，以保证后续 AI agent 实现时保持一致。_

## 项目上下文分析

### 需求概览

**功能需求：**

slicer 是一个 Windows 优先的 Rust/Tauri 桌面应用，核心目标不是简单“管理文档”，而是把不可直接检索的视觉页面转化为本地、稳定、可恢复、可追溯、可查询的页面级知识资产。

核心流水线为：

```text
source file -> canonical document record -> page image asset -> validated page analysis -> retrieval document -> query result with provenance
```

功能需求可分为五类：

1. 本地工作区与文件导入：工作目录选择、目录初始化、原始文件登记、重复文档识别、多文件导入。
2. 文档转换与页面资产：PDF 逐页渲染，Office 文档通过本机 LibreOffice headless 转 PDF 后再渲染，页面图片使用内容哈希命名。
3. 多模态分析：模型 provider/API key/base URL/custom endpoint/model name 配置，页面图片分析，JSON schema 校验，失败记录与重试。
4. 元数据与状态管理：SQLite 保存任务、文档、页面、分析状态和索引状态，同时输出 `metadata/pages.jsonl`。
5. 检索与对外接口：内置 BM25、搜索结果返回页面 JSON 与图片路径、GUI 搜索页、localhost HTTP API、索引重建，以及后续 SearchProvider 扩展。

**非功能需求：**

关键非功能需求会直接塑造架构：长任务必须后台执行且 GUI 不阻塞；转换、分析、索引重建必须可失败、可记录、可恢复、可重试；SQLite、页面图片、JSONL、BM25 索引之间必须保持可追溯关系；API key 不进入普通日志；调用云端模型前需要隐私提示；Windows、中文路径、中文文件名、路径空格和中文内容检索必须被当作第一版约束处理。

**规模与复杂度：**

- Primary domain: Windows 桌面应用、本地文档处理、多模态分析流水线、本地搜索与 localhost API。
- Complexity level: 中高。
- Estimated architectural components: 约 12 个。
- 推荐架构视角：Interface Layer、Application Service Layer、Orchestration Layer、Domain & Persistence Layer、Capability & Artifact Layer。

复杂度主要来自长任务编排、外部进程 LibreOffice、文件系统与数据库一致性、多模态 API 失败处理、索引原子重建、中文检索，以及 GUI 与后台任务状态同步。

### 技术约束与依赖

已知约束与依赖：

- 技术栈指定为 Rust + Tauri，Windows 为第一优先交付平台。
- Office 转换依赖用户本机 LibreOffice headless，不内置打包。
- PDF 渲染需要本地渲染模块，输出 PNG，默认 DPI 为 144。
- 数据存储采用 SQLite + 文件目录结构，工作区由用户选择。
- 页面图片文件名使用内容哈希；PRD 默认 `page_id = image_hash`，但架构上需要进一步确认该身份模型。
- 多模态分析通过云端 API 或用户自定义 HTTP endpoint，不要求本地视觉模型。
- 检索第一版以内置 BM25 为主，但查询接口不能硬编码为只支持 BM25。
- localhost HTTP API 默认仅监听本机地址。
- 路径、文件名、搜索内容都必须兼容中文与空格。

### 第一性原理架构框架

slicer 的架构应围绕“页面级知识资产流水线”设计，而不是围绕单个界面、单个转换工具或单个检索实现设计。

系统必须守住以下不变量：

1. 每个搜索结果都能追溯到原始文档、页码、页面图片和结构化 JSON。
2. 每个长任务状态都能从持久化状态恢复，而不是依赖内存事件。
3. 每个外部能力都可替换或可失败，包括 LibreOffice、PDF renderer、多模态模型 provider 和搜索 provider。
4. 每个进入检索链路的页面分析结果都必须通过版本化 schema 校验。
5. 每个文件系统资产都必须能与 SQLite 中的记录互相校验。
6. 每次索引重建都不能破坏上一个可用检索状态。
7. GUI 和 localhost HTTP API 应共享同一应用服务层，避免出现两套业务行为。

因此，架构上应优先定义这些核心边界：Workspace Ledger、Artifact Store、Job Orchestrator、Capability Providers、Application Services、Presentation Interfaces。

### Page Identity Decision Pressure

PRD 默认 `page_id = image_hash`，这适合内容去重，但可能不足以表达“同一图片出现在多个文档或页码位置”的来源关系。

架构设计应显式区分：

- `image_hash`：页面图片内容资产 ID，用于文件命名和去重。
- `page_id` 或 `page_record_id`：页面记录 ID，用于表示某个文档中的某一页。
- `document_id + page_number`：页面 occurrence 的自然来源关系。

如果 MVP 仍坚持 `page_id = image_hash`，则必须规定相同页面图片跨文档复用时的行为：是否允许一个 `image_hash` 对应多个 source entries；`GET /pages/{page_id}` 返回单个 source 还是 source 列表；搜索结果如何展示多个来源位置；重新分析同一图片是否影响所有引用该图片的页面。

### Pre-mortem: MVP 失败场景与架构预防

如果 slicer 的 MVP 在验收阶段失败，最可能不是少做某个页面，而是后台流水线、状态恢复、文件系统一致性和外部依赖边界没有被架构明确下来。

主要失败场景与预防措施：

1. 工作区状态不可恢复：明确 SQLite 为主状态源，文件系统和 JSONL 为可校验/可重建资产；定义启动时 workspace reconciliation。
2. 长任务让 GUI 卡死或状态漂移：设计统一 Job/Task 系统，覆盖 convert、analyze、index_rebuild，任务状态持久化到 SQLite。
3. LibreOffice 与 PDF 渲染成为不可控黑盒：封装独立 ConversionProvider/OfficeConverter，统一超时、临时目录、stderr 摘要、错误分类。
4. 多模态分析输出污染下游检索：拆分 ModelProvider、PromptTemplate、SchemaValidator、AnalysisService，写入前强制校验 `page_analysis_v1`。
5. 索引重建破坏已有搜索：采用临时构建目录，构建完成并校验后再原子切换 active index。
6. localhost API 安全边界被低估：明确默认绑定 `127.0.0.1`，为写操作或重任务接口预留本地 token 机制。
7. 中文检索质量不足：BM25 provider 内部需要显式 tokenizer/analyzer 决策，至少定义中文分词、字符 n-gram 或混合策略。

### 跨领域关注点

- Workspace reconciliation：启动时校验 SQLite、图片文件、JSONL 和索引之间的一致性。
- Atomic asset writes：页面图片、JSONL、索引构建应优先采用临时文件加原子替换策略。
- Job orchestration：转换、分析、索引重建需要统一任务模型。
- External process isolation：LibreOffice 和 PDF 渲染需要隔离封装、超时控制和结构化错误。
- Analysis contract enforcement：模型输出必须经过 schema 校验和规范化后才能进入检索链路。
- Index versioning：BM25 重建需要版本化与 active index 切换。
- Local API guardrails：localhost API 需要明确启停、监听地址、端口和写操作保护策略。
- Chinese retrieval quality：BM25 方案必须显式包含中文 tokenizer/analyzer 决策。

## Starter Template Evaluation

### Primary Technology Domain

主技术域是 Windows 优先的桌面应用，后端能力由 Rust + Tauri 承载，前端是用于工作台、搜索、设置和任务状态展示的本地 Web UI。

PRD 已明确以下技术偏好：

- Rust + Tauri 作为桌面应用基础。
- Windows 为第一优先平台。
- SQLite + 文件目录作为本地存储。
- LibreOffice headless 作为 Office 转 PDF 的外部依赖。
- BM25 作为 MVP 默认检索能力。
- localhost HTTP API 作为外部本地工具入口。

未发现独立 project-context 技术规则或 UX spec。因此 starter 选择应尽量贴近官方 Tauri 路径，减少早期自定义脚手架风险。

### Current Version Notes

截至 2026-05-14，已核验的官方信息：

- Tauri 官方文档推荐使用 `create-tauri-app` 创建新项目。
- `create-tauri-app` 支持 `react-ts`、`vanilla-ts`、`vue-ts`、`svelte-ts` 等模板。
- GitHub 显示 `create-tauri-app-js v4.7.0` 为当前最新 release。
- Tauri docs 侧栏列出 `@tauri-apps/cli` 最新为 `2.11.1`，`@tauri-apps/api` 最新为 `2.11.0`。
- Vite 当前文档要求 Node.js `20.19+` 或 `22.12+`。
- React docs 当前 latest version 为 `19.2`。

### Starter Options Considered

**Option 1: Official `create-tauri-app` with `react-ts`**

This is the recommended starter for slicer.

It provides the official Tauri v2 project shape, a Rust backend under `src-tauri/`, a TypeScript React frontend, and Vite-based development/build tooling. This fits slicer because the GUI needs a stateful workbench, task list, progress updates, settings forms, search result list, image preview, and JSON viewer.

Trade-off: React adds frontend dependency surface, but the UI complexity justifies a component model. The architecture should still keep business logic in Rust application services, not in React state.

**Option 2: Official `create-tauri-app` with `vanilla-ts`**

This is simpler and has fewer dependencies. It would be acceptable for a small utility, but slicer has enough UI state and view composition that vanilla TypeScript would likely create avoidable frontend structure decisions later.

Trade-off: lower dependency count, but more hand-rolled UI architecture.

**Option 3: Manual Vite React + `tauri init`**

This gives more control, but it recreates what the official starter already provides. Since there is no existing frontend, the manual path adds decision overhead without a clear benefit.

Trade-off: maximum control, but weaker consistency for AI agents implementing the first stories.

**Option 4: Rust-first UI with Yew/Leptos/Sycamore**

This keeps more code in Rust, but it increases frontend ecosystem risk for a desktop productivity UI. The PRD does not require Rust-only UI, and React is a safer fit for building dense, stateful interface screens quickly.

Trade-off: Rust purity, but less mainstream UI tooling for this use case.

**Rejected Option: Electron starter**

Electron is not aligned with the PRD because the product explicitly calls for Rust + Tauri.

### Selected Starter: Official Tauri React TypeScript Starter

**Rationale for Selection:**

Use official `create-tauri-app` with the `react-ts` template.

This starter makes the fewest surprising foundational decisions while still giving slicer the UI structure it needs. It keeps the core native capability layer in Rust/Tauri, gives the GUI a mature component model, and uses Vite for a fast development loop. It also aligns with Tauri's documented project structure, where frontend code lives at the top level and Rust desktop code lives under `src-tauri/`.

The starter should be treated as the shell only. It does not decide the architecture of conversion, analysis, indexing, storage, task orchestration, or localhost API. Those must be implemented as Rust-side application services and domain modules after initialization.

**Initialization Command:**

```bash
npm create tauri-app@latest slicer -- --template react-ts
```

Because the current repository already contains BMad planning artifacts, the first implementation story should not blindly overwrite the repo root. It should scaffold into a clean temporary directory or controlled app directory, then merge the generated `package.json`, frontend `src/`, and `src-tauri/` structure into the repository while preserving `_bmad`, `_bmad-output`, `.agents`, `.claude`, and `docs`.

**Architectural Decisions Provided by Starter:**

**Language & Runtime:**

- Rust backend through Tauri v2.
- TypeScript frontend.
- React UI layer.
- Node.js required for frontend tooling; current Vite docs require Node.js `20.19+` or `22.12+`.

**Styling Solution:**

The official starter should be treated as a minimal baseline. It should not lock slicer into a decorative UI framework. Styling should be added intentionally for a restrained desktop tool interface: workbench, search, settings, task status, preview pane, and JSON viewer.

**Build Tooling:**

- Vite for frontend dev server and production frontend build.
- Tauri CLI for desktop dev/build commands.
- Cargo project under `src-tauri/` for Rust application code.

**Testing Framework:**

The starter does not fully define slicer's testing strategy. Architecture should add:

- Rust unit/integration tests for workspace, conversion, job orchestration, schema validation, indexing, and API services.
- Frontend component tests for task list, search results, settings forms, and error states.
- End-to-end smoke tests for the Tauri shell once core flows exist.

**Code Organization:**

The starter establishes the physical split:

- Top-level frontend project for React/TypeScript UI.
- `src-tauri/` for Rust desktop backend and Tauri configuration.
- Tauri capability files for frontend-to-Rust permissions.

The project architecture should add a Rust module structure under `src-tauri/src/` around application services, domain models, repositories, job orchestration, artifact storage, providers, and HTTP API.

**Development Experience:**

- Hot frontend iteration through Vite.
- Tauri desktop dev command through npm scripts.
- Official Tauri project shape, making future documentation lookup and AI-agent implementation more predictable.

**Note:**

Project initialization using this starter should be the first implementation story. Immediately after scaffolding, the next implementation work should establish the Rust-side module boundaries before building features, so the UI does not accidentally become the owner of core workflow logic.

## Core Architectural Decisions

### Decision Priority Analysis

**Critical Decisions (Block Implementation):**

1. SQLite is the authoritative workspace ledger; file assets and JSONL are derived or reconcilable artifacts.
2. `page_id` is separated from `image_hash`; `image_hash` identifies image content, while `page_id` identifies a document page occurrence.
3. All long-running work uses a persistent Job Orchestrator, not ad hoc Tauri commands.
4. GUI and localhost HTTP API share the same Rust application service layer.
5. Search is implemented behind a `SearchProvider` abstraction, with MVP provider `TantivyBm25SearchProvider`.
6. Model output must pass `page_analysis_v1` schema validation before persistence or indexing.
7. Local HTTP API binds to `127.0.0.1` and protects write/heavy endpoints with a local token.

**Important Decisions (Shape Architecture):**

1. Use `sqlx` migrations for SQLite schema management.
2. Use atomic file writes for page images, JSONL exports, and index version activation.
3. Use `axum` for the embedded localhost REST API.
4. Use `tokio` for async tasks and `spawn_blocking` or dedicated workers for blocking conversion/rendering work.
5. Use `tracing` for structured diagnostics with explicit API key redaction.
6. Store model API keys in the OS credential store via `keyring`; do not store secrets in ordinary logs or page JSON.

**Deferred Decisions (Post-MVP):**

1. Cloud sync, multi-user auth, and permissions remain out of scope.
2. Vector or hybrid retrieval remains behind the `SearchProvider` interface.
3. macOS/Linux production packaging remains post-MVP.
4. Advanced document annotation/editing remains post-MVP.

### Data Architecture

SQLite is the primary state source for the selected workspace. The filesystem stores large or user-inspectable artifacts, but SQLite decides what the app believes exists.

**Technology Decision:**

- Database: SQLite.
- Rust access layer: `sqlx` with SQLite support.
- Migration approach: checked-in `sqlx` migrations under `src-tauri/migrations`.
- Runtime: Tokio-compatible database access.

**Core entities:**

- `settings`: workspace settings, LibreOffice path, DPI, concurrency, API server settings.
- `documents`: imported documents, original hash, original path, document type, import status.
- `image_assets`: unique page image assets keyed by full `image_hash`.
- `page_records`: page occurrences keyed by `page_id`, linked to `document_id`, `page_number`, and `image_hash`.
- `analysis_results`: validated page-level analysis JSON, schema version, provider, model name, status.
- `jobs`: persistent convert/analyze/index tasks.
- `job_events`: progress and status history.
- `errors`: structured retryable/non-retryable errors.
- `index_versions`: active and historical search index metadata.

**Identity decision:**

`page_id` must not equal `image_hash` in the architecture. The PRD default is useful for simple dedupe, but it creates ambiguity when the same rendered page appears in multiple documents or page positions.

Use:

- `image_hash`: content identity and image filename.
- `page_id`: stable page occurrence identity, derived from `document_id + page_number` or generated as a stable record id.
- `document_id + page_number`: source provenance.
- Full SHA-256 stored in SQLite; UI may display a shortened hash.

**Artifact rules:**

- `metadata/pages.jsonl` is an export/rebuildable artifact, not the source of truth.
- Page image writes use temp file + atomic rename before final status commit.
- Index rebuilds write to `indexes/bm25/build-<id>/` and only switch active index after validation.
- Startup runs workspace reconciliation to detect missing files, orphan files, invalid JSONL, or stale index pointers.

### Authentication & Security

There is no multi-user authentication in MVP.

Security decisions focus on local privacy, secret handling, and localhost API guardrails:

- Model API keys are stored via OS credential storage using `keyring`.
- API keys must never appear in normal logs, exported JSON, error summaries, search responses, or job events.
- HTTP API binds to `127.0.0.1` by default.
- Read endpoints may be available when the local API is enabled.
- Write/heavy endpoints, especially `POST /indexes/rebuild`, require a locally generated token.
- The settings UI must show API enabled/disabled status, bind address, port, and token reset action.
- Cloud model use requires an explicit privacy notice that page images are sent to the configured model endpoint.

### API & Communication Patterns

The application exposes two presentation interfaces:

1. Tauri GUI commands/events.
2. localhost REST API.

Both call the same Rust application service layer.

**GUI communication:**

- Frontend invokes Rust commands for user actions.
- Backend emits progress events for job updates.
- Frontend can also re-query persisted state, so task display survives missed events or app restart.

**HTTP API:**

Use `axum` for the embedded local API.

MVP endpoints:

- `GET /health`
- `GET /search?q={query}&limit={n}`
- `GET /pages/{page_id}`
- `GET /documents/{document_id}`
- `POST /indexes/rebuild`

**Error contract:**

All Tauri command errors and HTTP errors should map from the same internal `AppError` shape:

- `code`
- `message`
- `stage`
- `retryable`
- `details`
- `correlation_id`

### Frontend Architecture

The frontend uses React + TypeScript from the selected Tauri starter.

**State management:**

- Persistent/workspace state lives in Rust + SQLite.
- React stores only view-local state: selected tab, selected task, selected result, form drafts, loading state.
- Job progress is received through Tauri events and reconciled through explicit query commands.
- No Redux-style global store in MVP unless UI complexity later proves it necessary.

**Navigation:**

Use a simple app-level tab layout:

- Workbench
- Search
- Settings

No marketing page and no browser-style route complexity is needed for MVP.

**Component boundaries:**

- `features/workbench`
- `features/search`
- `features/settings`
- `components/common`
- `lib/tauriClient`

Frontend code must not own conversion, analysis, indexing, workspace reconciliation, or search logic. It only triggers use cases and renders state.

### Infrastructure & Deployment

This is a local desktop product, not a hosted cloud service.

**Packaging:**

- Windows-first Tauri build and installer.
- macOS/Linux packaging deferred.

**Runtime dependencies:**

- LibreOffice is detected and invoked from the user machine; it is not bundled in MVP.
- Missing LibreOffice produces a recoverable conversion error for Office documents.
- Intermediate PDFs are deleted after successful conversion by default, but retained for failed conversions and optionally under a debug setting.

**Diagnostics:**

- Use `tracing` for structured logs.
- Logs must redact API keys and avoid raw model responses unless explicitly saved as safely truncated diagnostic summaries.
- Job errors must be visible in GUI with stage, summary, retryability, and last occurrence time.

### Decision Impact Analysis

**Implementation Sequence:**

1. Scaffold official Tauri React TypeScript starter without overwriting BMad artifacts.
2. Establish Rust module boundaries and `AppState`.
3. Add SQLite migrations and repository layer.
4. Implement workspace initialization and reconciliation.
5. Implement artifact store with atomic writes.
6. Implement persistent job orchestrator.
7. Implement document import and conversion provider boundary.
8. Implement model provider, schema validation, and analysis service.
9. Implement Tantivy BM25 search provider with Chinese tokenizer support.
10. Implement shared application services for GUI and HTTP API.
11. Implement React workbench/search/settings views.

**Cross-Component Dependencies:**

- Job orchestration depends on SQLite repositories and artifact store.
- Conversion depends on artifact store, job orchestration, and error taxonomy.
- Analysis depends on model provider, schema validator, page records, and job orchestration.
- Search depends on validated analysis results and active index versioning.
- GUI and HTTP API depend on the same application service layer.
- Workspace reconciliation touches database, artifacts, JSONL, and index metadata.

## Implementation Patterns & Consistency Rules

### Pattern Categories Defined

**Critical Conflict Points Identified:**

本项目至少有 10 类容易产生 agent 实现冲突的地方：

1. SQLite 表名、字段名、枚举值命名。
2. Rust domain model、repository、service、provider 的模块归属。
3. `page_id`、`image_hash`、`document_id` 的身份语义。
4. Tauri command、事件名和 payload 格式。
5. localhost HTTP API response/error 格式。
6. Job 状态机、进度事件和错误记录格式。
7. 文件资产路径、临时文件和原子 rename 规则。
8. React feature 目录、组件命名、状态归属。
9. 日志、错误、用户可见消息和密钥脱敏规则。
10. 测试文件位置和命名。

### Naming Patterns

**Database Naming Conventions:**

- 表名使用小写复数 snake_case：`documents`、`page_records`、`analysis_results`、`index_versions`。
- 字段名使用 snake_case：`document_id`、`image_hash`、`created_at`。
- 主键字段统一为 `<entity>_id`，例如 `document_id`、`page_id`、`job_id`。
- 外键字段直接使用被引用实体 ID 名称，例如 `document_id`，不使用 `fk_document`。
- 索引命名使用 `idx_<table>_<columns>`，例如 `idx_page_records_document_id_page_number`。
- 唯一约束命名使用 `uq_<table>_<columns>`，例如 `uq_page_records_document_page`。

**API Naming Conventions:**

- REST endpoint 使用小写复数资源名：`/pages/{page_id}`、`/documents/{document_id}`、`/indexes/rebuild`。
- 路径参数使用 snake_case：`{page_id}`、`{document_id}`。
- Query 参数使用 snake_case：`q`、`limit`、`document_id`。
- JSON 字段对外统一使用 snake_case，保持与 Rust/SQLite 命名一致。
- HTTP API 不暴露 Rust enum 的内部大小写，状态值统一为 snake_case string。

**Code Naming Conventions:**

Rust:

- 模块和文件使用 snake_case：`job_orchestrator.rs`、`artifact_store.rs`。
- struct/enum/trait 使用 PascalCase：`JobOrchestrator`、`SearchProvider`、`AppError`。
- function/variable 使用 snake_case：`create_page_record`、`image_hash`。
- trait 名称表达能力边界：`OfficeConverter`、`PdfRenderer`、`ModelProvider`、`SearchProvider`。
- service struct 使用 `<Domain>Service`：`ImportService`、`AnalysisService`、`SearchService`。

TypeScript/React:

- React component 使用 PascalCase：`TaskList`、`SearchResults`。
- component 文件使用 PascalCase：`TaskList.tsx`。
- 非组件 TS 文件使用 camelCase 或 domain 名称：`tauriClient.ts`、`formatError.ts`。
- React hook 使用 `useXxx`：`useJobEvents`、`useWorkspaceStatus`。
- UI-only types 使用 camelCase 字段时只在 frontend 内部使用；与 Rust/API 交换的 DTO 保持 snake_case。

### Structure Patterns

**Project Organization:**

Rust/Tauri:

```text
src-tauri/src/
  app_state.rs
  commands/
  domain/
  services/
  repositories/
  jobs/
  providers/
  artifacts/
  api/
  errors.rs
  events.rs
  config.rs
```

Frontend:

```text
src/
  app/
  features/
    workbench/
    search/
    settings/
  components/
    common/
  lib/
    tauriClient.ts
  types/
```

**Rules:**

- Tauri commands only translate frontend calls into service calls; they must not contain business workflow logic.
- `services/` owns use cases and orchestration across repositories/providers.
- `repositories/` owns SQLite access only; no filesystem or model API calls.
- `providers/` owns replaceable external capabilities: LibreOffice, PDF rendering, model calls, search.
- `artifacts/` owns workspace file paths, temp files, atomic writes, and asset validation.
- `domain/` owns IDs, enums, entity structs, and state transition helpers.
- `api/` owns axum routes and HTTP DTO mapping only.
- React feature folders own screens and feature-specific components; shared visual primitives live in `components/common`。

**Testing Structure:**

- Rust unit tests can live next to modules with `#[cfg(test)]`。
- Rust integration tests live under `src-tauri/tests/`。
- Frontend component tests use `*.test.tsx` next to the component。
- End-to-end smoke tests should live under `tests/e2e/` once the Tauri shell is testable。

### Format Patterns

**API Response Formats:**

Successful list/search responses use explicit objects, not bare arrays:

```json
{
  "data": {
    "results": []
  }
}
```

Single-resource responses:

```json
{
  "data": {
    "page_id": "..."
  }
}
```

Error responses:

```json
{
  "error": {
    "code": "index_not_ready",
    "message": "Search index is not ready.",
    "stage": "search",
    "retryable": true,
    "correlation_id": "..."
  }
}
```

**Data Exchange Formats:**

- JSON field names are snake_case across Tauri DTOs, HTTP API, JSONL, and page analysis records。
- Timestamps use RFC 3339 strings in JSON。
- Status fields use snake_case strings: `analysis_pending`, `conversion_failed`, `index_rebuilding`。
- Optional values are `null` or omitted only when the DTO explicitly documents omission; default to `null` for API stability。
- `image_path` and local file paths are stored as strings but must be produced through path-safe Rust APIs, not manual string concatenation。

### Communication Patterns

**Tauri Command Patterns:**

- Command names use snake_case verbs: `select_workspace`, `import_documents`, `start_conversion`, `start_analysis`, `rebuild_index`, `search_pages`。
- Commands return DTOs or job IDs, not raw domain structs unless explicitly serialized for UI。
- Long-running commands enqueue a job and return quickly。

**Event System Patterns:**

Event names use `domain.action`:

- `job.created`
- `job.progress`
- `job.completed`
- `job.failed`
- `workspace.reconciled`
- `index.status_changed`

Event payloads include:

```json
{
  "event_id": "...",
  "occurred_at": "...",
  "job_id": "...",
  "stage": "...",
  "status": "...",
  "progress": {
    "current": 0,
    "total": 0
  }
}
```

Events are hints for live UI. They are not the source of truth. The frontend must be able to re-query persisted state after missed events or restart.

**State Management Patterns:**

- Rust + SQLite owns durable state。
- React owns transient view state only。
- React screens should load state through `lib/tauriClient.ts`, not call `invoke` directly from every component。
- No frontend-only copy of job state may be treated as authoritative。

### Process Patterns

**Error Handling Patterns:**

All Rust application errors map to `AppError`:

```rust
pub struct AppError {
    pub code: String,
    pub message: String,
    pub stage: ErrorStage,
    pub retryable: bool,
    pub details: Option<serde_json::Value>,
    pub correlation_id: String,
}
```

Rules:

- User-facing messages are concise and actionable。
- Diagnostic details may include stderr summaries but must never include API keys。
- Model raw responses may only be saved as safely truncated diagnostics when needed。
- Retryability is explicit; UI must not infer it from text。

**Loading State Patterns:**

- Long operations use job status, not local `isLoading` alone。
- UI buttons may use local loading state only while enqueueing a job。
- After a job is created, screens render from persisted job/page/document status。
- Empty, loading, failed, retrying, and rebuilding states must be explicit。

**Validation Patterns:**

- Validate user settings before saving when possible。
- Validate model output with `page_analysis_v1` before writing `analysis_results`。
- Validate workspace paths with Rust path APIs。
- Validate index build output before switching active index。

### Enforcement Guidelines

**All AI Agents MUST:**

- Keep durable workflow state in SQLite, not React state or memory-only Rust structs。
- Keep business logic in Rust services, not Tauri commands, axum handlers, or React components。
- Use `page_id` for page occurrence identity and `image_hash` for content identity。
- Use snake_case for database, API, event payload, JSONL, and page analysis fields。
- Use atomic write patterns for artifacts that are later referenced by SQLite。
- Emit and consume job events as live updates only; always support persisted-state re-query。
- Map user-visible failures through `AppError`。
- Redact API keys from logs, errors, events, JSONL, and HTTP responses。
- Avoid direct string path concatenation for filesystem work。

**Pattern Enforcement:**

- New modules must fit one of the documented Rust or frontend directories。
- New API endpoints must document success and error DTOs。
- New job types must define states, retryability, progress payload, and persistence behavior。
- New providers must be behind a trait and have at least one test double or mock implementation。
- Pattern violations should be corrected in the implementing story before adding new scope。

### Pattern Examples

**Good Examples:**

- `SearchService` calls `SearchProvider`, while `api/routes/search.rs` only maps HTTP request/response。
- `AnalysisService` validates model JSON before writing `analysis_results`。
- `ArtifactStore::write_page_image_atomic(...)` writes temp file, renames, then service commits DB state。
- React `SearchPage.tsx` calls `tauriClient.searchPages(...)` and renders DTOs。

**Anti-Patterns:**

- A Tauri command directly spawning LibreOffice and writing database rows。
- React components constructing local filesystem paths manually。
- Index rebuild writing directly into `indexes/bm25/active`。
- Treating `metadata/pages.jsonl` as the primary source of truth。
- Using `image_hash` as the only identity for API page resources without source occurrence handling。
- Logging full model requests or API key-bearing headers。

## Project Structure & Boundaries

### Complete Project Directory Structure

```text
slicer/
  README.md
  package.json
  package-lock.json
  index.html
  tsconfig.json
  tsconfig.node.json
  vite.config.ts
  eslint.config.js
  .gitignore
  .env.example

  src/
    main.tsx
    App.tsx
    app/
      AppShell.tsx
      navigation.ts
      appStyles.css
    features/
      workbench/
        WorkbenchPage.tsx
        components/
          WorkspacePicker.tsx
          ImportDropzone.tsx
          TaskList.tsx
          TaskStatusBadge.tsx
          RetryActions.tsx
        hooks/
          useJobEvents.ts
          useWorkbenchState.ts
      search/
        SearchPage.tsx
        components/
          SearchInput.tsx
          SearchResults.tsx
          PagePreview.tsx
          PageJsonViewer.tsx
          IndexStatusBanner.tsx
        hooks/
          useSearchState.ts
      settings/
        SettingsPage.tsx
        components/
          WorkspaceSettings.tsx
          LibreOfficeSettings.tsx
          ModelSettings.tsx
          ApiServerSettings.tsx
          PrivacyNotice.tsx
        hooks/
          useSettingsForm.ts
    components/
      common/
        Button.tsx
        IconButton.tsx
        Tabs.tsx
        Panel.tsx
        EmptyState.tsx
        ErrorMessage.tsx
        ProgressBar.tsx
        Spinner.tsx
    lib/
      tauriClient.ts
      eventClient.ts
      formatError.ts
      pathDisplay.ts
    types/
      api.ts
      events.ts
      jobs.ts
      pages.ts
      settings.ts
    styles/
      globals.css
      theme.css

  src-tauri/
    Cargo.toml
    tauri.conf.json
    build.rs
    capabilities/
      default.json
    migrations/
      0001_initial.sql
      0002_jobs_and_errors.sql
      0003_analysis_and_indexes.sql
    src/
      main.rs
      lib.rs
      app_state.rs
      config.rs
      errors.rs
      events.rs

      commands/
        mod.rs
        workspace_commands.rs
        import_commands.rs
        job_commands.rs
        search_commands.rs
        settings_commands.rs

      api/
        mod.rs
        server.rs
        state.rs
        auth.rs
        dto.rs
        routes/
          mod.rs
          health.rs
          search.rs
          pages.rs
          documents.rs
          indexes.rs

      domain/
        mod.rs
        ids.rs
        workspace.rs
        document.rs
        page.rs
        analysis.rs
        job.rs
        index.rs
        settings.rs

      repositories/
        mod.rs
        db.rs
        settings_repository.rs
        document_repository.rs
        page_repository.rs
        analysis_repository.rs
        job_repository.rs
        error_repository.rs
        index_repository.rs

      services/
        mod.rs
        workspace_service.rs
        import_service.rs
        conversion_service.rs
        analysis_service.rs
        search_service.rs
        index_service.rs
        settings_service.rs
        api_server_service.rs

      jobs/
        mod.rs
        job_orchestrator.rs
        job_runner.rs
        job_state_machine.rs
        progress.rs
        retry_policy.rs

      artifacts/
        mod.rs
        artifact_store.rs
        workspace_layout.rs
        atomic_write.rs
        jsonl_exporter.rs
        reconciliation.rs

      providers/
        mod.rs
        conversion/
          mod.rs
          office_converter.rs
          libreoffice_converter.rs
          pdf_renderer.rs
        model/
          mod.rs
          model_provider.rs
          custom_http_provider.rs
          prompt_template.rs
          schema_validator.rs
        search/
          mod.rs
          search_provider.rs
          tantivy_bm25_provider.rs
          chinese_analyzer.rs

      security/
        mod.rs
        secret_store.rs
        api_token.rs
        redaction.rs

      diagnostics/
        mod.rs
        logging.rs
        correlation.rs

    tests/
      workspace_reconciliation_tests.rs
      artifact_store_tests.rs
      job_orchestrator_tests.rs
      search_provider_tests.rs
      api_contract_tests.rs
    fixtures/
      sample_pages/
      sample_analysis/

  tests/
    e2e/
      smoke.spec.ts
      workbench.spec.ts
      search.spec.ts
    fixtures/
      documents/
      workspace/

  docs/
    architecture-notes.md
```

### Architectural Boundaries

**API Boundaries:**

- `src-tauri/src/api/` owns the localhost HTTP API.
- HTTP handlers must call `services/`; they must not call repositories, providers, or filesystem helpers directly.
- API authentication/token handling lives in `api/auth.rs` and `security/api_token.rs`.
- HTTP DTOs live in `api/dto.rs`; they are separate from database row structs.

**Tauri Command Boundaries:**

- `commands/` owns frontend command entry points.
- Commands should validate request DTO shape, call services, and return DTOs or job IDs.
- Commands must not spawn LibreOffice, write SQLite rows, mutate index files, or call model APIs directly.

**Service Boundaries:**

- `services/` owns use cases and cross-module orchestration.
- Services may call repositories, artifact store, jobs, and providers.
- Services are the shared layer used by both Tauri commands and HTTP API routes.

**Data Boundaries:**

- `repositories/` owns SQLite reads/writes and migrations.
- `domain/` owns stable domain types, IDs, status enums, and state transition helpers.
- `artifacts/` owns filesystem paths, page image files, JSONL export, index directories, temp files, and reconciliation.
- JSONL is produced through `artifacts/jsonl_exporter.rs`; it is not read as the primary state source.

**Provider Boundaries:**

- `providers/conversion/` owns Office conversion and PDF rendering capabilities.
- `providers/model/` owns model calls, prompt templates, and schema validation.
- `providers/search/` owns index/search implementations.
- Provider traits define replaceable capability boundaries; concrete provider modules implement MVP choices.

**Frontend Boundaries:**

- `features/workbench/` owns import, conversion, analysis, job list, retry, and index rebuild UI.
- `features/search/` owns query, result list, preview, JSON viewer, and index status UI.
- `features/settings/` owns workspace, LibreOffice, model provider, API server, and privacy settings UI.
- `components/common/` contains reusable UI primitives only.
- `lib/tauriClient.ts` is the only place that wraps `invoke`; feature components call typed client functions.

### Requirements to Structure Mapping

**FR-001 工作目录设置**

- Rust: `workspace_service.rs`, `settings_repository.rs`, `artifact_store.rs`, `workspace_layout.rs`, `reconciliation.rs`
- Frontend: `WorkspacePicker.tsx`, `WorkspaceSettings.tsx`
- Tests: `workspace_reconciliation_tests.rs`

**FR-002 文件导入**

- Rust: `import_service.rs`, `document_repository.rs`, `artifact_store.rs`
- Frontend: `ImportDropzone.tsx`, `TaskList.tsx`
- Domain: `document.rs`, `ids.rs`

**FR-003 文档转换**

- Rust: `conversion_service.rs`, `office_converter.rs`, `libreoffice_converter.rs`, `pdf_renderer.rs`
- Jobs: `job_orchestrator.rs`, `job_runner.rs`, `progress.rs`
- Frontend: `TaskStatusBadge.tsx`, `RetryActions.tsx`

**FR-004 图片哈希命名**

- Rust: `artifact_store.rs`, `atomic_write.rs`, `page_repository.rs`
- Domain: `page.rs`
- Tests: `artifact_store_tests.rs`

**FR-005 多模态模型配置**

- Rust: `settings_service.rs`, `secret_store.rs`, `model_provider.rs`, `custom_http_provider.rs`
- Frontend: `ModelSettings.tsx`, `PrivacyNotice.tsx`
- Security: `redaction.rs`

**FR-006 页面分析**

- Rust: `analysis_service.rs`, `prompt_template.rs`, `schema_validator.rs`, `analysis_repository.rs`
- Jobs: `job_orchestrator.rs`, `retry_policy.rs`
- Fixtures: `src-tauri/fixtures/sample_analysis/`

**FR-007 元数据保存**

- Rust: `analysis_repository.rs`, `page_repository.rs`, `jsonl_exporter.rs`
- Data: migrations under `src-tauri/migrations/`
- Tests: repository and reconciliation tests

**FR-008 BM25 检索**

- Rust: `search_service.rs`, `search_provider.rs`, `tantivy_bm25_provider.rs`, `chinese_analyzer.rs`
- Data: `index_repository.rs`, `index.rs`
- Tests: `search_provider_tests.rs`

**FR-009 查询返回**

- Rust: `search_commands.rs`, `api/routes/search.rs`, `api/routes/pages.rs`
- Frontend: `SearchResults.tsx`, `PagePreview.tsx`, `PageJsonViewer.tsx`
- Types: `src/types/pages.ts`, `src/types/api.ts`

**FR-010 索引重建**

- Rust: `index_service.rs`, `tantivy_bm25_provider.rs`, `index_repository.rs`
- Jobs: `job_runner.rs`, `progress.rs`
- Frontend: `IndexStatusBanner.tsx`, `RetryActions.tsx`

**FR-011 本地 HTTP API**

- Rust: `api/server.rs`, `api/routes/*`, `api/auth.rs`, `api_server_service.rs`
- Security: `api_token.rs`
- Tests: `api_contract_tests.rs`

**FR-012 扩展检索接口**

- Rust: `search_provider.rs`, `tantivy_bm25_provider.rs`
- Future providers must implement the same `SearchProvider` trait and not change service/API contracts.

### Integration Points

**Internal Communication:**

- Frontend -> Tauri commands -> Services -> Repositories/Artifacts/Providers.
- Frontend listens to Tauri events from `events.rs`, then re-queries state through `tauriClient.ts`.
- HTTP API -> API routes -> Services -> Repositories/Artifacts/Providers.
- Jobs -> Services/Providers/Artifacts -> Repositories -> Events.

**External Integrations:**

- LibreOffice executable: `providers/conversion/libreoffice_converter.rs`.
- PDF renderer implementation: `providers/conversion/pdf_renderer.rs`.
- Custom/cloud model endpoint: `providers/model/custom_http_provider.rs`.
- OS credential store: `security/secret_store.rs`.
- Localhost consumers: `api/routes/*`.

**Data Flow:**

```text
imported file
  -> ImportService
  -> DocumentRepository + ArtifactStore(originals)
  -> JobOrchestrator(convert)
  -> ConversionService
  -> OfficeConverter/PdfRenderer
  -> ArtifactStore(pages) + PageRepository
  -> JobOrchestrator(analyze)
  -> AnalysisService
  -> ModelProvider + SchemaValidator
  -> AnalysisRepository + JsonlExporter
  -> IndexService
  -> SearchProvider(Tantivy BM25)
  -> GUI Search / localhost API
```

### File Organization Patterns

**Configuration Files:**

- Root `package.json`, `vite.config.ts`, `tsconfig.json` configure frontend/Tauri dev tooling.
- `src-tauri/Cargo.toml` owns Rust dependencies.
- `src-tauri/tauri.conf.json` owns Tauri product configuration.
- Runtime user settings are stored in SQLite and edited through settings UI.
- Secrets are stored via OS credential store, not config files.

**Source Organization:**

- Rust source follows domain/service/repository/provider/artifact/API boundaries.
- Frontend source follows app/features/common/lib/types boundaries.
- Cross-boundary DTOs are duplicated intentionally when needed; do not leak database row structs directly to UI/API.

**Test Organization:**

- Rust integration tests live in `src-tauri/tests/`.
- Rust fixtures live in `src-tauri/fixtures/`.
- Frontend component tests live beside components.
- Tauri end-to-end tests live in root `tests/e2e/`.

**Asset Organization:**

- Source UI assets live under `src/assets/` only if needed.
- Runtime user assets live only inside the selected workspace, not in the app repo.
- Workspace runtime layout remains:

```text
workspace/
  originals/
  pages/
    <document_id>/
      <image_hash>.png
  metadata/
    pages.jsonl
  indexes/
    bm25/
      active
      build-<id>/
  app.db
```

### Development Workflow Integration

**Development Server Structure:**

- `npm run tauri dev` starts the Vite frontend and Tauri shell.
- Rust app state initializes repositories, artifact store, job orchestrator, provider registry, and optional local API server.
- Frontend screens should function against real Tauri commands, with mock clients only in tests.

**Build Process Structure:**

- Frontend builds through Vite.
- Tauri packages the Rust backend and built frontend.
- Windows packaging is the first distribution target.
- LibreOffice is detected at runtime and not bundled.

**Deployment Structure:**

- App installation and user workspace are separate.
- Application updates must not mutate user workspaces except through explicit migrations/reconciliation.
- SQLite migrations run against the selected workspace database.
- Index files and JSONL exports can be rebuilt from SQLite + page image assets.

## Architecture Validation Results

### Coherence Validation ✅

**Decision Compatibility:**

当前架构决策整体兼容：

- Rust + Tauri + React TypeScript starter 与 Windows-first 桌面应用目标一致。
- SQLite + 文件目录结构与本地优先、可恢复、无默认云同步的产品要求一致。
- `sqlx` migrations、repositories、services、artifacts 的分层支持 workspace ledger 的主状态源定位。
- `page_id` 与 `image_hash` 分离，解决了 PRD 默认身份模型在跨文档重复页面场景下的歧义。
- `JobOrchestrator`、持久化 job 状态、Tauri events 和主动查询组合，支持长任务、失败恢复和 GUI 不阻塞。
- `SearchProvider` + Tantivy BM25 + 中文 analyzer 决策，支持 MVP BM25，同时保留后续 qmd/wiki/vector/hybrid 扩展。
- `axum` localhost API 与 Tauri commands 共享 service layer，避免两套业务逻辑。
- `keyring`、redaction、local API token 和 privacy notice 覆盖本地隐私与密钥保护要求。

未发现互相冲突的核心技术决策。

**Pattern Consistency:**

实现模式与架构决策一致：

- snake_case 数据/API/event 命名统一 SQLite、Rust DTO、HTTP API、JSONL 与 page analysis。
- Tauri command、HTTP route、service、repository、provider 的职责边界支持“业务逻辑只在 services”这一核心规则。
- `AppError` 统一 Tauri 与 HTTP 错误映射，支持 retryable、stage、correlation_id 和用户可见错误。
- Job events 被定义为 live hints 而非 source of truth，符合持久化状态优先原则。
- 原子写入、active index 切换、workspace reconciliation 与数据一致性目标一致。

**Structure Alignment:**

项目结构支持所有主要架构决策：

- `src-tauri/src/services/` 支撑应用用例层。
- `src-tauri/src/repositories/` 支撑 SQLite ledger。
- `src-tauri/src/artifacts/` 支撑文件资产与 workspace reconciliation。
- `src-tauri/src/jobs/` 支撑后台任务和可恢复状态机。
- `src-tauri/src/providers/` 支撑 LibreOffice、PDF renderer、model provider、search provider 等外部能力隔离。
- `src-tauri/src/api/` 支撑 localhost HTTP API。
- `src/features/*` 支撑工作台、搜索、设置三个 PRD 指定主界面。

### Requirements Coverage Validation ✅

**Feature Coverage:**

PRD 中的 12 个 FR 均已映射到架构模块和项目结构：

- FR-001 工作目录设置：workspace service、settings repository、artifact store、reconciliation、settings/workspace UI。
- FR-002 文件导入：import service、document repository、artifact store、import dropzone。
- FR-003 文档转换：conversion service、LibreOffice converter、PDF renderer、job orchestrator。
- FR-004 图片哈希命名：artifact store、atomic write、page repository、page domain。
- FR-005 多模态模型配置：settings service、secret store、model provider、model settings UI。
- FR-006 页面分析：analysis service、prompt template、schema validator、analysis repository、jobs。
- FR-007 元数据保存：SQLite repositories、JSONL exporter、migrations。
- FR-008 BM25 检索：SearchProvider、Tantivy provider、Chinese analyzer、index repository。
- FR-009 查询返回：search service、Tauri search commands、HTTP search/pages routes、search UI。
- FR-010 索引重建：index service、job runner、index repository、index status UI。
- FR-011 本地 HTTP API：axum server、routes、auth/token、API contract tests。
- FR-012 扩展检索接口：SearchProvider trait and provider boundary。

**Functional Requirements Coverage:**

功能需求具备完整架构支撑。尤其是 PRD 中容易变成隐性风险的要求：失败重试、应用重启恢复、页面 JSON + 图片路径返回、索引重建不破坏旧索引、中文路径/中文检索，都已经有对应边界或模式。

**Non-Functional Requirements Coverage:**

- Performance: 长任务通过 job orchestrator、后台 runner、progress events 支撑；GUI 不直接执行阻塞任务。
- Stability: SQLite ledger、workspace reconciliation、job state machine、atomic writes、active index versioning 支撑恢复性。
- Security/Privacy: keyring、redaction、localhost bind、API token、privacy notice 支撑本地隐私目标。
- Compatibility: Windows-first、路径安全 API、中文 analyzer 决策覆盖第一版兼容性重点。
- Extensibility: provider traits、service layer、versioned schema、SearchProvider 边界覆盖扩展性。

### Implementation Readiness Validation ✅

**Decision Completeness:**

关键实现阻塞决策均已明确：

- Starter 与技术栈。
- SQLite 主状态源。
- 页面身份模型。
- 后台任务模型。
- HTTP API 技术与安全边界。
- SearchProvider 与 BM25 provider。
- Provider/service/repository/artifact 分层。
- 前端状态归属。
- 错误、事件、响应格式。

**Structure Completeness:**

项目结构足够具体，包含 root config、frontend feature folders、Rust modules、migrations、tests、fixtures、runtime workspace layout。AI agent 可以据此创建文件而不需要重新决定目录边界。

**Pattern Completeness:**

命名、结构、响应、事件、错误、loading、validation、secret redaction、atomic writes、test placement 均有规则和示例。足以约束多个 agent 的实现风格。

### Gap Analysis Results

**Critical Gaps:**

None. 当前未发现阻塞 implementation 的架构缺口。

**Important Gaps:**

1. PDF renderer 具体库尚未最终确定。架构已定义 `PdfRenderer` provider 边界，因此不阻塞，但第一批实现故事需要选择并验证 Windows 可用库。
2. Tantivy 中文 analyzer 的具体策略需要在实现 story 中用样例验证：jieba 分词、字符 n-gram 或混合策略。架构已保留 `chinese_analyzer.rs`。
3. `page_id` 生成算法需要在数据模型实现时最终定稿。架构已决定不等于 `image_hash`，但可以在实现时选择 deterministic ID 或 generated UUID。
4. API token 默认启用策略需要产品确认：本地 API 是否默认关闭，以及 read endpoint 是否也需要 token。架构已预留机制。
5. 中间 PDF 保留策略需要在设置项实现时落地：成功后默认删除、失败保留、debug 可保留。

**Nice-to-Have Gaps:**

1. 可补充 ADR 文件模板，用于后续记录 PDF renderer、BM25 tokenizer、API token 默认策略等细化选择。
2. 可补充 sample workspace fixture，帮助测试 workspace reconciliation。
3. 可补充 API contract 示例 JSON，帮助 HTTP API 与 Tauri DTO 保持一致。

### Validation Issues Addressed

验证中发现的最大潜在问题是 PRD 默认 `page_id = image_hash` 可能导致跨文档重复页面的来源歧义。架构已通过分离 `page_id` 和 `image_hash` 解决该问题。

另一个潜在问题是索引重建破坏旧索引。架构已通过 `index_versions`、临时 build directory、active index 切换和验证后提交解决。

### Architecture Completeness Checklist

**Requirements Analysis**

- [x] Project context thoroughly analyzed
- [x] Scale and complexity assessed
- [x] Technical constraints identified
- [x] Cross-cutting concerns mapped

**Architectural Decisions**

- [x] Critical decisions documented with versions
- [x] Technology stack fully specified
- [x] Integration patterns defined
- [x] Performance considerations addressed

**Implementation Patterns**

- [x] Naming conventions established
- [x] Structure patterns defined
- [x] Communication patterns specified
- [x] Process patterns documented

**Project Structure**

- [x] Complete directory structure defined
- [x] Component boundaries established
- [x] Integration points mapped
- [x] Requirements to structure mapping complete

### Architecture Readiness Assessment

**Overall Status:** READY FOR IMPLEMENTATION

**Confidence Level:** high

**Key Strengths:**

- Strong separation between durable state, file artifacts, jobs, providers, and UI.
- Explicit recovery and reconciliation model for a local-first app.
- Clear job/event/query pattern for long-running work.
- Search and model capabilities isolated behind provider traits.
- Identity model corrected before implementation begins.
- Project structure maps every FR to concrete modules.

**Areas for Future Enhancement:**

- Formal ADR files for implementation-level library choices.
- More detailed API contract examples.
- Dedicated sample workspace fixtures for recovery and indexing tests.
- Future vector/hybrid search provider after MVP.

### Implementation Handoff

**AI Agent Guidelines:**

- Follow all architectural decisions exactly as documented.
- Use implementation patterns consistently across all components.
- Respect project structure and boundaries.
- Refer to this document for all architectural questions.
- Do not move business logic into React components, Tauri commands, or axum route handlers.
- Do not treat JSONL, event payloads, or frontend state as source of truth.
- Do not use `image_hash` as the only page resource identity.

**First Implementation Priority:**

Initialize the official Tauri React TypeScript starter without overwriting planning artifacts:

```bash
npm create tauri-app@latest slicer -- --template react-ts
```

Because the repository already contains BMad artifacts, scaffold in a controlled temporary/app directory first, then merge the generated application structure into the repo while preserving `_bmad`, `_bmad-output`, `.agents`, `.claude`, and `docs`.
