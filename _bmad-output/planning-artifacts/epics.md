---
stepsCompleted:
  - step-01-validate-prerequisites
  - step-02-design-epics
  - step-03-create-stories
  - step-04-final-validation
inputDocuments:
  - "D:/AIProject/slicer/_bmad-output/planning-artifacts/prd.md"
  - "D:/AIProject/slicer/_bmad-output/planning-artifacts/architecture.md"
workflowType: 'epics-and-stories'
project_name: 'slicer'
user_name: 'xq'
status: complete
---

# slicer - Epic Breakdown

## Overview

This document provides the complete epic and story breakdown for slicer, decomposing the requirements from the PRD, UX Design if it exists, and Architecture requirements into implementable stories.

## Requirements Inventory

### Functional Requirements

FR1 (FR-001): 用户必须能够选择一个本地目录作为工作目录；应用必须在该目录下创建和维护标准子目录、数据库和索引文件，并在更改工作目录后重新加载该目录中的数据库、任务状态和索引状态。

FR2 (FR-002): 用户必须能够通过拖拽或文件选择一次导入一个或多个 PDF、PPT、PPTX、DOC、DOCX 文件；系统必须计算原文件哈希、识别重复文档、拒绝不支持的文件类型并展示原因，且将原始文件复制或登记到 `originals/` 并关联 `document_id`。

FR3 (FR-003): 系统必须将 PDF 文件逐页渲染为 PNG，并将 PPT/PPTX/DOC/DOCX 先通过本机 LibreOffice headless 转换为 PDF 后再逐页渲染为 PNG；转换失败或缺失 LibreOffice 时必须进入可恢复状态并记录诊断信息。

FR4 (FR-004): 每张页面图片必须使用图片内容哈希命名，相同图片内容生成相同 `image_hash`，图片文件名格式为 `<image_hash>.png`，并避免同一文档内不同页面因文件名冲突被覆盖。

FR5 (FR-005): 用户必须能够配置用于页面图片分析的云端 API 或自定义 HTTP endpoint，包括 provider、API key、base URL、custom endpoint 和 model name；未配置模型时分析不可直接执行，且 API key 不得出现在日志、错误提示或导出 JSON 中。

FR6 (FR-006): 系统必须对新生成或用户明确要求重跑的页面图片调用多模态模型，并生成 `page_analysis_v1` JSON；正常输出必须通过 schema 校验并入库，非法 JSON、超时和 API 错误必须进入失败状态，失败页面和单文档分析必须可重试。

FR7 (FR-007): 系统必须同时保存结构化 SQLite 记录和可读 JSONL 元数据；SQLite 保存任务、文档、页面和分析状态，`metadata/pages.jsonl` 保存页面级 JSON，且 `page_id`、`document_id`、`image_path` 在数据库和 JSONL 中保持一致。

FR8 (FR-008): 系统必须基于页面分析结果构建 BM25 索引，索引文本必须包含标题、摘要、可见文字、主题、关键词和来源文件名，并支持中文关键词命中、相关性排序、相关分数返回和索引不可用状态提示。

FR9 (FR-009): 搜索结果必须返回页面 JSON 和图片地址；GUI 搜索结果必须可打开页面图片预览并查看对应页面 JSON，HTTP API 必须返回 `page_id`、`score`、`image_path`、页面 JSON 摘要和来源信息，且不允许只返回文本片段。

FR10 (FR-010): 用户必须能够按全部页面重建 BM25 索引；重建不得删除原图片或页面 JSON，重建失败不得破坏上一个可用索引，GUI 必须显示重建进度、成功状态和失败原因。

FR11 (FR-011): 应用必须提供默认仅监听 localhost 的 HTTP API，支持 `GET /health`、`GET /search?q={query}&limit={n}`、`GET /pages/{page_id}`、`GET /documents/{document_id}` 和 `POST /indexes/rebuild`。

FR12 (FR-012): 系统必须保留 `SearchProvider` 或等价 adapter 概念，使第一版默认内置 BM25，同时为 qmd、wiki search、向量检索或混合检索预留扩展接口；查询接口不得硬编码为只能支持 BM25。

FR13: 应用第一屏必须是工作台而非营销页或介绍页，主导航必须包含工作台、搜索和设置三个主要区域。

FR14: 工作台必须展示当前工作目录、选择或更改工作目录入口、文件拖拽区域、文件选择入口、任务列表、每个任务的文件名/类型/页数/转换状态/分析状态/失败原因，以及转换、分析、单任务重试、全部失败项重试和索引重建入口。

FR15: 搜索页必须包含搜索输入框、搜索结果列表、结果项标题/摘要/来源文档/页码/相关分数、图片预览区、页面 JSON 查看区、无结果空状态，以及索引不可用或正在重建时的状态提示。

FR16: 设置页必须包含工作目录路径、LibreOffice 可执行文件路径、自动检测 LibreOffice 按钮、模型 provider 名称、API key、base URL、自定义 endpoint、model name、默认图片 DPI、转换并发数、分析并发数，以及启用云端模型时的隐私提示。

FR17: 页面 JSON schema 必须使用 `page_analysis_v1`，包含 `page_id`、`image_hash`、`image_path`、source 信息、analysis 信息、retrieval 文本、model 信息和 `schema_version`。

FR18: 工作目录结构必须至少包含 `originals/`、`pages/<document_id>/<image_hash>.png`、`metadata/pages.jsonl`、`indexes/bm25/` 和 `app.db`。

FR19: 文档状态至少必须支持 `imported`、`converting`、`converted`、`conversion_failed`、`analyzing`、`analyzed`、`analysis_failed`、`indexed` 和 `index_failed`。

FR20: 页面状态至少必须支持 `image_created`、`analysis_pending`、`analysis_running`、`analysis_succeeded`、`analysis_failed` 和 `indexed`。

FR21: 错误记录必须至少包含错误类型、错误摘要、发生阶段、关联文档或页面、是否可重试，以及最近一次发生时间。

### NonFunctional Requirements

NFR1: 30 页 PPTX 在正常 LibreOffice 环境下应能完成转换和页面记录生成。

NFR2: 300 页 PDF 转换时 GUI 不得卡死。

NFR3: 长任务必须后台执行，并持续更新进度。

NFR4: 搜索接口在普通本地资料库规模下应在可感知的短时间内返回结果。

NFR5: 转换、分析、索引重建任务都必须可失败、可记录、可恢复、可重试。

NFR6: 应用异常关闭后，重启不能丢失已完成页面记录。

NFR7: 重建索引失败不能破坏已有可用索引。

NFR8: 所有数据默认保存在用户选择的本地工作目录。

NFR9: 应用不做默认云同步。

NFR10: API key 不得写入普通日志。

NFR11: 调用云端模型前必须让用户知道页面图片会发送到配置的模型服务。

NFR12: localhost API 默认不得监听公网地址。

NFR13: 第一版仅承诺 Windows 优先。

NFR14: 路径包含中文或空格时必须可用。

NFR15: 系统必须支持中文文件名和中文页面内容检索。

NFR16: macOS/Linux 作为后续兼容方向，不进入第一版验收承诺。

NFR17: 检索层必须保留 provider/adapter 抽象。

NFR18: 模型调用层必须保留 provider/endpoint 抽象。

NFR19: 页面 JSON schema 必须带版本号，并支持后续升级。

NFR20: GUI 视觉风格必须简洁、克制、桌面工具感强，信息密度适中，适合批量任务处理。

NFR21: GUI 不应使用营销式大 hero 或过度装饰性渐变背景。

NFR22: 常见操作必须使用清晰按钮和图标。

NFR23: 错误状态必须明确、可定位、可恢复。

NFR24: Windows MVP 验收必须覆盖 PDF、PPTX、DOCX 样例文件端到端流程、30 页 PPTX、中文路径/长文件名、应用重启恢复和 Windows 打包验证。

NFR25: 转换、哈希存储、模型分析、检索和 GUI 均必须具备对应验收测试或测试计划覆盖。

### Additional Requirements

- Starter Template: 必须使用官方 `create-tauri-app` 的 `react-ts` 模板作为项目初始化基础，提供 Tauri v2 Rust 后端、React TypeScript 前端和 Vite 开发/构建工具链。
- 初始化时不得直接覆盖仓库根目录；必须先在临时或受控目录中 scaffold，再合并 `package.json`、前端 `src/` 和 `src-tauri/`，并保留 `_bmad`、`_bmad-output`、`.agents`、`.claude` 和 `docs`。
- Starter 只负责应用壳，不决定转换、分析、索引、存储、任务编排或 localhost API；这些能力必须在 Rust 应用服务和领域模块中实现。
- 前端必须使用 React + TypeScript；Rust/Tauri 承载核心 native 能力；Vite 负责前端开发服务器和生产构建；Tauri CLI 负责桌面 dev/build。
- Node.js 版本需要满足当前 Vite 要求：`20.19+` 或 `22.12+`。
- SQLite 是工作区权威账本；文件资产和 JSONL 是可校验或可重建 artifacts，不是主状态源。
- 架构必须显式分离 `page_id` 和 `image_hash`：`image_hash` 是图片内容身份和文件名，`page_id` 是文档页面 occurrence 身份，来源关系由 `document_id + page_number` 表达。
- SQLite 必须至少包含 `settings`、`documents`、`image_assets`、`page_records`、`analysis_results`、`jobs`、`job_events`、`errors`、`index_versions` 等核心实体。
- 数据库访问必须使用 `sqlx` with SQLite，迁移文件必须放在 `src-tauri/migrations`。
- `metadata/pages.jsonl` 必须作为导出/可重建 artifact 处理，不得作为主状态源读取。
- 页面图片写入必须采用临时文件加原子 rename，并在最终状态提交前完成 artifact 写入。
- BM25 索引重建必须写入 `indexes/bm25/build-<id>/`，验证通过后再切换 active index。
- 应用启动时必须执行 workspace reconciliation，用于检测缺失文件、孤儿文件、非法 JSONL 或 stale index pointer。
- 所有长任务必须通过持久化 Job Orchestrator 执行，不得由 Tauri command 直接执行长耗时转换、分析或索引重建。
- Job 事件只作为 live UI hints；前端必须能通过显式查询恢复持久化任务状态，不能把事件或 React 内存状态当作 source of truth。
- GUI 和 localhost HTTP API 必须共享同一 Rust application service layer，避免出现两套业务逻辑。
- Tauri commands 只负责把前端请求转换为 service 调用并返回 DTO 或 job ID，不得直接 spawn LibreOffice、写 SQLite、修改索引文件或调用模型 API。
- `services/` 负责用例和跨 repository/provider/artifact/job 的编排。
- `repositories/` 只负责 SQLite 读写和迁移，不负责文件系统、模型 API 或外部进程。
- `providers/` 负责 LibreOffice、PDF 渲染、模型调用、搜索等可替换能力；provider 必须通过 trait 定义边界。
- `artifacts/` 负责 workspace 文件路径、临时文件、原子写入、JSONL 导出、索引目录和 reconciliation。
- `domain/` 负责 ID、状态枚举、实体和状态转换 helper。
- `api/` 只负责 axum routes、认证和 HTTP DTO 映射，handlers 必须调用 services，不得绕过 service 访问 repositories/providers/artifacts。
- localhost HTTP API 必须使用 `axum`，默认绑定 `127.0.0.1`。
- 写操作或重任务接口，尤其是 `POST /indexes/rebuild`，必须使用本地生成 token 保护。
- Settings UI 必须展示 API 启用/禁用状态、bind address、port 和 token reset action。
- 模型 API key 必须使用 OS credential storage，通过 `keyring` 保存；不得出现在普通日志、导出 JSON、错误摘要、搜索响应或 job events 中。
- 日志和诊断必须使用 `tracing`，并显式进行 API key redaction；原始模型响应只允许在必要时作为安全截断的诊断摘要保存。
- `AppError` 必须统一承载 Tauri command 和 HTTP API 错误映射，至少包含 `code`、`message`、`stage`、`retryable`、`details`、`correlation_id`。
- HTTP 成功响应必须使用显式对象结构，例如 `{ "data": { ... } }`；错误响应必须使用 `{ "error": { ... } }`。
- JSON 字段、Tauri DTO、HTTP API、JSONL 和 page analysis 字段必须统一使用 snake_case。
- 时间戳必须使用 RFC 3339 字符串；状态值必须使用 snake_case string。
- 文件路径必须由 Rust path-safe API 生成和校验，不能手写字符串拼接。
- 前端持久状态属于 Rust + SQLite；React 只保留 selected tab、selected task/result、form draft、loading 等视图本地状态。
- 前端必须通过 `lib/tauriClient.ts` 封装 `invoke`，feature components 不应直接散落调用 Tauri invoke。
- 前端主区域必须为 Workbench、Search、Settings，不需要浏览器式复杂 routing。
- Rust 模块结构必须围绕 `app_state.rs`、`commands/`、`domain/`、`services/`、`repositories/`、`jobs/`、`providers/`、`artifacts/`、`api/`、`security/`、`diagnostics/` 组织。
- 前端模块结构必须围绕 `src/app`、`src/features/workbench`、`src/features/search`、`src/features/settings`、`src/components/common`、`src/lib`、`src/types`、`src/styles` 组织。
- Rust integration tests 必须放在 `src-tauri/tests/`；Rust fixtures 放在 `src-tauri/fixtures/`；前端组件测试放在组件旁边；Tauri E2E smoke tests 放在 `tests/e2e/`。
- LibreOffice 是用户机器上的运行时依赖，MVP 不内置打包；缺失 LibreOffice 必须产生可恢复的 Office 文档转换错误。
- 中间 PDF 默认在成功转换后删除，失败时保留，并可通过 debug 设置保留。
- 搜索必须通过 `SearchProvider` 抽象实现，MVP provider 为 `TantivyBm25SearchProvider`。
- BM25 provider 必须显式支持中文 tokenizer/analyzer 策略；具体策略可在实现故事中通过 jieba、字符 n-gram 或混合策略验证。
- 模型输出必须在写入 `analysis_results` 或进入索引前通过 `page_analysis_v1` schema 校验。
- 本地 API 端点至少包括 `GET /health`、`GET /search?q={query}&limit={n}`、`GET /pages/{page_id}`、`GET /documents/{document_id}` 和 `POST /indexes/rebuild`。
- 实现顺序应优先完成：Tauri React TypeScript scaffold、Rust module boundaries/AppState、SQLite migrations/repositories、workspace initialization/reconciliation、atomic artifact store、persistent job orchestrator、import/conversion provider boundary、model provider/schema validation/analysis service、Tantivy BM25 with Chinese tokenizer、shared GUI/API services、React workbench/search/settings。
- 尚需在实现阶段进一步定稿：PDF renderer 具体库、Tantivy 中文 analyzer 策略、`page_id` 生成算法、API token 默认启用策略、中间 PDF 保留策略。

