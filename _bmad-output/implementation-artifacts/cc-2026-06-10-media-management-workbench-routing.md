---
title: 'Change Story CC-2026-06-10: 媒体管理与工作台路由调整'
type: 'correct-course-story'
created: '2026-06-11'
status: 'review'
source_change_request: 'D:/AIProject/slicer/_bmad-output/planning-artifacts/change-request-media-management-2026-06-10.md'
source_proposal: 'D:/AIProject/slicer/_bmad-output/planning-artifacts/sprint-change-proposal-2026-06-11.md'
story_key: 'cc-2026-06-10-media-management-workbench-routing'
scope: 'Correct Course / Moderate Change'
impacted_epics:
  - 'Epic 1: 工作区、导航与工作台分流体验'
  - 'Epic 2: 媒体导入与媒体资产管理'
  - 'Epic 3: 模型分析、重分析与可信 JSON 修正'
  - 'Epic 4: BM25 索引与页面级搜索体验'
primary_frs:
  - 'FR-013 导航与命名一致性'
  - 'FR-014 工作台概览与功能分流'
  - 'FR-015 媒体管理'
  - 'FR-016 重分析上下文路由'
  - 'FR-017 自定义提示词重分析'
  - 'FR-018 JSON 编辑/微调'
---

# Change Story CC-2026-06-10: 媒体管理与工作台路由调整

**Status:** review

## Story

As a 本地媒体处理用户,  
I want `图片导入` 被统一更名为 `媒体导入`，并新增 `媒体管理` tab，把工作台中的具体媒体管理操作迁移出去,  
So that 工作台只负责状态概览与功能分流，而导入、管理、重分析和 JSON 微调都发生在职责清晰的功能模块中。

## Acceptance Criteria

1. **导航命名与 tab 完整性**  
   Given 用户查看左侧 sidebar 或页面标题, when 应用渲染主导航和功能页面, then 应显示 `媒体导入`，不得再显示旧的 `图片导入` 文案, and 主导航应包含 `媒体管理`。

2. **工作台职责收缩**  
   Given 用户进入 `工作台`, when 工作区已选择并存在媒体、任务或失败状态, then 工作台只展示工作区状态、媒体/页面/失败/索引摘要、最近任务摘要和快捷入口, and 工作台不得直接承载完整导入 dropzone、媒体/文档管理列表、删除、模型调用、JSON 保存、索引重建、搜索执行或导出执行。

3. **媒体导入归属**  
   Given 用户进入 `媒体导入`, when 用户拖拽或选择文件, then 页面应支持图片和文档媒体的导入入口、类型预检、导入提交和导入反馈, and 不得在 `媒体导入` 中承载删除、源文件定位、JSON 编辑或重分析选择。

4. **媒体管理归属**  
   Given 用户进入 `媒体管理`, when 工作区存在已导入媒体、文档或页面资产, then 页面应展示列表、搜索筛选、详情、状态、缩略图、源文件定位、删除和重分析选择入口, and 数据必须来自后端 service/SQLite 权威账本，不得直接扫描 workspace 文件系统作为主数据源。

5. **重分析路由上下文**  
   Given 用户在 `媒体管理` 中选择单个媒体、单个文档、单页或批量对象, when 用户点击 `重分析`, then 应跳转到 `模型分析` tab, and navigation context 必须包含 `source_tab`、`return_to`、`action = reanalyze`、选择对象类型、ID 列表、来源筛选和选择数量, and `媒体管理` 不得直接调用模型 provider、创建模型请求或保存 JSON。

6. **模型分析接收上下文**  
   Given `模型分析` 接收到重分析上下文, when 页面展示可执行动作, then 应展示选择对象摘要、当前 JSON 状态、预计重分析页数，并提供默认重分析、自定义提示词重分析和 JSON 编辑/微调入口。

7. **返回与刷新**  
   Given 用户完成、取消或关闭重分析/JSON 编辑流程, when 用户返回来源页面, then 应返回 `媒体管理` 中原先的筛选、选中范围或列表上下文, and 最新 JSON 状态应通过后端查询刷新，而不是复用过期前端缓存。

