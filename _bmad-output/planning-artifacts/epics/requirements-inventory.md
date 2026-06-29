# Requirements Inventory

### Functional Requirements

FR1 (FR-001): 用户必须能够选择一个本地目录作为工作目录；应用必须在该目录下创建和维护标准子目录、数据库和索引文件，并在更改工作目录后重新加载该目录中的数据库、任务状态和索引状态。

FR2 (FR-002): 用户必须能够在 `媒体导入` 中通过拖拽或文件选择一次导入一个或多个受支持媒体/文档文件，包括 PNG、JPG/JPEG、WEBP、PDF、PPT、PPTX、DOC、DOCX；系统必须计算原文件哈希、识别重复文件、拒绝不支持的文件类型并展示原因，且将原始文件复制或登记到 `originals/` 并关联 `document_id` 或等价媒体记录。

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

FR13: 应用第一屏必须是工作台而非营销页或介绍页，左侧 sidebar/主导航必须清晰包含工作台、媒体导入、媒体管理、模型分析、一键导出、BM25 索引、搜索和设置等主要功能入口。

FR14: 工作台必须展示当前工作目录、媒体总量、页面总量、待处理数量、可搜索状态、失败摘要、最近任务和主要功能快捷入口；导入、管理、重分析、JSON 编辑、删除、索引重建、搜索和导出等具体操作必须跳转至对应功能选项卡执行。

FR15: 搜索页必须包含搜索输入框、搜索结果列表、结果项标题/摘要/来源文档/页码/相关分数、图片预览区、页面 JSON 查看区、无结果空状态，以及索引不可用或正在重建时的状态提示。

FR16: 设置页必须包含工作目录路径、LibreOffice 可执行文件路径、自动检测 LibreOffice 按钮、模型 provider 名称、API key、base URL、自定义 endpoint、model name、默认图片 DPI、转换并发数、分析并发数，以及启用云端模型时的隐私提示。

FR17: 页面 JSON schema 必须使用 `page_analysis_v1`，包含 `page_id`、`image_hash`、`image_path`、source 信息、analysis 信息、retrieval 文本、model 信息和 `schema_version`。

FR18: 工作目录结构必须至少包含 `originals/`、`pages/<document_id>/<image_hash>.png`、`metadata/pages.jsonl`、`indexes/bm25/` 和 `app.db`。

FR19: 文档状态至少必须支持 `imported`、`converting`、`converted`、`conversion_failed`、`analyzing`、`analyzed`、`analysis_failed`、`indexed` 和 `index_failed`。

FR20: 页面状态至少必须支持 `image_created`、`analysis_pending`、`analysis_running`、`analysis_succeeded`、`analysis_failed` 和 `indexed`。

FR21: 错误记录必须至少包含错误类型、错误摘要、发生阶段、关联文档或页面、是否可重试，以及最近一次发生时间。

FR22: 用户必须能够对单个页面输入纠错提示词，并让多模态模型基于原页面图片、当前页面 JSON 和用户提示词重新生成符合 `page_analysis_v1` schema 的页面 JSON；重新生成成功后，新 JSON 应替换该页面当前有效分析结果，重新生成失败时旧 JSON 必须保持可用，失败原因必须可见且可重试。

FR23: 页面 JSON 查看区必须支持直接编辑 JSON，并提供全屏编辑模式；用户保存前，系统必须校验 JSON 语法和 `page_analysis_v1` schema，校验失败不得写入，校验成功后必须更新 SQLite 权威账本、页面 JSON/JSONL artifact，并触发对应搜索索引更新或标记索引需要刷新。

FR24: 左侧 sidebar 中原有 `图片导入` 导航项必须更名为 `媒体导入`，相关页面标题、空状态、按钮文案、路由命名和用户可见操作文案应保持一致，避免同一能力在界面中同时出现“图片导入”和“媒体导入”两套名称。

FR25: 系统必须新增 `媒体管理` 功能选项卡，并将当前工作台中的文档/媒体管理列表、搜索筛选、状态展示、页面详情、源文件定位、删除和重分析入口迁移到 `媒体管理` 中；迁移后工作台不得继续承载这些具体管理操作。

FR26: 工作台必须转为展示与功能分流视图，只展示工作区状态、媒体数量、待处理数量、可搜索状态、失败摘要、最近任务和快捷入口；导入、媒体管理、模型分析、索引、搜索、导出等具体操作必须跳转到对应功能选项卡完成。

FR27: 用户必须能够在 `媒体管理` 中选择单个图片/媒体项、单个文档或批量媒体项并点击 `重分析`；系统必须带着选中上下文跳转到原 `模型分析` 模块，并在模型分析模块中展示待重分析对象、数量、来源和当前分析状态。