### UX Design Requirements

未发现独立 UX Design 文档。PRD 中的界面、交互、状态和视觉风格要求已纳入 Functional Requirements 与 NonFunctional Requirements。本节暂无独立 UX-DR 项。

### FR Coverage Map

FR1: Epic 1 - 选择、更改并加载本地工作目录。

FR2: Epic 2 - 多文件导入、重复识别、类型拒绝和 originals 登记。

FR3: Epic 2 - PDF/Office 转换与逐页 PNG 渲染。

FR4: Epic 2 - 页面图片内容哈希命名与冲突避免。

FR5: Epic 3 - 模型 provider、密钥、endpoint 和 model name 配置。

FR6: Epic 3 - 页面图片多模态分析、schema 校验、失败和重试。

FR7: Epic 2 - SQLite 与 JSONL 页面、任务、文档记录一致性。

FR8: Epic 4 - 基于页面分析构建中文可检索 BM25 索引。

FR9: Epic 4 - 搜索返回页面 JSON、图片地址、分数和来源信息。

FR10: Epic 4 - 全量重建索引、进度、失败保护和状态展示。

FR11: Epic 5 - localhost HTTP API 端点集合。

FR12: Epic 4 - SearchProvider/adapter 抽象与未来检索扩展。

FR13: Epic 1 - 第一屏工作台和工作台、搜索、设置主导航。

FR14: Epic 2 - 工作台文件导入、任务列表、状态、失败原因和处理入口。

FR15: Epic 4 - 搜索页输入、结果列表、预览、JSON、空状态和索引状态。

FR16: Epic 1 - 设置页字段、LibreOffice、模型、API、并发和隐私提示入口。

FR17: Epic 3 - `page_analysis_v1` 页面 JSON schema。

FR18: Epic 1 - 标准工作目录结构。

FR19: Epic 1 - 文档状态枚举。

FR20: Epic 1 - 页面状态枚举。

FR21: Epic 1 - 结构化错误记录。

## Epic List

### Epic 1: 本地工作区与桌面工具基础体验

用户可以打开应用后直接进入工作台，选择或切换本地工作目录，看到清晰的工作区结构、基础导航、设置入口、状态模型和错误记录能力，为后续导入、分析、检索提供可靠本地账本。

**FRs covered:** FR1, FR13, FR16, FR18, FR19, FR20, FR21

**实现备注:** 包含 Tauri React TypeScript scaffold、Rust AppState、SQLite migrations、workspace initialization/reconciliation、Settings 基础表单、状态枚举、错误模型和克制的桌面工具 UI。SQLite 是权威账本，文件和 JSONL 是 artifact；`page_id` 与 `image_hash` 必须分离。

### Story 1.1: 初始化 Tauri React TypeScript Starter 与主导航

**FRs implemented:** FR13

As a 本地文档处理用户,  
I want 基于官方 Tauri React TypeScript starter 初始化 slicer 桌面应用，并在启动后看到稳定可用的工作台首屏与主导航,  
So that 我可以从安全的项目基础进入工作台、搜索和设置，并为后续导入、分析、索引与 API 能力建立一致的桌面交互基础。

**Acceptance Criteria:**

**Given** 当前仓库已包含 `_bmad`、`_bmad-output`、`.agents`、`.claude`、`docs` 等规划和上下文工件  
**When** 开发者初始化官方 Tauri React TypeScript starter  
**Then** 初始化不得直接覆盖仓库根目录中的既有规划工件  
**And** 应先在临时或受控目录 scaffold，再合并 `package.json`、前端 `src/` 和 `src-tauri/` 结构，并保留既有 BMad/文档目录

**Given** starter 代码已合并到项目  
**When** 开发者检查初始工程配置  
**Then** 项目应包含可运行的 Tauri v2、React、TypeScript、Vite 基础结构和必要依赖脚本  
**And** starter 只作为应用壳基础，不得决定转换、分析、索引、存储、任务编排或 localhost API 的业务架构

**Given** 用户在 Windows 本机启动 slicer 桌面应用  
**When** 应用完成初始化并显示主窗口  
**Then** 首屏默认进入“工作台”视图，页面包含应用名称、当前工作区状态区域、主要操作入口占位，以及可识别的空状态  
**And** 应用窗口在未配置工作区时不得崩溃，必须显示“尚未选择工作区”的明确状态

**Given** 用户位于应用任意主视图  
**When** 用户点击主导航中的“工作台”“搜索”“设置”入口  
**Then** 应用应在不重启窗口的情况下切换到对应视图  
**And** 当前激活视图在导航中有明确的选中状态

**Given** 前端需要调用 Rust/Tauri 后端能力  
**When** 后续功能通过前端客户端发起命令调用  
**Then** 项目中应存在统一的 `tauriClient` 或等价客户端封装，用于集中处理 Tauri command 调用  
**And** 业务组件不得直接散落调用底层 Tauri invoke API

**Given** 开发者查看前端项目结构  
**When** 检查应用壳、路由/视图切换、共享组件和 Tauri 客户端封装  
**Then** 相关代码应按清晰模块组织，能够支撑后续工作台、搜索、设置页面继续扩展  
**And** Story 1.1 不应实现真实导入、搜索、模型分析、索引构建或 localhost API 行为，只保留必要入口与占位状态

**Given** 应用处于无工作区、加载中或基础错误状态  
**When** 主界面渲染这些状态  
**Then** 用户应看到可理解的中文状态文案  
**And** 状态展示应为后续统一错误模型与 Job 状态接入预留清晰位置

### Story 1.2: 选择并初始化本地工作目录

**FRs implemented:** FR1, FR18

As a 本地文档处理用户,  
I want 在桌面应用中选择或切换一个本地工作目录,  
So that slicer 可以在我指定的位置创建标准工作区结构，并在下次启动时恢复到可信的本地工作状态。

**Acceptance Criteria:**

**Given** 用户首次打开 slicer 且尚未配置工作区  
**When** 用户在工作台或设置页选择一个本地目录作为工作区  
**Then** 应用应在该目录下初始化 slicer 标准工作区结构  
**And** 至少创建或确认存在 `originals/`、`pages/`、`analysis/`、`indexes/`、`jobs/`、`logs/`、`tmp/`、`app.db` 等基础路径或文件

**Given** 用户选择的工作区目录为空目录或已有合法 slicer 工作区  
**When** 应用执行工作区初始化  
**Then** 初始化过程应是幂等的，重复选择同一目录不得破坏已有文件、数据库或 artifact  
**And** 应用应记录当前工作区路径，并在界面中显示已选择的工作区状态

**Given** 用户关闭并重新打开 slicer  
**When** 应用启动并读取最近一次工作区配置  
**Then** 应用应自动恢复最近使用的工作区  
**And** 如果该目录仍可访问，应显示可用状态；如果目录缺失或不可访问，应显示可恢复的错误状态并允许用户重新选择工作区

**Given** 用户选择一个不可写、无效或受权限限制的目录  
**When** 应用尝试初始化工作区  
**Then** 初始化必须失败且不得留下部分不可识别状态  
**And** 用户应看到中文错误信息，说明目录不可用或无法写入，并保留重新选择入口

**Given** 后续导入、分析、索引和 API 功能需要访问工作区  
**When** 开发者查看工作区初始化代码  
**Then** 工作区路径解析、目录创建和基础文件定位应由 Rust service 层集中处理  
**And** 前端不得自行拼接工作区内部路径或假设 artifact 目录结构

**Given** 应用已完成工作区初始化  
**When** 后续故事需要访问 SQLite、originals、pages、analysis、indexes、jobs、logs 或 tmp 位置  
**Then** 应存在可复用的 Workspace/AppState 基础能力返回这些路径  
**And** `page_id` 与 `image_hash` 的身份语义不得在本故事中混用或提前写死为同一个值

### Story 1.3: 建立 SQLite 权威账本与核心状态枚举

**FRs implemented:** FR19, FR20

As a 本地文档处理用户,  
I want slicer 使用本地 SQLite 账本记录工作区、文档、页面、任务和错误的核心状态,  
So that 应用重启、失败恢复和后续导入分析流程都能依赖一致、可追踪的本地状态来源。

**Acceptance Criteria:**

**Given** 用户已选择并初始化本地工作区  
**When** 应用首次打开该工作区的 `app.db`  
**Then** 应用应执行版本化 SQLite migration，创建当前阶段所需的最小权威账本 schema  
**And** migration 应可重复运行且不会破坏已有数据