8. **实现边界可审计**  
   Given 开发者检查实现, when 查看 route/tab state、feature 组件和 typed client, then 工作台、媒体导入、媒体管理、模型分析的职责边界必须清晰, and 具体业务逻辑必须通过对应 service/client 调用，不得塞入工作台或路由层。

## Tasks / Subtasks

- [x] 导航、页面标题与 typed navigation context
  - [x] 在 `src/app/navigation.ts` 中将用户可见 `图片导入` 改为 `媒体导入`，新增 `mediaManagement` 视图；推荐把旧 `imageImport` view id 迁移为 `mediaImport`，如保留旧 id 必须只作为兼容别名，不得继续暴露旧文案。（AC: 1）
  - [x] 在 `src/app/AppShell.tsx` 注册 `媒体管理` 页面，并让 `pageTitles`、sidebar、active view state 使用同一份 `ViewId` 契约。（AC: 1）
  - [x] 定义 typed navigation context，例如 `NavigationContext` / `ReanalysisNavigationContext`，字段至少覆盖 `source_tab`、`return_to`、`action`、`selected_kind`、`selected_ids`、`filter`、`query`、`scroll_anchor`、`selection_count`。（AC: 5, 7, 8）
  - [x] AppShell 负责保存和传递路由/恢复上下文，不负责导入、删除、模型调用、JSON 保存、索引重建或搜索执行。（AC: 8）

- [x] 工作台改为 overview 与 routing surface
  - [x] 收缩 `src/features/workbench/WorkbenchPage.tsx`：移除完整导入 dropzone、文档管理列表、删除、源文件定位、单页分析、文档重分析、失败页重分析、一键导出执行和索引重建面板。（AC: 2）
  - [x] 工作台保留工作区状态、媒体/页面/失败/索引摘要、最近任务摘要和跳转入口；跳转入口只调用 AppShell/context 回调切换 tab，不直接打开文件选择器或执行业务命令。（AC: 2, 8）
  - [x] 工作台摘要数据仍通过 `tauriClient.listDocuments()`、`tauriClient.listWorkbenchPages()`、`tauriClient.listJobs()`、`tauriClient.getIndexStatus()` 等后端/service-backed client 获取，不直接扫描文件系统。（AC: 2, 8）
  - [x] 移除或迁走 `JobList` 中的“创建示例任务”入口；如保留任务摘要，必须显示真实任务状态，不再使用 placeholder 任务作为产品入口。（AC: 2）

- [x] 媒体导入页面统一为图片/文档媒体导入
  - [x] 将 `src/features/image-import/ImageImportPage.tsx` 迁移或重命名为 `src/features/media-import/MediaImportPage.tsx`；如短期保留旧目录，应提供清晰迁移说明并确保 UI 文案全部为 `媒体导入`。（AC: 1, 3）
  - [x] 复用 `src/lib/fileValidation.ts` 的 `SUPPORTED_EXTENSIONS`、`isSupportedFileType`、`getUnsupportedReason`，以及 `tauriClient.importFile()` / `importMultipleFiles()`，避免再维护单独的“图片导入”和“文档导入”分支。（AC: 3, 8）
  - [x] 扩展当前只支持图片的 `openImageImportDialog()` 使用路径：媒体导入应支持 PNG、JPG/JPEG、PDF、PPT、PPTX、DOC、DOCX；可新增 `openMediaImportDialog()` 或扩展 `openImportDialog()`，但必须与 `fileValidation` 支持类型一致。（AC: 3）
  - [x] 媒体导入只显示导入入口、类型预检、导入中/成功/重复/不支持/失败反馈和刷新入口；不得展示删除、源文件定位、重分析、JSON 编辑或文档管理列表。（AC: 3）
  - [x] 拖拽导入和按钮选择必须复用同一导入校验与导入提交逻辑，保持并发锁、逐文件结果和路径缺失错误反馈。（AC: 3）