FR28: `模型分析` 模块必须支持对从 `媒体管理` 带入的单个或批量对象执行重新分析；用户可以添加自定义提示词发起大模型重新分析，也可以进入 JSON 编辑/微调流程直接修改当前有效 `page_analysis_v1` JSON，所有保存仍必须通过 schema 校验、原子写入和索引更新/标记刷新流程。

FR29: 选项卡之间的跳转必须保留操作上下文；从工作台或媒体管理跳转到媒体导入、模型分析、搜索、BM25 索引或一键导出时，目标模块应能接收来源、选择项、过滤条件或建议操作，并提供清晰的返回路径。

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

NFR26: 提示词重生成和手动 JSON 编辑都必须是原子更新；失败不得破坏上一版有效 JSON、页面图片、SQLite 记录或当前可用搜索结果。

NFR27: 手动编辑或提示词重生成后的 JSON 不得包含 API key、token、完整原始模型响应或未脱敏错误详情；用户纠错提示词如需记录，只能作为可审计的非敏感元数据保存。

NFR28: 全屏 JSON 编辑器必须能处理常见 `page_analysis_v1` JSON 长度，编辑、格式化、校验和保存时不得让 GUI 卡死，并且错误定位要清晰。

NFR29: 导航命名和信息架构必须一致、可扫描；`媒体导入`、`媒体管理`、`模型分析`、`工作台` 等 tab 的职责边界必须清楚，不得让用户在多个入口中看到重复或冲突的同一操作。

NFR30: 工作台作为分流视图时不得复制业务逻辑；所有快捷入口只能携带上下文跳转到对应 feature，具体导入、管理、分析、索引、搜索和导出逻辑必须继续由对应 feature 和 shared service layer 承担。

NFR31: 批量重分析必须保持 GUI 不阻塞，并继承持久化 Job Orchestrator 的可失败、可记录、可恢复和可重试特性；批量对象数量较大时，界面必须清楚展示选择数量、处理中数量、成功数量、失败数量和可恢复入口。

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
- 用户纠错提示词重新生成 JSON 必须继续通过 `AnalysisService`、`ModelProvider`、`PromptTemplate`、`SchemaValidator`、`AnalysisRepository` 和持久化 Job Orchestrator 执行；前端不得直接调用模型或写入分析结果。
- 手动 JSON 编辑必须通过 service 层执行 JSON 语法校验、`page_analysis_v1` schema 校验、敏感信息检查和原子保存；前端不得直接写 SQLite、JSONL 或索引文件。
- 分析结果应记录来源类型或等价审计字段，例如 `model_generated`、`prompt_regenerated`、`manual_edit`，并明确当前有效结果指针，避免旧失败结果覆盖新有效结果。
- JSON 修正保存成功后必须更新 `metadata/pages.jsonl` 或等价 artifact，并对该页搜索索引执行局部更新；如果 MVP 暂不支持局部索引更新，则必须将索引标记为 stale/needs_rebuild 并在搜索页提示用户刷新。
- 搜索页 JSON 查看区应提供“纠错提示词重生成”和“全屏编辑 JSON”入口，但保存和重生成逻辑必须复用 Epic 3 的服务边界。
- 前端信息架构必须新增 route-level feature：`features/mediaImport`、`features/mediaManagement` 和 `features/modelAnalysis`，并保留 `features/workbench` 作为展示与分流视图；如沿用既有文件夹命名，必须在页面层和路由层清楚表达这些职责边界。
- `媒体管理` 必须通过 service/command 查询 SQLite 权威账本中的 documents、page_records、analysis_results、jobs 和 errors，不得直接扫描 workspace 文件系统作为主数据来源。
- 从 `媒体管理` 发起的单个或批量 `重分析` 必须创建模型分析上下文，并复用 `AnalysisService`、持久化 Job Orchestrator、schema validator 和统一分析结果保存管线；不得在媒体管理页面直接调用模型或直接写 JSON。
- 工作台快捷入口必须只做上下文路由，例如跳转到 `媒体导入`、`媒体管理`、`模型分析`、`BM25 索引`、`搜索` 或 `一键导出`，不得在工作台实现删除、重分析、JSON 保存、索引重建等具体业务操作。
- 本地 API 端点至少包括 `GET /health`、`GET /search?q={query}&limit={n}`、`GET /pages/{page_id}`、`GET /documents/{document_id}` 和 `POST /indexes/rebuild`。
- 实现顺序应优先完成：Tauri React TypeScript scaffold、Rust module boundaries/AppState、SQLite migrations/repositories、workspace initialization/reconciliation、atomic artifact store、persistent job orchestrator、import/conversion provider boundary、model provider/schema validation/analysis service、Tantivy BM25 with Chinese tokenizer、shared GUI/API services、React workbench/search/settings。
- 尚需在实现阶段进一步定稿：PDF renderer 具体库、Tantivy 中文 analyzer 策略、`page_id` 生成算法、API token 默认启用策略、中间 PDF 保留策略。