**Given** 后续功能需要记录文档与页面生命周期  
**When** 开发者检查数据库 schema 与领域类型  
**Then** 系统应定义文档状态枚举，至少覆盖 `pending`、`importing`、`ready`、`failed` 等基础状态  
**And** 系统应定义页面状态枚举，至少覆盖 `pending`、`rendered`、`analysis_pending`、`analyzed`、`failed` 等基础状态

**Given** 系统需要区分页面发生位置与图片内容身份  
**When** schema 定义文档、页面记录和图片资产关系  
**Then** `page_id` 应表示某个文档中的页面 occurrence identity  
**And** `image_hash` 应表示页面图片内容 identity 与文件命名依据，二者不得被定义为同一个字段或同一个语义

**Given** 应用需要支撑后续导入、分析、索引和错误恢复  
**When** migration 创建当前阶段核心表  
**Then** schema 应只创建本故事直接需要的最小账本结构，例如 `settings`、`jobs`、`errors`、migration metadata 或等价基础表  
**And** `documents`、`page_records`、`image_assets`、`analysis_results`、`index_versions` 等业务表应在首次真正写入它们的后续故事中创建或扩展，不应在 Epic 1 一次性提前实现全部未来表

**Given** Rust 代码需要访问 SQLite 账本  
**When** 开发者查看数据访问实现  
**Then** 应存在 repository/service 边界，Tauri command 和前端不得直接访问数据库细节  
**And** 数据库读写应通过 Rust path-safe 工作区上下文定位 `app.db`

**Given** 应用写入状态、时间戳或跨层 DTO  
**When** 数据从 Rust service 返回给前端或未来 API  
**Then** 状态值应使用 `snake_case` 字符串  
**And** 时间戳应使用 RFC 3339 字符串，字段命名应保持 `snake_case`

**Given** migration 或数据库初始化失败  
**When** 应用启动或切换工作区时遇到该失败  
**Then** 应用不得继续把该工作区显示为可用  
**And** 用户应看到中文可恢复错误，并保留重新选择工作区或查看诊断信息的入口

### Story 1.4: 建立持久化 Job Orchestrator 基础能力

**FRs implemented:** FR21

As a 本地文档处理用户,  
I want slicer 将导入、分析、索引等长时间操作抽象为可持久化、可查询、可恢复的任务,  
So that 应用关闭、失败或重启后仍能清楚知道哪些工作已完成、失败或需要继续处理。

**Acceptance Criteria:**

**Given** 用户已选择可用工作区且 SQLite 账本已初始化  
**When** Rust service 创建一个后台任务  
**Then** 任务应写入本地 `jobs` 账本记录，包含任务类型、状态、进度、创建时间、更新时间和可选错误关联  
**And** 任务状态至少覆盖 `queued`、`running`、`succeeded`、`failed`、`cancelled` 等基础状态

**Given** 后续导入、分析和索引功能都需要执行长任务  
**When** 开发者查看任务编排代码  
**Then** 应存在统一 Job Orchestrator 或等价服务边界，用于创建、查询、更新和恢复任务  
**And** Tauri command 不得直接承载长时间业务流程，只能调用 service/orchestrator 并返回任务状态或任务标识

**Given** 用户在工作台查看当前处理状态  
**When** 应用查询任务列表或任务详情  
**Then** 前端应能显示任务类型、状态、进度、最近更新时间和失败摘要  
**And** 在本故事中可以使用示例/占位任务类型验证展示，但必须为后续真实导入、分析、索引任务复用同一结构

**Given** 应用在任务运行期间关闭或崩溃  
**When** 用户重新打开同一工作区  
**Then** Job Orchestrator 应识别上次遗留的 `running` 或不确定状态任务  
**And** 系统应将其标记为可恢复状态，例如 `failed` 或 `queued`，并保留清晰的恢复/失败原因记录，避免界面永久显示运行中

**Given** 后续功能需要更新任务进度  
**When** service 报告任务阶段、百分比或当前处理项  
**Then** Job Orchestrator 应提供统一进度更新接口  
**And** 进度值应可被前端轮询或订阅展示，且不得要求前端理解具体业务内部步骤

**Given** 任务执行失败  
**When** Orchestrator 记录失败状态  
**Then** 任务记录应关联结构化错误或失败摘要  
**And** 用户界面应展示中文可理解失败信息，并为后续重试入口预留位置

**Given** 任务状态从 Rust service 返回给前端或未来 HTTP API  
**When** 状态 DTO 被序列化  
**Then** 字段命名应使用 `snake_case`  
**And** 时间戳应使用 RFC 3339 字符串，状态值应使用稳定的 `snake_case` 枚举字符串

### Story 1.5: 建立统一错误模型、诊断日志与敏感信息保护

**FRs implemented:** FR21

As a 本地文档处理用户,  
I want slicer 在失败时提供一致、可理解且不会泄露密钥的错误与诊断信息,  
So that 我可以安全地定位问题、重试操作，并信任本地工具不会把模型 API key 或敏感内容暴露到日志、界面或导出文件中。

**Acceptance Criteria:**

**Given** Rust service、Tauri command 或未来 HTTP API 发生错误  
**When** 错误被转换为应用层响应  
**Then** 系统应使用统一 `AppError` 或等价错误模型承载错误信息  
**And** 错误模型至少包含 `code`、`message`、`stage`、`retryable`、`details`、`correlation_id` 字段或等价语义

**Given** 应用需要把错误返回给前端  
**When** Tauri command 返回失败结果  
**Then** 前端应收到结构化错误对象，而不是不可解析的字符串或调试栈  
**And** 用户界面应展示中文可理解错误摘要，并保留查看诊断详情的入口

**Given** 应用需要记录错误用于恢复和排查  
**When** 工作区已初始化且发生可记录错误  
**Then** 系统应将错误写入 `errors` 账本记录，包含错误 code、stage、retryable、correlation_id、发生时间和安全摘要  
**And** 错误记录可被 job、workspace 或后续业务实体引用

**Given** 开发者或用户需要排查运行问题  
**When** 应用写入诊断日志  
**Then** 日志应通过 `tracing` 或等价结构化日志机制生成  
**And** 日志应写入工作区 `logs/` 或应用诊断位置，并能通过 correlation_id 与错误记录关联

**Given** 用户在设置页保存模型 API key 或未来 API token  
**When** 应用持久化这些敏感值  
**Then** 密钥应通过 OS credential storage 或 `keyring` 保存  
**And** 密钥不得以明文进入 SQLite 普通字段、JSONL artifact、诊断日志、错误详情、搜索响应或前端持久状态

**Given** 错误、日志或诊断摘要中包含 URL、header、请求体、模型响应或配置项  
**When** 系统输出这些信息  
**Then** 应用必须对 API key、Authorization header、token 和类似 secret 字段执行 redaction  
**And** redaction 后的信息仍应保留足够上下文用于定位阶段、provider、endpoint 类别和失败原因

**Given** 后续模型分析功能可能保存原始模型响应摘要  
**When** 当前故事建立诊断基础  
**Then** 只允许保存安全截断且经过 redaction 的诊断摘要  
**And** 不得在本故事中实现真实模型调用或保存完整原始模型响应

**Given** 错误模型从 Rust service 返回给前端或未来 HTTP API  
**When** 错误对象被序列化  
**Then** 字段命名应使用 `snake_case`  
**And** HTTP API 未来应能映射为 `{ "error": { ... } }` 结构，Tauri 和 HTTP 不应各自发明不兼容错误格式

### Story 1.6: 完成设置页基础体验与工作台空状态

**FRs implemented:** FR1, FR13, FR16

As a 本地文档处理用户,  
I want 在设置页配置基础运行参数，并在工作台看到清晰的空状态和系统状态摘要,  
So that 我可以在真正导入文档前确认工作区、依赖、模型、API、并发和隐私相关配置是否处于可用状态。

**Acceptance Criteria:**

**Given** 用户打开 slicer 并进入“设置”视图  
**When** 设置页完成加载  
**Then** 页面应展示当前工作区路径与切换入口、LibreOffice 路径配置、模型 provider/base URL/model name/API key 状态、localhost API 启用状态占位、并发设置和隐私提示  
**And** 对尚未实现的真实模型调用、HTTP API 启动或 Office 转换能力，应清楚显示为“未配置”或“后续功能可用”，不得假装已完成

**Given** 用户需要更新基础设置  
**When** 用户编辑 LibreOffice 路径、模型 endpoint/model name、并发数量或隐私相关选项并保存  
**Then** 应用应将非敏感配置保存到 SQLite settings 或等价本地账本中  
**And** 保存后重新打开应用应恢复这些配置

**Given** 用户需要保存或更新模型 API key  
**When** 用户在设置页输入 API key 并保存  
**Then** 应用应通过 Story 1.5 建立的 keyring/credential storage 能力保存密钥  
**And** 设置页只显示密钥是否已配置或已更新，不得回显完整明文密钥

**Given** 用户尚未选择工作区  
**When** 用户进入工作台  
**Then** 工作台应显示中文空状态，说明需要先选择本地工作区  
**And** 页面应提供选择工作区的主要操作入口，并显示导入、任务、索引等区域的占位状态

**Given** 用户已选择工作区但尚未导入任何文档  
**When** 用户进入工作台  
**Then** 工作台应显示当前工作区可用、暂无文档、暂无任务、暂无索引的状态摘要  
**And** 导入入口可以作为占位或禁用说明存在，但不得实现 Epic 2 的真实导入流水线

**Given** 工作区、数据库、日志或设置保存发生错误  
**When** 用户在工作台或设置页触发相关操作  
**Then** 前端应使用统一错误展示组件显示中文错误摘要  
**And** 错误详情应包含 correlation_id 或诊断入口，且不得包含 API key/token 等敏感信息

**Given** 开发者检查前端状态管理  
**When** 查看工作台和设置页实现  
**Then** React 只能持有选中标签、表单草稿、加载中和错误展示等视图状态  
**And** 工作区、settings、job/error 状态的持久来源必须来自 Rust service 与 SQLite

**Given** 用户在常见桌面窗口尺寸下使用应用  
**When** 查看工作台、搜索占位页和设置页  
**Then** 界面应保持克制、清晰、适合桌面工具，不应出现营销式首屏、装饰性渐变或与当前功能无关的说明区块  
**And** 文本、按钮和状态区域不得互相遮挡，主导航应持续可用

### Epic 2: 文档导入与页面图片生成流水线

用户可以拖拽或选择 PDF、PPT、PPTX、DOC、DOCX，系统识别重复和不支持文件，将原文件纳入工作区，并把文档可恢复地转换为逐页 PNG 页面资产，在工作台中看到任务进度、失败原因和重试入口。

**FRs covered:** FR2, FR3, FR4, FR7, FR14

**实现备注:** 包含 originals 登记、原文件哈希、LibreOffice headless 转 PDF、PDF 渲染 PNG、图片内容哈希命名、原子写入、页面记录、JSONL 导出 artifact、持久化 Job Orchestrator，以及工作台任务列表。Office 转换失败或缺失 LibreOffice 必须是可恢复错误。

### Story 2.1: PDF 导入到页面 PNG 的端到端纵切片

**FRs implemented:** FR2, FR3, FR4, FR7, FR14

As a 本地文档处理用户,  
I want 从工作台导入单个 PDF 并生成逐页 PNG 图片,  
So that 我可以先验证 slicer 的核心文档切片流水线能从原始文件产生可追踪的页面资产。

**Acceptance Criteria:**