- [x] 新增媒体管理 feature
  - [x] 新增 `src/features/media-management/MediaManagementPage.tsx`，集中承载媒体/文档/页面列表、搜索筛选、详情、状态、缩略图、源文件定位、删除和重分析选择入口。（AC: 4）
  - [x] 从 `src/features/workbench/components/DocumentList.tsx` 迁移/抽取可复用展示组件到媒体管理所属目录或共享目录；避免让媒体管理继续依赖 `features/workbench` 的内部组件边界。（AC: 4, 8）
  - [x] 列表数据使用 `tauriClient.listDocuments()` 和 `tauriClient.listWorkbenchPages(documentId)`；缩略图/页面预览使用现有 `PageWorkbenchDto.image_path` 或 `getPageImagePreview(pageId)` 模式，不扫描 workspace 目录自行推导。（AC: 4）
  - [x] 支持按文件名、类型、导入/转换/分析状态、失败状态和来源上下文过滤；从工作台或搜索跳入时应读取 context 中的筛选条件。（AC: 4, 7）
  - [x] 删除和源文件定位继续通过 typed client/service 执行：`tauriClient.deleteDocument(documentId)`、`tauriClient.revealDocumentInFolder(path)`；删除前必须有确认流程，并在完成后刷新后端数据。（AC: 4, 7, 8）

- [x] 媒体管理到模型分析的重分析上下文
  - [x] 媒体管理中支持选择单个文档、单页或批量对象；点击 `重分析` 时只构建 typed navigation context 并跳转 `模型分析`，不直接调用 `reanalyzeDocument`、`reanalyzeFailedPages` 或任何模型 provider。（AC: 5）
  - [x] 对没有页面图片、已删除、状态不完整、没有可分析页的对象显示逐项不可重分析原因，不把无效对象带入上下文。（AC: 5）
  - [x] context 中只放路由和恢复数据：对象类型、ID 列表、来源 tab、return target、筛选、滚动锚点、选择数量；目标页必须再查后端获取当前对象状态和 JSON 状态。（AC: 5, 7）
  - [x] 批量选择时清楚展示选择数量和对象范围，避免让用户误以为会立刻开始模型调用。（AC: 5）

- [x] 模型分析页接收上下文并展示后续入口
  - [x] 扩展 `src/features/analysis/AnalysisPage.tsx` props，使其接收 typed navigation context 和返回回调；直接进入页面时仍显示模型配置状态、待分析页数、失败页数和默认分析入口。（AC: 6）
  - [x] 当 context.action 为 `reanalyze` 时，通过 service/client 查询选中对象当前状态，展示 `ReanalysisContextSummary`：对象类型、选择数量、来源范围、待分析页数、已有 JSON 数量、失败项数量、来源 tab。（AC: 6）
  - [x] 默认重分析入口可复用现有 `tauriClient.reanalyzeDocument()` / `reanalyzeFailedPages()` 能力；自定义提示词重分析和 JSON 编辑/微调如果后端能力尚未完整，应以 disabled/coming-ready 状态或最小入口呈现，不能伪造保存成功。（AC: 6）
  - [x] 如果模型配置缺失，模型调用类动作必须 disabled 或跳转设置；已有有效 JSON 的查看/编辑入口不应被模型配置完全阻断。（AC: 6）
  - [x] 完成、取消或返回时调用 AppShell/context 回调回到 `媒体管理`，并触发媒体管理重新查询后端数据，不复用旧 React 内存快照。（AC: 7）

- [x] 搜索/JSON 入口边界
  - [x] `src/features/search/SearchPage.tsx` 如增加 `重分析` 或 `编辑 JSON` 入口，必须跳转 `模型分析` 并携带 typed context；搜索页不得直接调用模型、写 SQLite、写 JSONL 或更新索引。（AC: 5, 8）
  - [x] 搜索页现有 `page_json` 查看保持只读；如果加入编辑入口，应进入模型分析/JSON 微调流程并复用可信保存管线。（AC: 6, 8）

- [x] 样式、可访问性和响应式
  - [x] 在 `src/styles/globals.css` 中为工作台 overview、媒体导入、媒体管理和重分析摘要补充稳定布局；不要把 page section 做成卡片套卡片，不要使用营销 hero。（AC: 2, 3, 4, 6）
  - [x] 长文件名、长路径、多个状态 badge、批量选择摘要和按钮在窄窗口下不得重叠或撑破容器；缩略图和操作按钮使用稳定尺寸。（AC: 4, 6）
  - [x] `查看页面`、`源文件`、`删除`、`重分析`、`返回` 等操作必须有清晰可访问名称、焦点态和键盘触发路径；危险删除必须确认。（AC: 4, 5）