### UX Design Requirements

UX-DR1: 主导航/侧边栏必须明确呈现 `工作台`、`媒体导入`、`媒体管理`、`模型分析`、`一键导出`、`BM25 索引`、`搜索`、`设置` 等功能入口；`图片导入` 必须统一更名为 `媒体导入`，当前选中态、hover/focus 态和可访问名称保持一致。

UX-DR2: 工作台必须采用 dashboard/overview 体验，优先展示工作区可用性、媒体总量、页面总量、可搜索状态、失败摘要和最近任务；工作台中的按钮应作为清晰跳转入口，而不是直接执行复杂操作。

UX-DR3: `媒体管理` 必须承载文档/图片/页面资产列表，支持按名称、路径、类型、状态搜索或筛选，展示缩略图、来源、页数、分析/索引状态和最近更新时间，并提供查看页面、源文件、删除、重分析等上下文操作。

UX-DR4: 单个或批量重分析交互必须从 `媒体管理` 清楚进入 `模型分析`：选中对象数量、来源范围、当前 JSON 状态、失败项数量和可执行动作必须在跳转后仍可见，避免用户不确定即将重分析哪些内容。

UX-DR5: `模型分析` 中的重新分析流程必须同时支持自定义提示词和 JSON 微调入口；自定义提示词适合重新调用模型，JSON 编辑适合人工微调，两者在界面上必须区分清楚，并共享校验、保存和失败恢复反馈。

UX-DR6: 工作台、媒体管理和模型分析之间的跳转必须保留上下文和返回路径；用户从工作台跳到媒体管理或模型分析后，应能返回原先的筛选、滚动位置或摘要上下文。

UX-DR7: 媒体管理列表和批量操作必须在桌面宽屏与窄窗口下保持可扫描；缩略图、状态 badge、按钮、长文件名和长路径必须有稳定尺寸与截断/tooltip 方案，不得撑破布局。

UX-DR8: 所有 `重分析`、`编辑 JSON`、`删除`、`源文件`、`查看页面` 等 icon-only 或短文本操作必须具备 tooltip、aria-label 和键盘可访问路径；批量重分析状态更新不得高频刷屏，应只在阶段变化、失败和完成时进行可访问反馈。

### FR Coverage Map

FR1: Epic 1 - 工作区选择、更改和加载。

FR2: Epic 2 - 媒体/文档导入、重复识别、类型拒绝和 originals 登记。

FR3: Epic 2 - PDF/Office 转换与逐页 PNG 渲染。

FR4: Epic 2 - 页面图片内容哈希命名与冲突保护。

FR5: Epic 3 - 模型 provider、密钥、endpoint 和 model name 配置。

FR6: Epic 3 - 页面图片多模态分析、schema 校验、失败和重试。

FR7: Epic 2 - SQLite 与 JSONL 页面、任务、文档记录一致性。

FR8: Epic 4 - 基于页面分析构建中文可检索 BM25 索引。

FR9: Epic 4 - 搜索返回页面 JSON、图片地址、分数和来源信息。

FR10: Epic 4 - 全量重建索引、进度、失败保护和状态展示。

FR11: Epic 5 - localhost HTTP API 端点集合。

FR12: Epic 4 - SearchProvider/adapter 抽象与未来检索扩展。

FR13: Epic 1 - 第一屏工作台和完整 sidebar 功能导航。

FR14: Epic 1 - 工作台状态展示、摘要与功能跳转。

FR15: Epic 4 - 搜索页输入、结果列表、预览、JSON、空状态和索引状态。

FR16: Epic 1 - 设置页字段、LibreOffice、模型、API、并发和隐私提示入口。

FR17: Epic 3 - `page_analysis_v1` 页面 JSON schema。

FR18: Epic 1 - 标准工作目录结构。

FR19: Epic 1 - 文档状态枚举。

FR20: Epic 1 - 页面状态枚举。

FR21: Epic 1 - 结构化错误记录。

FR22: Epic 3 - 基于用户纠错提示词重新生成单页可信 JSON。

FR23: Epic 3 - 全屏 JSON 编辑、schema 校验、原子保存与索引刷新提示。

FR24: Epic 1 - `图片导入` 统一更名为 `媒体导入`。

FR25: Epic 2 - 新增 `媒体管理` 并迁移原工作台文档/媒体管理模块。

FR26: Epic 1 - 工作台只做展示和功能分流。

FR27: Epic 3 - 从媒体管理单个/批量选择并跳转模型分析重分析。

FR28: Epic 3 - 模型分析支持自定义提示词重分析和 JSON 微调。

FR29: Epic 1 - 选项卡跳转保留上下文和返回路径。