**Given** 用户已选择可用工作区且应用已完成 SQLite、Job Orchestrator 和错误模型初始化  
**When** 用户在工作台选择一个受支持的 PDF 文件进行导入  
**Then** 应用应创建一个导入任务并立即返回任务标识或任务状态  
**And** PDF 复制、页面渲染和账本写入必须由 Rust service/Job Orchestrator 执行，Tauri command 不得直接承载长时间导入流程

**Given** 导入任务开始处理 PDF 文件  
**When** 系统接收源文件  
**Then** 应用应计算原文件内容哈希，并将原文件保存到工作区 `originals/` 下的受控路径  
**And** SQLite `documents` 或等价账本记录应包含 `document_id`、原文件名、文件类型、原文件哈希、状态和创建时间

**Given** PDF 已登记为文档记录  
**When** 系统渲染 PDF 页面  
**Then** 应用应为每一页生成 PNG 页面图片并写入工作区页面资产位置  
**And** 每个页面都应创建 `page_records` 与 `image_assets` 或等价记录，包含 `page_id`、`document_id`、页码、`image_hash`、图片路径、页面状态和时间戳

**Given** 页面图片写入成功  
**When** 系统为页面生成身份字段  
**Then** `page_id` 应表示该文档中的页面 occurrence identity  
**And** `image_hash` 应表示 PNG 图片内容 identity 与文件命名依据，二者不得混用或互相覆盖

**Given** PDF 导入任务成功完成  
**When** 用户查看工作台任务列表或文档摘要  
**Then** 用户应看到导入成功、页数、文档名称和最近更新时间  
**And** 文档状态应变为可用于后续分析/索引的基础状态，例如 `ready` 或等价状态

**Given** PDF 文件损坏、加密不可读或渲染失败  
**When** 导入任务执行失败  
**Then** 任务状态应变为 `failed`，并关联结构化错误记录  
**And** 已完成的原子写入不得留下不可识别的半成品页面资产，用户应看到中文失败原因

**Given** 开发者验证该纵切片  
**When** 检查代码边界和测试  
**Then** PDF 导入应通过 import service、artifact store、repositories 和 Job Orchestrator 协作完成  
**And** 本故事不应实现 Office 转换、多文件批量导入、模型分析、BM25 索引或 localhost API

**Given** 开发者需要自动化验证 PDF 纵切片  
**When** 编写测试或夹具  
**Then** 应至少覆盖单页 PDF、多页 PDF、损坏 PDF 和中文文件名 PDF 的处理路径  
**And** PDF renderer 应通过 provider 边界或测试替身验证，不应让所有测试依赖某个不可控的系统级渲染环境

### Story 2.2: 多文件导入、文件类型校验与重复识别

**FRs implemented:** FR2, FR14

As a 本地文档处理用户,  
I want 一次选择或拖入多个文档并让系统识别不支持和重复文件,  
So that 我可以批量准备待处理文档，同时避免错误文件或重复内容污染工作区。

**Acceptance Criteria:**

**Given** 用户已选择可用工作区  
**When** 用户通过文件选择器或工作台拖拽区域提交多个文件  
**Then** 应用应逐项校验文件类型，只接受 PDF、PPT、PPTX、DOC、DOCX  
**And** 不支持的文件应被拒绝并显示中文原因，不得写入 `originals/`、文档账本或页面记录

**Given** 批量导入中包含多个受支持文件  
**When** 系统创建导入计划  
**Then** 每个可接受文件应生成独立文档记录和独立导入任务，或生成可追踪的批量任务子项  
**And** 工作台应能逐项显示 `queued`、`running`、`succeeded`、`failed`、`duplicate` 或等价状态

**Given** 用户导入与已有文档内容相同的文件  
**When** 系统计算原文件哈希并与账本比对  
**Then** 应用应识别重复文件并避免重复复制原文件和重复创建页面资产  
**And** 用户应看到该文件已重复的状态，以及关联的既有文档信息或安全摘要

**Given** 同一次批量导入中包含两个内容相同的文件  
**When** 系统处理该批导入  
**Then** 应用应只让其中一个文件进入真实导入流水线  
**And** 其他重复项应记录为重复/跳过状态，不得产生重复页面记录

**Given** 批量导入中部分文件有效、部分文件无效或重复  
**When** 导入计划执行  
**Then** 有效文件应继续处理，不应因其他文件失败而全部中止  
**And** 工作台应清楚区分成功、失败、拒绝和重复项

**Given** 开发者查看导入入口实现  
**When** 检查前端和 Rust 边界  
**Then** 前端只负责收集用户选择、展示状态和调用统一 `tauriClient`  
**And** 类型判断、哈希计算、重复识别、受控路径生成和账本写入必须由 Rust service 层完成

### Story 2.3: Office 文档转换为中间 PDF 并接入页面渲染

**FRs implemented:** FR3, FR7, FR14

As a 本地文档处理用户,  
I want 导入 DOC、DOCX、PPT、PPTX 时由应用转换为页面图片,  
So that 我可以用同一套工作流处理常见 Office 文档，而不只局限于 PDF。

**Acceptance Criteria:**

**Given** 用户已在设置中配置 LibreOffice 路径或系统可发现 LibreOffice  
**When** 用户导入 DOC、DOCX、PPT 或 PPTX 文件  
**Then** 应用应通过 LibreOffice headless 或等价 provider 将 Office 文件转换为中间 PDF  
**And** 转换任务必须通过 Job Orchestrator 报告阶段和进度，不得阻塞 GUI

**Given** Office 文件已成功转换为中间 PDF  
**When** 系统继续处理该文档  
**Then** 应用应复用 Story 2.1 的 PDF 页面渲染流水线生成 PNG 页面图片  
**And** 最终文档、页面、图片资产和任务状态应与 PDF 导入产物保持同一数据模型

**Given** 用户机器缺失 LibreOffice 或配置路径不可执行  
**When** 用户导入 Office 文档  
**Then** 应用应产生可恢复错误，说明 LibreOffice 不可用或路径无效  
**And** 文档/任务状态应允许用户在修正设置后重试，不得将问题表现为未知失败

**Given** LibreOffice 转换失败、超时或输出 PDF 缺失  
**When** 转换任务结束  
**Then** 应用应记录结构化错误、转换阶段、失败摘要和 correlation_id  
**And** 失败时应保留必要的中间诊断文件或日志引用，避免泄露用户文档内容到普通日志

**Given** Office 转换成功且页面渲染成功  
**When** 系统清理中间文件  
**Then** 中间 PDF 默认应删除  
**And** 如果 debug 设置要求保留中间 PDF，则应保存到受控临时/诊断位置并在账本中标记

**Given** 开发者查看转换实现  
**When** 检查模块边界  
**Then** Office 转换应通过 provider/service 边界封装，不能把 LibreOffice 调用散落在 UI 或 Tauri command 中  
**And** 本故事不应实现模型分析、搜索索引或 HTTP API

**Given** 开发者需要自动化验证 Office 转换行为  
**When** 编写 provider 测试  
**Then** 应提供 fake/test double 覆盖转换成功、LibreOffice 缺失、转换超时和输出 PDF 缺失  
**And** 默认测试不得要求开发者机器一定安装真实 LibreOffice

### Story 2.4: 页面图片资产写入、内容哈希命名与冲突保护

**FRs implemented:** FR4, FR7

As a 本地文档处理用户,  
I want 页面图片以内容哈希命名并被安全写入工作区,  
So that 后续分析、索引和预览都能引用稳定、可校验且不会相互覆盖的页面资产。

**Acceptance Criteria:**

**Given** PDF 或 Office 渲染过程产生一张页面 PNG  
**When** 应用将该图片写入工作区  
**Then** 系统应基于 PNG 内容计算 `image_hash` 并使用该哈希生成受控文件名  
**And** 图片路径必须由 Rust artifact store/path-safe API 生成，不得通过前端或手写字符串拼接

**Given** 页面图片正在写入磁盘  
**When** 写入操作执行  
**Then** 应用应优先使用临时文件加原子替换或等价安全写入策略  
**And** 如果写入失败，不得在正式页面资产目录留下不可识别的部分文件

**Given** 两个页面渲染出相同内容图片  
**When** 系统计算到相同 `image_hash`  
**Then** `image_assets` 或等价记录应允许复用同一图片资产  
**And** 每个 `page_records` 仍应保留独立 `page_id`、`document_id` 和页码，不能因为图片相同而合并页面 occurrence

**Given** 页面图片文件名发生哈希冲突或目标文件已存在但内容不一致  
**When** artifact store 检测到冲突  
**Then** 应用应拒绝覆盖并记录结构化错误  
**And** 受影响任务应失败为可诊断状态，提示用户查看诊断信息

**Given** SQLite 账本引用某个页面图片资产  
**When** 系统提交页面记录和图片资产记录  
**Then** 数据库中的 `page_id`、`image_hash`、图片相对路径、文件大小或校验信息应能对应到实际 PNG 文件  
**And** 数据库提交与文件写入顺序应避免出现账本指向不存在正式文件的成功状态

**Given** 后续模型分析和搜索预览需要读取页面图片  
**When** service 请求页面图片路径或元数据  
**Then** 应通过 artifact store 或 service 返回受控引用  
**And** 调用方不得直接猜测 `pages/` 下的文件布局

### Story 2.5: 导入任务进度、失败恢复与重试能力

**FRs implemented:** FR7, FR14

As a 本地文档处理用户,  
I want 导入、转换和渲染任务展示进度并支持失败后重试,  
So that 文档处理被中断或失败时，我可以理解当前状态并继续完成工作。

**Acceptance Criteria:**

**Given** 导入任务正在执行  
**When** 系统进入复制原文件、Office 转换、PDF 渲染、页面写入、账本提交等阶段  
**Then** Job Orchestrator 应更新任务阶段、进度、当前文件或当前页信息  
**And** 工作台应能展示这些进度而不需要理解具体实现细节

**Given** 用户关闭应用或应用崩溃时存在运行中的导入任务  
**When** 用户重新打开同一工作区  
**Then** 系统应通过 SQLite 账本识别未完成任务和相关文档/页面状态  
**And** 不确定状态不得永久显示为运行中，必须被标记为可恢复失败、可重试或需要 reconciliation 的状态

**Given** 导入任务在原文件复制、Office 转换、页面渲染或资产写入阶段失败  
**When** 系统记录失败  
**Then** 任务应关联结构化错误，包含失败阶段、retryable 标记和 correlation_id  
**And** 工作台应显示中文失败摘要和重试入口

**Given** 用户点击失败导入的重试入口  
**When** 失败类型被标记为可重试  
**Then** 应用应创建新的任务尝试继续或重新执行安全的导入步骤  
**And** 重试不得重复创建已确认成功的原文件、页面记录或图片资产

**Given** 失败原因是不可重试问题，例如不支持类型、源文件已删除、权限不足或文档损坏  
**When** 用户查看失败项  
**Then** 应用应禁用或解释重试不可用原因  
**And** 用户应能通过重新选择文件、修复设置或查看诊断继续处理

**Given** 重试过程中发现已有半成品 artifact 或不一致账本状态  
**When** reconciliation 或导入 service 检查这些状态  
**Then** 系统应优先依据 SQLite 权威账本和 artifact 校验结果决定恢复策略  
**And** 不得把 JSONL、前端内存状态或 job event 当作 source of truth

### Story 2.6: 页面、文档与任务 JSONL artifact 导出和一致性校验

**FRs implemented:** FR7

As a 本地文档处理用户,  
I want 导入结果生成可读且可重建的 JSONL 元数据 artifact,  
So that 我可以审查页面记录，并让后续分析、索引和诊断拥有稳定的数据出口。

**Acceptance Criteria:**