- [x] 测试与验证
  - [x] 运行 `npm run build`，确保 TypeScript 与 Vite 构建通过。（AC: 1-8）
  - [x] 至少补充可执行的前端单元/轻量集成测试，或在当前无测试框架时记录手动测试脚本：导航文案无 `图片导入`、存在 `媒体管理`、工作台无 dropzone/DocumentList/删除/分析执行入口、媒体导入不显示管理操作、媒体管理重分析只构建 context、模型分析能接收 context。（AC: 1-8）
  - [x] 在 Tauri 壳或可用浏览器预览中做视觉检查：桌面宽度与窄窗口下文字不重叠，按钮不溢出，状态可读。（AC: 2-6）
  - [x] 如改动 Rust service/DTO，运行相关 `cargo test`；本故事预期主要为前端重分区，非必要不改数据库 schema 或核心后端服务。（AC: 8）

## Dev Notes

### Authoritative Context And Overrides

- 本 story 来自已批准 Correct Course：`D:/AIProject/slicer/_bmad-output/planning-artifacts/sprint-change-proposal-2026-06-11.md`，批准时间为 2026-06-11，状态为 `approved`。
- `D:/AIProject/slicer/_bmad-output/planning-artifacts/epics/epic-list.md` 已追加 `Change Story CC-2026-06-10: 媒体管理与工作台路由调整`，AC 明确要求 `媒体导入`、`媒体管理`、工作台职责收缩、重分析上下文路由和模型分析接收上下文。
- 旧 `prd.md`、`architecture.md`、`ux-design-specification.md`、`docs/*` 中仍有“工作台负责导入/转换/分析/任务/管理”的描述。对本 story 来说，这是已识别的 artifact conflict；实现时以 `sprint-change-proposal-2026-06-11.md` 和 sharded `epics/epic-list.md` 的新边界为准。
- 不要把已完成 Epic 1-5 的 sprint 历史改回 backlog。本 story 是独立 correct-course 执行项，key 为 `cc-2026-06-10-media-management-workbench-routing`。

### Current Implementation State

- `src/app/navigation.ts` 当前 `ViewId` 包含 `imageImport`，用户可见 label 是 `图片导入`，没有 `mediaManagement`。
- `src/app/AppShell.tsx` 当前只注册 `WorkbenchPage`、`ImageImportPage`、`AnalysisPage`、`ExportPage`、`IndexPage`、`SearchPage`、`SettingsPage`，没有 `媒体管理` view，也没有 typed navigation context。
- `src/features/workbench/WorkbenchPage.tsx` 当前约 1100 行，集中持有 jobs、documents、pages、import、modelStatus、analysis、reanalyze、delete、export 等状态，并直接实现：
  - 文档导入 dialog、拖拽导入、导入并发锁和逐文件结果。
  - `DocumentList` 文档/页面管理。
  - 单页分析、批量分析、文档重分析、失败页重分析。
  - 源文件定位、页面图片定位、删除。
  - 一键导出和索引状态面板。
  这正是本 story 要拆分的职责集中点。
- `src/features/image-import/ImageImportPage.tsx` 当前仍是图片导入页：文案是 `图片导入`，只支持 PNG/JPG/JPEG，并且还展示 `DocumentList`，包含源文件定位和删除，违反本 story 对媒体导入的边界。
- `src/features/analysis/AnalysisPage.tsx` 当前只支持模型配置状态与 `analyzeNewPages()`，未接收媒体管理选择上下文，也没有 `ReanalysisContextSummary`。
- `src/features/search/SearchPage.tsx` 当前搜索和 JSON 查看为只读，索引重建在搜索页内执行。若本 story 添加重分析/编辑入口，只能跳转模型分析，不能在搜索页保存 JSON 或调用模型。
- `src/features/workbench/components/DocumentList.tsx` 当前已经包含大量可迁移能力：文档列表、页面缩略图、源文件、删除、单页分析、失败页重试、文档重分析、分析摘要。迁移时优先抽取到 `media-management` 或共享组件，不要让新 feature 继续依赖 `features/workbench` 内部目录。
- `src/lib/tauriClient.ts` 已有可复用 client：`importFile`、`importMultipleFiles`、`listDocuments`、`listWorkbenchPages`、`deleteDocument`、`revealDocumentInFolder`、`analyzePage`、`analyzeNewPages`、`reanalyzeDocument`、`reanalyzeFailedPages`、`getIndexStatus`、`searchPages`、`getPageImagePreview`、`exportMedia`。
- `src/lib/fileValidation.ts` 已有统一支持类型：文档 PDF/DOC/DOCX/PPT/PPTX，图片 PNG/JPG/JPEG，组合为 `SUPPORTED_EXTENSIONS`。媒体导入应优先复用这些 helper。
- `src/features/workbench/components/JobList.tsx` 当前仍显示 `创建示例任务` placeholder 文案；`deferred-work.md` 已记录这会削弱真实产品感。本 story 收缩工作台时应移除或迁走该入口。

### Architecture Guardrails

- 前端只触发 use case 和渲染状态，不拥有转换、分析、索引、workspace reconciliation、搜索或可信保存逻辑。底层能力继续由 Rust/Tauri commands、services、repositories 和 SQLite 权威账本承载。
- 媒体管理数据必须来自 `list_documents` / `list_workbench_pages` 等 service-backed client。不要直接扫描 workspace 目录、拼路径推断状态，或绕过 service 修改 SQLite。
- 工作台只能是 overview/routing surface。工作台不得直接执行导入、删除、模型调用、JSON 保存、索引重建、搜索或导出。
- 媒体导入只负责接收媒体、预检、提交导入和显示导入反馈。不要在媒体导入页显示完整管理列表、删除、源文件定位、JSON 编辑或重分析选择。
- 媒体管理可以删除、定位源文件、选择重分析对象，但重分析按钮只构建 typed navigation context 并跳转模型分析，不创建模型请求。
- 模型分析是默认分析、重分析、自定义提示词重分析和 JSON 编辑/微调的归属。任何 JSON 保存必须复用 schema 校验、敏感信息检查、原子写入和索引 stale/refresh 管线；如果该后端能力还没实现，前端必须诚实显示不可用或待实现，不得假成功。
- Search 页面保持搜索与只读 JSON 查看边界；如提供编辑/重分析入口，也必须路由到模型分析/可信 JSON 流程。

### Suggested File Map

- Update: `src/app/navigation.ts` -- ViewId、navigationItems、用户可见 label。
- Update: `src/app/AppShell.tsx` -- pageTitles、view registration、cross-tab context state 与回调。
- Update: `src/features/workbench/WorkbenchPage.tsx` -- 删除具体业务操作，改为概览与跳转入口。
- Move/Update: `src/features/image-import/ImageImportPage.tsx` -> `src/features/media-import/MediaImportPage.tsx` -- 统一媒体导入，去除管理操作。
- Add: `src/features/media-management/MediaManagementPage.tsx` -- 列表、筛选、详情、源文件、删除、重分析选择。
- Move/Extract: `src/features/workbench/components/DocumentList.tsx` -- 迁出或抽成共享媒体列表组件。
- Update: `src/features/analysis/AnalysisPage.tsx` -- 接收重分析上下文，展示 `ReanalysisContextSummary`。
- Optional Add: `src/features/analysis/components/ReanalysisContextSummary.tsx`。
- Update: `src/features/search/SearchPage.tsx` -- 如加入口，只能构建 context 并跳转。
- Update: `src/lib/tauriClient.ts` -- 仅在需要统一媒体 dialog 或 typed helper 时扩展，不改既有命令语义。
- Update: `src/types/app.ts` -- typed navigation context 可放这里或 `src/app/navigation.ts`，保持前端类型集中。
- Update: `src/styles/globals.css` -- 新页面/状态/响应式样式。

### Previous Work Intelligence