**Given** 一个或多个文档已成功导入并生成页面记录  
**When** 导入任务完成或用户触发元数据导出  
**Then** 应用应从 SQLite 权威账本生成或更新 `metadata/pages.jsonl` 或等价页面 JSONL artifact  
**And** JSONL 不得作为主状态源，只能作为从 SQLite 和文件资产重建出的 artifact

**Given** JSONL 写入正在执行  
**When** exporter 生成文件  
**Then** 应使用临时文件加原子替换或等价安全写入策略  
**And** 写入失败不得破坏上一版可用 JSONL

**Given** `metadata/pages.jsonl` 中包含页面记录  
**When** 开发者或后续服务读取记录  
**Then** 每条记录应包含可与 SQLite 对应的 `page_id`、`document_id`、页码、`image_hash`、图片路径、页面状态和时间戳  
**And** 字段名必须使用 `snake_case`，时间戳必须使用 RFC 3339 字符串

**Given** 文档和任务元数据也需要外部审查或调试  
**When** 本故事生成 JSONL artifact  
**Then** 可同时生成 `metadata/documents.jsonl`、`metadata/jobs.jsonl` 或等价文件，只要它们同样由 SQLite 重建  
**And** 这些文件不得包含 API key、token、未脱敏错误详情或完整敏感诊断内容

**Given** 用户或启动流程执行一致性校验  
**When** 系统比较 SQLite 页面记录、PNG 文件和 JSONL artifact  
**Then** 应能识别缺失图片、孤儿图片、过期 JSONL 或无法解析 JSONL  
**And** 识别结果应记录为可诊断状态，并允许后续通过重建 JSONL 或修复 artifact 恢复

**Given** 后续 Epic 3 和 Epic 4 需要消费页面基础数据  
**When** 它们请求页面列表或页面图片引用  
**Then** 应优先通过 service/repository 从 SQLite 获取权威数据  
**And** JSONL 可以用于调试、导出或重建校验，但不得绕过 service 边界成为业务读写入口

### Story 2.7: 工作台导入体验完善：任务列表、失败原因和处理入口

**FRs implemented:** FR2, FR3, FR14

As a 本地文档处理用户,  
I want 在工作台中完整查看导入入口、任务列表、文档状态、页面生成结果和失败处理操作,  
So that 我能清楚知道每个导入文件发生了什么，并从界面继续下一步处理。

**Acceptance Criteria:**

**Given** 用户打开已配置工作区的工作台  
**When** 工作台加载导入区域  
**Then** 页面应提供文件选择入口和拖拽区域，接受 PDF、PPT、PPTX、DOC、DOCX  
**And** 导入入口应使用统一 `tauriClient` 调用后端，不得在前端直接实现文件处理逻辑

**Given** 用户提交一个或多个文件导入  
**When** 后端返回任务或文件处理状态  
**Then** 工作台应展示任务列表，包含文件名、类型、状态、进度、页数、最近更新时间和失败摘要  
**And** 状态值应与 Job Orchestrator 和数据库状态保持一致，不得仅依赖 React 内存

**Given** 文档已经成功生成页面图片  
**When** 用户查看该文档导入结果  
**Then** 工作台应展示文档成功状态、生成页数和可用页面资产摘要  
**And** 可提供页面预览入口或占位，但不得实现 Epic 3 的模型分析结果或 Epic 4 的搜索结果体验

**Given** 某个导入项失败、重复或被拒绝  
**When** 用户查看任务列表  
**Then** 工作台应以中文显示失败/重复/拒绝原因  
**And** 对可重试失败显示重试入口，对不可重试失败显示修复建议或重新选择入口

**Given** 没有任何导入记录  
**When** 用户进入工作台  
**Then** 页面应显示空状态，提示用户拖入或选择文档开始导入  
**And** 空状态应保持桌面工具风格，避免营销式说明或与当前功能无关的装饰内容

**Given** 导入任务正在运行  
**When** 用户切换到搜索或设置再返回工作台  
**Then** 工作台应通过后端查询恢复任务状态  
**And** 不得因为页面切换丢失任务进度或显示过期状态

**Given** 工作台需要展示大量历史导入任务  
**When** 任务数量超出首屏  
**Then** 列表应保持可扫描、可滚动或分页，并优先显示最近任务  
**And** 文本、按钮和状态区域不得互相遮挡，常见桌面窗口尺寸下主导航应持续可用

**Given** 用户完成 Epic 2 的典型导入流程  
**When** 检查工作区内容和界面状态  
**Then** 原文件、页面 PNG、SQLite 记录、JSONL artifact 和任务状态应相互可追踪  
**And** Epic 2 不应实现模型分析、BM25 搜索或 localhost HTTP API 的业务行为

### Epic 3: 页面模型分析与可信 JSON 元数据

用户可以配置模型 provider、API key、base URL、custom endpoint 和 model name，并对页面图片执行多模态分析；系统生成符合 `page_analysis_v1` schema 的页面 JSON，失败时可定位、可重试，并确保密钥不泄露到日志、错误提示或导出文件。

**FRs covered:** FR5, FR6, FR17

**实现备注:** 包含模型 provider/endpoint 抽象、keyring 密钥保存、API key redaction、分析任务编排、schema 校验、`analysis_results` 入库、页面级 JSON 生成和失败重试。启用云端模型前必须让用户知道页面图片会发往配置的模型服务。

### Story 3.1: 配置模型 Provider、Endpoint 与密钥安全状态

**FRs implemented:** FR5

As a 本地文档处理用户,  
I want 在设置页配置用于页面图片分析的模型 provider、endpoint、model name 和 API key,  
So that 我可以用自己选择的云端 API 或自定义 HTTP endpoint 执行多模态分析，同时确保密钥不会泄露。

**Acceptance Criteria:**

**Given** 用户已选择可用工作区并打开设置页  
**When** 用户配置模型 provider、base URL、custom endpoint、model name 和分析并发数  
**Then** 应用应将非敏感配置保存到 SQLite settings 或等价本地账本  
**And** 重新打开应用后应恢复这些配置，字段名和 DTO 应使用 `snake_case`

**Given** 用户输入或更新模型 API key  
**When** 用户保存模型设置  
**Then** API key 应通过 OS credential storage 或 `keyring` 保存  
**And** SQLite、JSONL artifact、前端持久状态、日志和错误记录中不得保存完整明文 key

**Given** 模型配置尚未完成或 API key 缺失  
**When** 用户查看工作台分析入口  
**Then** 分析按钮应不可执行或在执行前提示需要完成模型配置  
**And** 提示应提供进入设置页的入口

**Given** 用户启用云端模型或自定义 HTTP endpoint  
**When** 应用首次允许执行页面分析  
**Then** 界面应显示隐私提示，说明页面图片会发送到用户配置的模型服务  
**And** 用户确认前不得启动真实模型调用

**Given** 应用需要读取模型密钥执行分析  
**When** Rust service 构建模型请求  
**Then** 密钥读取应集中在 secret store/security 边界中完成  
**And** 前端、Tauri command 和普通 repository 不得直接接触完整明文密钥

**Given** 配置保存、密钥读取或密钥写入失败  
**When** 用户保存设置或启动分析  
**Then** 应用应返回统一 `AppError` 结构并显示中文可恢复错误  
**And** 错误详情、correlation_id 和日志不得包含 API key 或 Authorization header

### Story 3.2: 建立 `page_analysis_v1` Schema、Prompt 契约与校验器

**FRs implemented:** FR6, FR17

As a 本地文档处理用户,  
I want slicer 对模型输出执行版本化 schema 校验和规范化,  
So that 只有可信、结构一致的页面 JSON 会进入本地账本和后续搜索索引。

**Acceptance Criteria:**

**Given** 开发者查看模型分析模块  
**When** 检查 schema 定义  
**Then** 项目应包含 `page_analysis_v1` 的结构化 schema 或等价强类型校验定义  
**And** schema 至少覆盖 `page_id`、`image_hash`、`image_path`、`source`、`analysis`、`retrieval`、`model`、`schema_version` 等核心字段

**Given** 模型返回 JSON 内容  
**When** 系统准备写入 `analysis_results`  
**Then** 输出必须先通过 `page_analysis_v1` schema 校验  
**And** 校验失败时不得写入成功分析结果，也不得进入后续索引构建链路

**Given** schema 中同时包含 `page_id` 与 `image_hash`  
**When** 校验器验证页面身份字段  
**Then** `page_id` 必须匹配 SQLite `page_records` 中的页面 occurrence identity  
**And** `image_hash` 必须匹配页面图片内容 identity，二者不得被校验器等同处理

**Given** 模型输出缺少检索文本  
**When** 输出仍包含可规范化的 title、summary、topics、visible_text 或 keywords  
**Then** 规范化逻辑可生成或补齐 `retrieval.bm25_text`  
**And** 生成规则应可测试、稳定，并保留原始结构化字段用于审查

**Given** 模型返回非法 JSON、错误字段类型、未知 schema version 或超长内容  
**When** 校验器处理输出  
**Then** 应返回结构化校验错误，包含失败路径、stage、retryable 标记和安全摘要  
**And** 日志只允许保存经过 redaction 和安全截断的诊断摘要

**Given** 后续版本可能升级分析 schema  
**When** 开发者查看 schema 模块  
**Then** schema version 应显式记录并可扩展  
**And** Story 3.2 不应实现 BM25 索引构建、搜索体验或 localhost API

### Story 3.3: 单页图片分析端到端纵切片

**FRs implemented:** FR6, FR17

As a 本地文档处理用户,  
I want 对单个已渲染页面图片执行多模态分析并得到可信 JSON,  
So that 我可以验证从页面图片、模型 provider、schema 校验到结果入库的核心分析链路。

**Acceptance Criteria:**

**Given** 工作区中存在已导入且页面图片可访问的 `page_record`  
**When** 用户或 service 请求分析单个页面  
**Then** 应用应创建分析任务或任务子项，并通过 Job Orchestrator 报告状态  
**And** 长时间模型调用不得直接在 Tauri command 中执行

**Given** 模型配置已完成且用户已确认隐私提示  
**When** AnalysisService 分析页面图片  
**Then** 系统应通过 `ModelProvider` trait 或等价 provider 边界调用具体模型实现  
**And** MVP 至少应提供 custom/cloud HTTP provider 及一个测试替身或 mock provider

**Given** provider 构建模型请求  
**When** 请求包含页面图片、prompt、model name 和 endpoint 配置  
**Then** prompt 应要求模型返回符合 `page_analysis_v1` 的 JSON  
**And** 请求日志不得记录完整 API key、Authorization header 或未脱敏图片/响应内容

**Given** 模型返回符合要求的 JSON  
**When** schema 校验通过  
**Then** 应用应将分析结果写入 `analysis_results` 或等价账本记录，包含 `page_id`、schema version、provider、model name、状态、结果 JSON 和时间戳  
**And** 对应页面分析状态应更新为 `analyzed` 或等价成功状态

**Given** 模型返回结果中的 source 或图片字段与当前页面不一致  
**When** AnalysisService 校验结果上下文  
**Then** 系统应拒绝该结果并记录结构化错误  
**And** 不得让错误页面身份进入 `analysis_results`

**Given** 单页分析成功完成  
**When** 用户查看工作台或页面详情占位  
**Then** 用户应能看到该页已分析成功和基础摘要状态  
**And** 本故事不应实现全文搜索、索引构建或 HTTP 查询行为

### Story 3.4: 新页面批量分析与单文档重新分析

**FRs implemented:** FR6

As a 本地文档处理用户,  
I want 对新页面批量执行分析，并能对单个文档重新分析,  
So that 我可以高效处理导入后的页面，同时在 prompt 或模型配置变化后刷新某个文档的分析结果。

**Acceptance Criteria:**