- 最近提交 `b9f9f94 feat: polish workbench import UI` 强化了工作台导入、资产摘要和文档行视觉，但该方向仍是工作台中心化。本 story 应迁移这些能力，而不是丢弃已做的导入锁、逐文件反馈、缩略图、失败摘要和响应式样式。
- 最近提交 `24457e4 feat: add image import functionality and enhance settings page for OpenAI model management` 新增了 `ImageImportPage`、`imageImport` 导航和 OpenAI model list；本 story 应把 image import 概念提升为 media import，并复用已存在图片导入命令。
- `spec-workbench-ui-polish.md` 已验证 `npm run build`、`git diff --check`、浏览器 sanity check；继续保持这些验证习惯。
- `deferred-work.md` 中记录 `JobList` placeholder 文案、前端 async safety、重复 helper 等问题。拆分时保留 generation counter/cleanup 思路，避免跨 tab 切换后过期请求写入新状态。

### Current Version And Latest-Tech Notes

- 当前项目实际解析版本：React 19.2.6、React DOM 19.2.6、Vite 7.3.3、TypeScript 5.8.3、`@tauri-apps/api` 2.11.0、`@tauri-apps/plugin-dialog` 2.7.1、Tauri Rust crate 2.11.2、sqlx 0.8.6、tantivy 0.22.1。
- 2026-06-11 查询到的外部最新版本包括 React 19.2.7、Vite 8.0.16、TypeScript 6.0.3、Tauri crate 2.11.2、sqlx 0.9.0、axum 0.8.9。该信息只用于避免过时判断；本 story 不要求依赖升级。
- 不要在本 story 顺手升级 React/Vite/TypeScript/Tauri/sqlx/tantivy。依赖升级会引入额外迁移和验证风险，应另开任务。
- Tauri v2 前端命令调用继续使用 `@tauri-apps/api/core` 的 `invoke`，文件选择继续使用 dialog plugin 的 `open`；遵循当前 `tauriClient.ts` 封装，不在组件里散落裸 `invoke`。
- React 状态设计要注意 tab 切换时 state preservation/reset：context 应由 AppShell 持有并作为 typed props 下发，目标页根据 context 重新查询后端，不依赖来源页内存。

### Testing Guidance

- Build: `npm run build`。
- Optional type-only check: `npx tsc --noEmit --pretty false`。
- If Rust touched: run targeted `cargo test` under `src-tauri` before full suite.
- Manual UI checklist:
  - Sidebar 与 topbar 不再出现 `图片导入`，出现 `媒体导入` 与 `媒体管理`。
  - 工作台无完整 dropzone、DocumentList、删除、分析执行、导出执行、索引重建执行。
  - 媒体导入可选择/拖拽图片和文档，显示类型预检与导入结果，不显示删除/重分析/JSON 编辑。
  - 媒体管理可查看列表、筛选、详情、缩略图、源文件、删除、选择重分析。
  - 点击媒体管理 `重分析` 切到模型分析，并展示选择摘要；媒体管理没有直接发起模型调用。
  - 从模型分析返回媒体管理后，列表通过后端刷新并尽量恢复原筛选/选中/滚动上下文。
  - 窄窗口下长文件名、路径、状态 badge、批量摘要和按钮不重叠。

## References