**Given** 工作区中存在多个已渲染但尚未分析的页面  
**When** 用户点击分析入口或系统启动分析任务  
**Then** 应用应只选择新页面或被明确标记为需重跑的页面进入分析队列  
**And** 已有有效分析结果的页面不得被无理由重复调用模型

**Given** 用户选择对单个文档重新分析  
**When** 应用创建重新分析任务  
**Then** 系统应将该文档下所有页面标记为待分析或创建新的分析版本  
**And** 不应影响其他文档的有效分析结果

**Given** 批量分析任务正在运行  
**When** 多个页面等待分析  
**Then** Job Orchestrator 应报告总页数、已完成页数、失败页数、当前阶段和最近更新时间  
**And** 并发度应遵守设置页中的分析并发数

**Given** 批量分析中部分页面成功、部分页面失败  
**When** 任务结束或暂停  
**Then** 成功页面应保留已校验的分析结果  
**And** 失败页面应保留失败状态和可重试信息，不得导致整个文档所有页面回滚为失败

**Given** 应用关闭或崩溃时存在运行中的分析任务  
**When** 用户重新打开同一工作区  
**Then** 系统应通过 SQLite 识别未完成分析任务和页面状态  
**And** 不确定状态应被标记为可恢复失败、可重试或重新排队，不能永久显示运行中

**Given** 批量分析任务完成  
**When** 用户查看工作台任务列表或文档摘要  
**Then** 应显示文档分析进度、成功页数、失败页数和最近分析时间  
**And** 界面应为失败页面提供单独重试入口

### Story 3.5: 分析失败处理、单页重试与安全诊断

**FRs implemented:** FR6

As a 本地文档处理用户,  
I want 非法 JSON、超时、API 错误或网络失败的页面分析能被清楚记录并单独重试,  
So that 我可以修复配置或服务问题后继续处理失败页面，而不用重做全部文档。

**Acceptance Criteria:**

**Given** 模型调用发生超时、网络错误、HTTP 错误、provider 错误、非法 JSON 或 schema 校验失败  
**When** AnalysisService 捕获失败  
**Then** 页面分析状态应更新为失败或待重试状态  
**And** 任务和页面应关联结构化 `AppError`，包含 stage、retryable、correlation_id 和中文安全摘要

**Given** 失败原因包含 API response、headers、endpoint、prompt 摘要或模型输出片段  
**When** 系统写入日志、错误详情或诊断摘要  
**Then** 应执行 redaction，移除 API key、Authorization header、token 和类似 secret 字段  
**And** 原始模型响应只允许以安全截断摘要保存，不得完整写入普通日志或 JSONL

**Given** 某个页面分析失败且被标记为可重试  
**When** 用户点击单页重试  
**Then** 应用应只为该页面创建新的分析尝试  
**And** 其他页面的有效分析结果不得被覆盖或删除

**Given** 失败原因是配置缺失、API key 无效或 endpoint 不可达  
**When** 用户查看失败页面或任务  
**Then** 界面应提示用户检查模型设置或网络/provider 状态  
**And** 在配置修复前可以阻止继续批量重试，避免重复产生同类失败

**Given** 重试后模型返回有效 `page_analysis_v1` JSON  
**When** schema 校验通过并入库  
**Then** 页面状态应从失败变为已分析  
**And** 历史失败记录应保留用于诊断，但当前有效分析结果应明确可识别

**Given** 开发者编写测试  
**When** 使用 mock provider 模拟超时、非法 JSON、API 错误和成功重试  
**Then** 每类失败都应映射到可测试的错误 code 和 retryable 行为  
**And** 测试不得需要真实外部模型服务或真实 API key

### Story 3.6: 分析结果持久化、页面 JSON 生成与 JSONL 一致性

**FRs implemented:** FR6, FR17

As a 本地文档处理用户,  
I want 已校验的页面分析结果被持久化，并生成可审查的页面 JSON/JSONL artifact,  
So that 后续搜索索引和外部诊断能使用一致、可追踪且不泄露密钥的页面元数据。

**Acceptance Criteria:**

**Given** 页面分析结果通过 `page_analysis_v1` schema 校验  
**When** 系统保存结果  
**Then** 应写入 SQLite `analysis_results` 或等价账本记录  
**And** 记录应包含 `page_id`、schema version、provider、model name、分析状态、结果 JSON、创建/更新时间和可选错误关联

**Given** 同一页面被重新分析  
**When** 新分析结果通过校验  
**Then** 系统应明确处理分析版本或当前有效结果指针  
**And** 不得让旧失败结果覆盖新的有效分析结果

**Given** 已有页面基础记录、图片资产和分析结果  
**When** 应用生成页面级 JSON artifact  
**Then** 输出应组合 `page_id`、`image_hash`、`image_path`、source、analysis、retrieval、model 和 `schema_version`  
**And** 字段命名必须使用 `snake_case`，时间戳必须使用 RFC 3339 字符串

**Given** 应用更新 `metadata/pages.jsonl` 或分析相关 JSONL artifact  
**When** exporter 写入文件  
**Then** JSONL 应从 SQLite 权威账本和页面图片资产重建  
**And** 应使用原子写入策略，写入失败不得破坏上一版可用 artifact

**Given** 页面 JSON 或 JSONL 中包含模型配置摘要  
**When** 导出 artifact  
**Then** 可以包含 provider 和 model name  
**And** 不得包含 API key、Authorization header、完整请求体、完整原始模型响应或未脱敏错误详情

**Given** 后续 Epic 4 需要构建 BM25 索引  
**When** 索引服务读取页面分析数据  
**Then** 应能通过 service/repository 获取已校验且当前有效的分析结果和 `retrieval.bm25_text`  
**And** Epic 3 不应实现具体 BM25 索引构建或搜索 UI

### Story 3.7: 工作台分析体验完善：分析入口、进度、结果摘要与重试

**FRs implemented:** FR5, FR6

As a 本地文档处理用户,  
I want 在工作台中启动页面分析、查看分析进度、结果摘要和失败重试入口,  
So that 我可以从导入完成自然进入可信的页面理解流程。

**Acceptance Criteria:**

**Given** 用户打开已配置工作区的工作台  
**When** 存在已渲染但未分析的页面  
**Then** 工作台应显示可用的分析入口、待分析页数和相关文档摘要  
**And** 如果模型配置缺失或隐私提示未确认，分析入口应禁用或引导用户完成配置

**Given** 用户启动分析任务  
**When** 后端返回任务状态  
**Then** 工作台应展示分析任务列表或任务详情，包含状态、进度、成功页数、失败页数、当前阶段和最近更新时间  
**And** 状态应来自 Rust service/SQLite 查询，不得只依赖 React 内存

**Given** 分析任务正在运行  
**When** 用户切换到搜索或设置再返回工作台  
**Then** 工作台应通过后端查询恢复最新任务和页面分析状态  
**And** 页面切换不得导致分析进度丢失或显示过期状态

**Given** 某个页面已分析成功  
**When** 用户查看文档或页面摘要  
**Then** 界面应展示标题、摘要、关键词、可见文本数量或基础分析状态  
**And** 不应在 Epic 3 中实现 BM25 搜索排序或搜索结果页

**Given** 某些页面分析失败  
**When** 用户查看工作台分析状态  
**Then** 界面应显示中文失败原因、失败页数、correlation_id 或诊断入口  
**And** 对可重试页面显示单页重试或失败项重试入口

**Given** 用户查看设置或工作台中的模型相关状态  
**When** API key 已配置  
**Then** 界面只显示已配置/可更新状态，不得回显完整明文 key  
**And** 错误提示、任务事件和页面摘要不得包含 secret

**Given** 用户在常见桌面窗口尺寸下使用分析体验  
**When** 查看分析入口、进度和结果摘要  
**Then** 界面应保持克制、清晰、适合桌面工具  
**And** 文本、按钮和状态区域不得互相遮挡，主导航应持续可用

### Epic 4: 本地 BM25 索引与搜索体验

用户可以基于页面分析结果构建或重建本地 BM25 索引，在搜索页用中文关键词检索页面，看到相关性排序、分数、来源文档、页面图片预览和页面 JSON；索引不可用或正在重建时界面有明确状态提示。

**FRs covered:** FR8, FR9, FR10, FR12, FR15

**实现备注:** 包含 `SearchProvider` 抽象、`TantivyBm25SearchProvider`、中文 tokenizer/analyzer 策略、索引 build 目录与 active index 原子切换、重建失败不破坏旧索引、搜索结果 DTO、GUI 搜索页、图片预览和 JSON 查看区。查询接口不能硬编码成只支持 BM25。

### Story 4.1: 建立 SearchProvider 抽象与索引状态基础

**FRs implemented:** FR10, FR12, FR15

As a 本地文档处理用户,  
I want slicer 的搜索能力建立在可替换的检索 provider 和清晰的索引状态之上,  
So that 第一版可以使用 BM25，同时未来能扩展 qmd、wiki、向量或混合检索而不重写业务入口。

**Acceptance Criteria:**

**Given** 开发者查看检索模块  
**When** 检查 Rust 后端结构  
**Then** 项目应定义 `SearchProvider` 或等价 trait/adapter，覆盖索引构建、查询、健康检查和 provider 标识  
**And** `SearchService` 应通过该抽象调用检索能力，不得直接耦合具体 Tantivy 实现

**Given** MVP 使用内置 BM25 检索  
**When** 应用初始化 provider registry 或检索服务  
**Then** 默认 provider 应为 `TantivyBm25SearchProvider` 或等价实现  
**And** 查询接口、service DTO 和未来 API contract 不得命名或设计成只能支持 BM25

**Given** 工作区中需要记录索引状态  
**When** 初始化或迁移 SQLite schema  
**Then** `index_versions` 或等价账本结构应记录 provider、版本 ID、状态、目录、构建时间、激活时间和错误关联  
**And** 状态值应使用稳定的 `snake_case` 字符串，例如 `not_built`、`building`、`ready`、`failed`

**Given** 用户切换工作区或应用启动  
**When** 检索服务加载当前工作区  
**Then** 应从 SQLite 权威账本读取 active index 状态  
**And** 不得仅根据 `indexes/` 文件夹是否存在判断索引可用

**Given** 索引不可用、缺失或 provider 初始化失败  
**When** 用户进入搜索页或工作台索引区域  
**Then** 应返回明确索引状态供前端展示  
**And** 错误应使用统一 `AppError`，包含 stage、retryable 和 correlation_id

**Given** 开发者编写 provider 测试  
**When** 使用 mock search provider  
**Then** `SearchService` 应能在不依赖 Tantivy 的情况下测试查询、状态和错误映射  
**And** 本故事不应实现完整 BM25 构建、搜索 UI 或 HTTP API

### Story 4.2: 实现 Tantivy BM25 Provider 与中文 Analyzer 策略

**FRs implemented:** FR8, FR12

As a 本地文档处理用户,  
I want 本地 BM25 索引能够检索中文标题、摘要、可见文字、关键词和来源文件名,  
So that 我用中文关键词搜索时可以找到相关页面。

**Acceptance Criteria:**

**Given** MVP 默认检索 provider 为 Tantivy BM25  
**When** 开发者实现 `TantivyBm25SearchProvider`  
**Then** provider 应封装 Tantivy schema、index writer、reader/searcher 和 query parser 细节  
**And** 上层 service 只能通过 `SearchProvider` 调用它

**Given** 页面分析结果包含中文内容  
**When** provider 构建索引 schema 和 analyzer  
**Then** 应显式实现中文 tokenizer/analyzer 策略，例如 jieba、字符 n-gram 或混合策略  
**And** 该策略应记录在代码/测试中，不能依赖 Tantivy 默认英文分词作为隐式行为

**Given** 页面分析字段包含 title、summary、visible_text、topics、keywords、retrieval.bm25_text 和来源文件名  
**When** provider 准备索引文档  
**Then** 索引文本应覆盖这些字段  
**And** 不得把 API key、错误详情、完整原始模型响应或非检索诊断内容写入索引

**Given** 中文文件名、中文路径或路径包含空格  
**When** provider 索引来源文档和页面图片引用  
**Then** 索引构建和搜索结果返回应保持路径安全  
**And** 不得因中文或空格路径导致索引失败

**Given** 开发者运行检索测试  
**When** 使用中文样例页面分析结果  
**Then** 搜索标题、摘要、可见文字、关键词和来源文件名中的中文词应能命中页面  
**And** 搜索结果应按相关性排序，至少在测试夹具中验证高相关页面排在低相关页面之前

**Given** analyzer 策略未来可能调整  
**When** provider 记录 index version 元数据  
**Then** 应保存 analyzer/provider 版本或等价元数据  
**And** 未来 analyzer 改动应能触发重建，而不是悄悄复用旧索引

### Story 4.3: 从可信页面分析结果构建首个 BM25 索引

**FRs implemented:** FR8, FR10

As a 本地文档处理用户,  
I want slicer 基于已校验的页面分析结果构建本地 BM25 索引,  
So that 我可以在本机搜索已经分析完成的页面内容。

**Acceptance Criteria:**

**Given** 工作区中存在通过 `page_analysis_v1` 校验并入库的页面分析结果  
**When** 用户触发首次构建索引或系统检测需要构建索引  
**Then** 应用应创建索引构建任务并通过 Job Orchestrator 报告状态  
**And** 构建过程不得阻塞 GUI，Tauri command 只返回任务标识或任务状态

**Given** 索引构建任务读取数据  
**When** `SearchService` 准备构建输入  
**Then** 应只使用 SQLite/repository 中当前有效且 schema 校验通过的分析结果  
**And** 不得把 `metadata/pages.jsonl`、前端状态或未校验模型输出当作 source of truth

**Given** 某些页面尚未分析、分析失败或缺少 `retrieval.bm25_text`  
**When** 构建索引输入集合  
**Then** 系统应跳过不可索引页面或标记为不可索引状态  
**And** 任务摘要应显示已索引页数、跳过页数和失败原因

**Given** 构建任务向 provider 写入索引  
**When** Tantivy provider 接收文档集合  
**Then** 每个索引文档应保留 `page_id`、`document_id`、页码、图片引用和页面 JSON 引用所需字段  
**And** 搜索命中后必须能追溯到原始文档、页码、页面图片和结构化 JSON

**Given** 首次索引构建成功  
**When** 构建结果通过验证  
**Then** `index_versions` 应记录 ready 状态和 active version  
**And** 搜索页应能识别当前索引可用

**Given** 首次索引构建失败  
**When** provider 或文件写入返回错误  
**Then** 任务和 index version 应记录 failed 状态与结构化错误  
**And** 用户界面应显示中文失败摘要和重试入口

### Story 4.4: 全量索引重建、Active Index 原子切换与失败保护

**FRs implemented:** FR10

As a 本地文档处理用户,  
I want 在页面分析更新后全量重建 BM25 索引且失败不破坏旧索引,  
So that 我可以安全刷新检索结果，不会因为一次重建失败而失去已有搜索能力。

**Acceptance Criteria:**

**Given** 已存在一个可用 active index  
**When** 用户触发全量重建索引  
**Then** 应用应创建 `index_rebuild` 或等价任务，并在新的临时 build 目录中构建索引  
**And** 重建过程不得直接写入或覆盖当前 active index 目录

**Given** 重建任务正在执行  
**When** provider 写入索引文件  
**Then** 索引应写入类似 `indexes/bm25/build-<id>/` 的受控目录  
**And** 路径应由 artifact store/path-safe API 生成，不能手写字符串拼接

**Given** 新索引构建完成  
**When** 系统准备切换 active index  
**Then** 应先验证新索引可打开、文档数量符合预期且基本查询可执行  
**And** 只有验证通过后才能在 `index_versions` 中切换 active version

**Given** 新索引构建或验证失败  
**When** 任务结束  
**Then** 旧 active index 必须继续保持可用  
**And** 失败 build 目录应被安全标记、保留诊断或清理，用户应看到中文失败原因

**Given** 重建索引过程中存在页面图片和页面 JSON artifact  
**When** 重建任务失败或成功  
**Then** 应用不得删除原图片或页面 JSON  
**And** 索引可以重建，但不能成为唯一元数据来源

**Given** 应用在索引重建期间关闭或崩溃  
**When** 用户重新打开同一工作区  
**Then** 系统应识别未完成 build 目录和 `building` 状态 index version  
**And** 不确定状态应标记为 failed 或 needs_rebuild，不能破坏旧 active index

### Story 4.5: SearchService 查询结果 DTO 与页面可追溯返回

**FRs implemented:** FR9, FR12

As a 本地文档处理用户,  
I want 搜索结果返回页面 JSON、图片地址、来源文档、页码和相关分数,  
So that 我可以从命中项快速判断结果是否有用并追溯原始页面。

**Acceptance Criteria:**

**Given** 工作区存在 ready 的 active index  
**When** 用户提交关键词查询和 limit 参数  
**Then** `SearchService` 应通过 `SearchProvider` 查询 active index  
**And** 查询结果应按相关性分数排序返回

**Given** provider 返回命中的 `page_id` 和 score  
**When** `SearchService` 组装结果 DTO  
**Then** 每条结果应包含 `page_id`、`document_id`、来源文件名、页码、页面图片地址/引用、相关分数和页面 JSON  
**And** 字段命名应使用 `snake_case`，时间戳应使用 RFC 3339 字符串

**Given** 搜索结果需要返回页面 JSON  
**When** service 读取分析结果  
**Then** 应返回当前有效且通过 `page_analysis_v1` 校验的页面 JSON  
**And** 不得返回未校验模型输出、完整原始模型响应或未脱敏错误详情

**Given** 搜索命中页面的图片文件缺失或 artifact 校验失败  
**When** service 组装结果  
**Then** 结果应明确标记图片不可用或返回结构化错误/警告  
**And** 不得因单个图片缺失导致整个搜索请求崩溃

**Given** active index 不存在、正在构建或已失败  
**When** 用户发起搜索  
**Then** service 应返回明确的索引不可用状态或结构化 `AppError`  
**And** 前端应能据此展示建立/重建索引入口

**Given** 查询为空、只包含空白或 limit 非法  
**When** service 校验查询参数  
**Then** 应返回可理解的验证结果或空结果状态  
**And** 不得把无效查询直接传入 provider 造成不可诊断错误

### Story 4.6: 搜索页基础体验：输入、结果列表、图片预览与 JSON 查看

**FRs implemented:** FR9, FR15

As a 本地文档处理用户,  
I want 在搜索页输入关键词并查看结果列表、页面图片预览和页面 JSON,  
So that 我可以用桌面 GUI 检索并审查文档页面。

**Acceptance Criteria:**

**Given** 用户进入“搜索”视图且 active index 可用  
**When** 用户输入中文或英文关键词并执行搜索  
**Then** 页面应展示按相关性排序的结果列表  
**And** 每条结果应显示标题/摘要、来源文档、页码、分数和基础命中信息

**Given** 用户选择某条搜索结果  
**When** 搜索页加载详情区域  
**Then** 应显示页面图片预览  
**And** 图片引用必须来自后端 service 返回的受控路径/URL，不得由前端猜测文件布局

**Given** 用户需要查看结构化结果  
**When** 用户打开 JSON 查看区域  
**Then** 应展示该页面的 `page_analysis_v1` JSON  
**And** JSON 中不得包含 API key、token、完整原始模型响应或未脱敏错误详情

**Given** 用户提交查询但没有命中结果  
**When** SearchService 返回空结果  
**Then** 搜索页应显示中文空状态  
**And** 空状态应区分“没有匹配结果”和“索引不可用”

**Given** 搜索请求正在执行  
**When** 后端尚未返回结果  
**Then** 搜索页应显示加载状态并保持主导航可用  
**And** 用户可以修改查询并重新提交，不应导致界面卡死

**Given** 搜索请求失败  
**When** service 返回结构化错误  
**Then** 搜索页应显示中文错误摘要、correlation_id 或诊断入口  
**And** 错误展示不得泄露路径以外的敏感配置或 secret

**Given** 用户在常见桌面窗口尺寸下使用搜索页  
**When** 查看输入区、列表、预览和 JSON 区域  
**Then** 界面应保持可扫描、克制、适合桌面工具  
**And** 文本、按钮、预览和 JSON 面板不得互相遮挡

### Story 4.7: 索引重建入口、状态展示与搜索体验收口

**FRs implemented:** FR10, FR15

As a 本地文档处理用户,  
I want 在工作台或搜索页看到索引状态并触发重建,  
So that 当分析数据变化或索引不可用时，我可以主动恢复搜索能力。

**Acceptance Criteria:**

**Given** 用户打开工作台或搜索页  
**When** 应用加载索引状态  
**Then** 界面应显示索引是否未构建、构建中、可用、失败或需要重建  
**And** 状态应来自 Rust service/SQLite `index_versions`，不得只依赖前端内存

**Given** 存在已分析但未索引的页面或 schema/analyzer/provider 版本变化  
**When** 用户查看索引状态  
**Then** 应用应提示索引需要构建或重建  
**And** 用户应能通过明确入口触发全量重建索引

**Given** 用户触发索引重建  
**When** 后端创建重建任务  
**Then** 前端应展示任务状态、进度、已处理页数、跳过页数、失败摘要和最近更新时间  
**And** 重建任务必须走 Job Orchestrator，不得阻塞 GUI

**Given** 索引正在重建  
**When** 用户进入搜索页  
**Then** 搜索页应显示“索引正在重建”的状态提示  
**And** 如果旧 active index 仍可用，界面应明确当前搜索结果可能来自旧索引；如果无旧索引，应禁用搜索或显示不可用状态

**Given** 索引重建失败  
**When** 用户查看索引状态  
**Then** 界面应显示中文失败原因、correlation_id 或诊断入口  
**And** 如果旧索引仍可用，搜索页应继续允许使用旧索引；如果没有旧索引，应提供重试入口

**Given** 用户重试失败的索引重建  
**When** 失败类型被标记为可重试  
**Then** 应用应创建新的重建任务并使用新的 build 目录  
**And** 重试不得删除页面图片、页面 JSON、分析结果或旧 active index

**Given** Epic 4 结束时用户完成典型流程  
**When** 检查工作区和 GUI  
**Then** 用户应能从已分析页面构建索引、搜索中文关键词、查看结果图片和 JSON，并安全重建索引  
**And** Epic 4 不应实现 localhost HTTP API 端点或 token 保护行为，这些由 Epic 5 承担

### Epic 5: Localhost HTTP API 与外部自动化访问

用户或本机工具可以通过默认仅监听 `127.0.0.1` 的 HTTP API 查询健康状态、搜索结果、页面、文档，并触发受 token 保护的索引重建；GUI 与 API 使用同一套 Rust service layer，避免业务逻辑分叉。

**FRs covered:** FR11

**实现备注:** 包含 axum API、`GET /health`、`GET /search`、`GET /pages/{page_id}`、`GET /documents/{document_id}`、`POST /indexes/rebuild`、token 保护、统一成功/错误响应结构，以及 Tauri commands 和 HTTP handlers 共享 application services。

### Story 5.1: 嵌入式 Axum Localhost API 服务基础与设置控制

**FRs implemented:** FR11