- `D:/AIProject/slicer/_bmad-output/planning-artifacts/change-request-media-management-2026-06-10.md` -- 用户原始变更请求。
- `D:/AIProject/slicer/_bmad-output/planning-artifacts/sprint-change-proposal-2026-06-11.md` -- approved Correct Course proposal、scope、success criteria、risk controls。
- `D:/AIProject/slicer/_bmad-output/planning-artifacts/epics/epic-list.md` -- Change Story CC-2026-06-10、Story 1.5、Story 1.6、Story 2.6、Story 2.7、Story 3.1。
- `D:/AIProject/slicer/_bmad-output/planning-artifacts/implementation-readiness-report-2026-06-10.md` -- 已识别导航/feature boundary 冲突。
- `D:/AIProject/slicer/_bmad-output/planning-artifacts/architecture.md` -- 旧架构约束与前端不得拥有业务逻辑的通用原则；本 story 的 feature split 覆盖旧 workbench ownership 描述。
- `D:/AIProject/slicer/_bmad-output/planning-artifacts/ux-design-specification.md` -- 旧 UX 主旅程参考；本 story 的 media-management journey 覆盖旧 workbench-centric 描述。
- `D:/AIProject/slicer/docs/component-inventory.md`、`D:/AIProject/slicer/docs/source-tree-analysis.md` -- 当前 docs 仍描述旧组件归属，后续需同步。
- `D:/AIProject/slicer/src/app/navigation.ts`
- `D:/AIProject/slicer/src/app/AppShell.tsx`
- `D:/AIProject/slicer/src/features/workbench/WorkbenchPage.tsx`
- `D:/AIProject/slicer/src/features/image-import/ImageImportPage.tsx`
- `D:/AIProject/slicer/src/features/analysis/AnalysisPage.tsx`
- `D:/AIProject/slicer/src/features/search/SearchPage.tsx`
- `D:/AIProject/slicer/src/features/workbench/components/DocumentList.tsx`
- `D:/AIProject/slicer/src/lib/tauriClient.ts`
- `D:/AIProject/slicer/src/lib/fileValidation.ts`
- `D:/AIProject/slicer/src/types/app.ts`

## Completion Status

Ultimate context engine analysis completed - comprehensive developer guide created.  
Ready for `dev-story` implementation.

## Dev Agent Record

### Agent Model Used

GPT-5 Codex

### Debug Log References

- 2026-06-11T16:30:03+08:00: Marked story and sprint status as in-progress and started Correct Course implementation.
- 2026-06-11T17:09:14+08:00: `npm run test:media-boundaries` passed; verified route and feature ownership boundaries.
- 2026-06-11T17:09:14+08:00: `npm run build` passed; TypeScript and Vite build succeeded.
- 2026-06-11T17:09:14+08:00: `git diff --check` passed with no whitespace errors.
- 2026-06-11T17:09:14+08:00: Browser preview sanity check passed; desktop navigation showed media tabs and 390px viewport had no horizontal overflow.

### Completion Notes List

- Migrated navigation from `imageImport` / old image-import wording to `mediaImport` / media-import wording, added `mediaManagement`, and centralized typed navigation context in AppShell.
- Reduced WorkbenchPage to an overview and routing surface that reads service-backed summary data and switches tabs without executing import/delete/model/export/index actions.
- Added MediaImportPage that reuses `SUPPORTED_EXTENSIONS`, `isSupportedFileType`, `getUnsupportedReason`, and `tauriClient.importMultipleFiles()` for both picker and drag/drop imports.
- Added MediaManagementPage and media-owned asset list using `listDocuments()`, `listWorkbenchPages()`, and `getPageImagePreview()` for ledger-backed list, filtering, thumbnails, source reveal, delete confirmation, and reanalysis selection.
- Media management reanalysis now builds `ReanalysisNavigationContext` only; model/provider calls are owned by AnalysisPage.
- AnalysisPage now receives reanalysis context, requeries backend state, displays a reanalysis summary, runs default reanalysis through existing client calls, and shows custom prompt / JSON edit as disabled until trusted backend support exists.
- Added `scripts/verify-media-boundaries.mjs` and `npm run test:media-boundaries` as the executable boundary regression check for this story.

### File List

- `package.json`
- `scripts/verify-media-boundaries.mjs`
- `src/app/navigation.ts`
- `src/app/AppShell.tsx`
- `src/features/analysis/AnalysisPage.tsx`
- `src/features/image-import/ImageImportPage.tsx` (deleted)
- `src/features/media-import/MediaImportPage.tsx`
- `src/features/media-management/MediaManagementPage.tsx`
- `src/features/media-management/components/MediaAssetList.tsx`
- `src/features/workbench/WorkbenchPage.tsx`
- `src/features/workbench/components/JobList.tsx`
- `src/lib/tauriClient.ts`
- `src/styles/globals.css`
- `_bmad-output/implementation-artifacts/cc-2026-06-10-media-management-workbench-routing.md`
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

### Change Log

- 2026-06-11: Implemented media import/management route split, workbench responsibility reduction, reanalysis navigation context, model analysis context receiver, styling, and boundary validation. Status moved to review.