As a 本地自动化用户,  
I want slicer 提供默认仅监听本机地址的 HTTP API 服务,  
So that 我可以从本机脚本或工具安全地访问页面、搜索和索引能力。

**Acceptance Criteria:**

**Given** 用户已选择可用工作区  
**When** 用户在设置页启用 localhost API  
**Then** 应用应启动嵌入式 `axum` HTTP 服务  
**And** 默认监听地址必须是 `127.0.0.1`，不得默认绑定 `0.0.0.0` 或公网地址

**Given** 用户查看 API 设置  
**When** API 服务已启用或禁用  
**Then** 设置页应显示启用状态、bind address、port 和 token reset action  
**And** 非技术用户应能看懂当前 API 是否可用

**Given** 用户修改 API port 或启停状态  
**When** 用户保存设置  
**Then** 应用应将非敏感 API 设置保存到 SQLite settings 或等价账本  
**And** 重启应用后应恢复设置并按配置决定是否启动 API 服务

**Given** API 服务启动失败，例如端口占用或 bind 地址不可用  
**When** 应用尝试启动服务  
**Then** 系统应返回统一 `AppError` 并在设置页显示中文错误摘要  
**And** 错误详情应包含 stage 和 correlation_id，不得泄露 token 或其他 secret

**Given** 应用关闭、切换工作区或用户禁用 API  
**When** API 服务需要停止  
**Then** 系统应优雅关闭 server task 并释放端口  
**And** 前端状态应能通过 service 查询确认 API 已停止

**Given** 开发者查看 API server 模块  
**When** 检查后端结构  
**Then** API server 启停应由 `api/server.rs`、`api_server_service.rs` 或等价边界管理  
**And** 该故事不应实现具体业务 endpoints 的完整返回，只建立可启动、可停止、可配置的 axum 服务基础

### Story 5.2: 统一 HTTP DTO、成功响应与 AppError 错误映射

**FRs implemented:** FR11

As a 本地自动化用户,  
I want HTTP API 使用稳定一致的成功和错误响应格式,  
So that 我的脚本可以可靠解析结果和失败原因。

**Acceptance Criteria:**

**Given** HTTP endpoint 成功返回业务数据  
**When** API 序列化响应  
**Then** 成功响应应使用显式对象结构，例如 `{ "data": { ... } }`  
**And** 字段命名必须使用 `snake_case`

**Given** HTTP endpoint 发生业务、验证、工作区或内部错误  
**When** route handler 返回失败  
**Then** 错误响应应使用 `{ "error": { ... } }` 结构  
**And** error 对象应映射自统一 `AppError`，包含 `code`、`message`、`stage`、`retryable`、`details`、`correlation_id` 或等价字段

**Given** API 需要返回状态枚举、任务状态或索引状态  
**When** DTO 被序列化  
**Then** 状态值应使用稳定 `snake_case` 字符串  
**And** 时间戳应使用 RFC 3339 字符串

**Given** HTTP route handler 需要执行业务逻辑  
**When** 开发者查看 `api/routes/*`  
**Then** handler 只能做 HTTP 参数解析、认证、DTO 映射和调用 services  
**And** handler 不得直接访问 repositories、providers、artifact store、SQLite 或文件系统 helper

**Given** Tauri commands 和 HTTP API 调用同一业务能力  
**When** 比较它们的错误与状态语义  
**Then** 两者应共享内部 service 和 `AppError` 映射  
**And** 不应出现 GUI 成功但 HTTP 失败语义不同的分叉业务逻辑

**Given** 响应或错误详情包含路径、headers、token、API key 或诊断摘要  
**When** API 返回 JSON  
**Then** 响应必须执行 redaction，避免泄露 token、Authorization header、模型 API key 或未脱敏诊断内容  
**And** 搜索响应和页面 JSON 不得包含 secret

### Story 5.3: GET /health 返回应用、工作区与索引状态

**FRs implemented:** FR11

As a 本地自动化用户,  
I want 通过 `GET /health` 检查 slicer 服务、工作区和索引状态,  
So that 我的外部工具可以在调用搜索或页面接口前确认应用是否可用。

**Acceptance Criteria:**

**Given** localhost API 已启用  
**When** 本机工具请求 `GET /health`  
**Then** API 应返回成功响应 `{ "data": { ... } }`  
**And** data 至少包含服务状态、工作区是否就绪、索引状态、API 版本或等价元数据

**Given** 工作区尚未选择或不可用  
**When** 请求 `GET /health`  
**Then** API 应返回可解析的健康状态，说明 `workspace_ready` 为 false 或等价状态  
**And** 不应因为缺少工作区而让 health endpoint 崩溃

**Given** active index 已可用、未构建、正在重建或失败  
**When** 请求 `GET /health`  
**Then** response 应包含当前索引状态  
**And** 状态应来自 shared service/SQLite `index_versions`，不得由 route handler 直接读文件夹判断

**Given** API 服务运行但某个内部 service 出错  
**When** health endpoint 无法读取完整状态  
**Then** API 应返回结构化错误或 degraded 状态  
**And** 错误对象应包含 correlation_id，且不得包含 token、API key 或未脱敏内部诊断

**Given** 开发者编写 API contract 测试  
**When** 测试 `GET /health` 在有/无工作区、有/无索引状态下的响应  
**Then** 每种响应都应符合统一 DTO 结构  
**And** 测试不得需要真实外部网络或模型服务

### Story 5.4: GET /search、GET /pages/{page_id} 与 GET /documents/{document_id} 读取接口

**FRs implemented:** FR11

As a 本地自动化用户,  
I want 通过 HTTP API 查询搜索结果、单页 JSON 和文档元数据,  
So that 我可以把 slicer 的页面理解结果接入其他本地工具链。

**Acceptance Criteria:**

**Given** active index 可用且 API 已启用  
**When** 本机工具请求 `GET /search?q={query}&limit={n}`  
**Then** API 应调用 shared `SearchService` 返回搜索结果  
**And** 每条结果应包含 score、`page_id`、图片地址/引用、页面 JSON 摘要、来源文档、页码和相关元数据

**Given** 查询为空、limit 非法或索引不可用  
**When** 请求 `GET /search`  
**Then** API 应返回结构化验证错误、空结果或索引不可用错误  
**And** 响应格式必须符合统一 `{ "data": ... }` / `{ "error": ... }` contract

**Given** 工作区中存在指定 `page_id`  
**When** 本机工具请求 `GET /pages/{page_id}`  
**Then** API 应返回完整页面 JSON、图片地址/引用、来源文档和页码  
**And** `page_id` 必须按页面 occurrence identity 查询，不得被当成 `image_hash`

**Given** 请求的页面不存在、图片缺失或页面分析结果不可用  
**When** 请求 `GET /pages/{page_id}`  
**Then** API 应返回明确结构化错误或带可诊断状态的响应  
**And** 不得暴露未脱敏内部路径以外的 secret 或完整原始模型响应

**Given** 工作区中存在指定 `document_id`  
**When** 本机工具请求 `GET /documents/{document_id}`  
**Then** API 应返回文档元数据、页面数量、转换状态、分析状态和页面列表  
**And** 页面列表应能追溯到对应 `page_id`、页码、页面状态和可用图片/JSON 状态

**Given** route handler 实现这些读取接口  
**When** 开发者检查代码  
**Then** handlers 应调用 shared services，例如 SearchService、Page/Document service 或等价服务  
**And** handlers 不得直接读取 SQLite、JSONL、Tantivy index 或页面图片目录

### Story 5.5: POST /indexes/rebuild Token 保护与后台重建任务

**FRs implemented:** FR11

As a 本地自动化用户,  
I want 通过受保护的 HTTP endpoint 触发索引重建,  
So that 本机自动化工具可以刷新搜索索引，同时避免未授权写操作或重任务被随意触发。

**Acceptance Criteria:**

**Given** localhost API 已启用  
**When** 应用初始化 API token  
**Then** 系统应生成或加载本地 token，并通过安全存储或受控本地配置保存  
**And** token 不得出现在普通日志、错误响应、搜索响应、页面 JSON 或前端持久状态中

**Given** 用户在设置页查看 API token 状态  
**When** token 已存在  
**Then** 设置页应显示 token 已配置/可重置状态  
**And** 不得默认回显完整 token，除非产品明确提供一次性显示或复制机制并避免日志记录

**Given** 本机工具请求 `POST /indexes/rebuild`  
**When** 请求缺少 token 或 token 无效  
**Then** API 应拒绝请求并返回统一错误响应  
**And** 不得创建索引重建任务

**Given** 本机工具使用有效 token 请求 `POST /indexes/rebuild`  
**When** endpoint 校验通过  
**Then** API 应调用 shared IndexService/Job Orchestrator 创建索引重建任务  
**And** 响应应返回任务 ID、初始任务状态和可查询的状态字段

**Given** 索引重建任务正在运行或已有重建任务排队  
**When** 再次请求 `POST /indexes/rebuild`  
**Then** API 应按 service 规则返回已有任务、拒绝重复任务或排队新任务  
**And** 行为应与 GUI 触发重建保持一致，不得形成独立 HTTP-only 重建逻辑

**Given** token reset action 被用户触发  
**When** 系统重置 API token  
**Then** 旧 token 应失效，新 token 状态应保存  
**And** 重置过程和日志不得暴露完整 token

### Story 5.6: API 合同测试、Settings 可见性与外部访问收口

**FRs implemented:** FR11

As a 本地自动化用户,  
I want slicer 的 localhost API 端点经过合同测试，并在设置页清楚展示可用状态,  
So that 我可以稳定地从本机工具调用它，并理解哪些接口需要 token。

**Acceptance Criteria:**

**Given** Epic 5 的 endpoints 已实现  
**When** 开发者运行 API contract tests  
**Then** 测试应覆盖 `GET /health`、`GET /search`、`GET /pages/{page_id}`、`GET /documents/{document_id}` 和 `POST /indexes/rebuild`  
**And** 成功响应、错误响应、认证失败、工作区缺失、索引不可用等场景都应符合统一 DTO contract

**Given** API 默认配置  
**When** 应用首次启用 localhost API  
**Then** 服务默认只监听 `127.0.0.1`  
**And** 测试或启动校验应防止误绑定 `0.0.0.0`

**Given** 写操作或重任务 endpoint 存在  
**When** 外部工具请求这些 endpoint  
**Then** `POST /indexes/rebuild` 必须要求本地 token  
**And** 读取 endpoint 是否要求 token 应遵守当前产品设置，但实现应保留可扩展认证机制

**Given** 用户打开设置页  
**When** 查看 Localhost API 区域  
**Then** 应显示 API 启用/禁用、监听地址、端口、token 状态、token reset action 和基本 endpoint 摘要  
**And** 页面不得显示完整 token、模型 API key 或其他 secret

**Given** 用户或自动化工具读取 API 文档或设置说明  
**When** 查看端点列表  
**Then** 应能看到本机访问示例、哪些端点只读、哪些端点需要 token  
**And** 示例不得包含真实 token 或真实私密路径

**Given** GUI 与 HTTP API 调用同一能力  
**When** 比较搜索、页面、文档和索引重建行为  
**Then** GUI 和 API 应共享 Rust application service layer  
**And** 不得出现 route handler 内复制业务逻辑、绕过 job orchestrator 或绕过 `SearchProvider` 的实现

**Given** Epic 5 完成  
**When** 用户从外部本地工具调用 slicer  
**Then** 可以检查健康状态、执行搜索、读取页面 JSON、读取文档元数据，并用 token 触发索引重建  
**And** 整个 API 不应暴露公网监听、secret、未脱敏诊断或与 GUI 不一致的业务结果
