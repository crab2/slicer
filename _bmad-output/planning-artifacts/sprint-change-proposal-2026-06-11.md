---
source: bmad-correct-course
date: 2026-06-11
project: slicer
mode: incremental
status: approved
scope: moderate
trigger_file: D:/AIProject/slicer/_bmad-output/planning-artifacts/change-request-media-management-2026-06-10.md
---

# Sprint Change Proposal: 媒体管理与工作台路由调整

## 1. Issue Summary

### 触发问题

用户确认本次 Correct Course 的触发点为“媒体管理与工作台路由调整”。已有变更请求明确提出：

1. 将左侧 sidebar 中的 `图片导入` 更名为 `媒体导入`。
2. 新增 `媒体管理` 功能 tab。
3. 将当前工作台中的文档/媒体管理模块迁移到 `媒体管理`。
4. 用户对单个或批量媒体点击 `重分析` 时，应路由到原 `模型分析` 模块。
5. `模型分析` 支持自定义提示词重分析，也支持直接编辑 JSON 做微调。
6. `工作台` 只展示状态并负责路由分流，具体操作应进入对应功能 tab。

### 问题定义

当前产品规划与实现存在职责边界不一致：

- 部分规划文档仍把工作台定义为导入、转换、分析、任务进度和管理操作的集中页面。
- 新版 Epic 与 Requirements Inventory 已经吸收了 `媒体导入`、`媒体管理`、`模型分析` 和工作台分流方向。
- 当前前端实现仍存在 `图片导入` 文案，且没有 `媒体管理` tab。
- 当前 `WorkbenchPage` 仍承载导入、文档列表、删除、源文件定位和重分析等具体操作。

这会导致用户在工作台、导入页、管理页和模型分析页之间看到重复或冲突入口，也会让后续开发继续把业务逻辑塞回工作台。

### 证据

- `src/app/navigation.ts` 当前仍包含 `imageImport` 和用户可见文案 `图片导入`。
- `src/app/AppShell.tsx` 当前没有注册 `媒体管理` view。
- `src/features/workbench/WorkbenchPage.tsx` 当前包含导入、文档列表、删除、分析和重分析相关状态与处理函数。
- `src/features/image-import/ImageImportPage.tsx` 当前仍显示 `图片导入`，且偏向图片导入而非统一媒体导入。
- `prd.md` 的信息架构仍写为工作台、搜索、设置三大入口，并要求工作台包含拖拽区域、转换按钮、分析按钮和重试按钮。
- `architecture.md` 的前端边界仍写为 `features/workbench/` 拥有 import、conversion、analysis、job list、retry 和 index rebuild UI。
- `ux-design-specification.md` 仍把“用户把本地文档拖入工作台”定义为主体验。

## 2. Impact Analysis

### Epic Impact

#### Epic 1: 工作区、导航与工作台分流体验

影响级别：高。

Epic 1 的规划文本已经接近新方向，但当前实现和部分文档未完全对齐。需要将工作台明确收缩为 overview 和 routing surface，并让 sidebar 完整呈现 `工作台`、`媒体导入`、`媒体管理`、`模型分析`、`一键导出`、`BM25 索引`、`搜索`、`设置`。

#### Epic 2: 媒体导入与媒体资产管理

影响级别：高。

需要拆清 `媒体导入` 与 `媒体管理`：

- `媒体导入` 只负责图片/文档媒体接收、预检、导入提交和导入反馈。
- `媒体管理` 负责媒体/文档/页面资产列表、筛选、详情、源文件定位、删除和重分析选择。

当前工作台中的文档/媒体管理能力应迁移到 `媒体管理`。

#### Epic 3: 模型分析、重分析与可信 JSON 修正

影响级别：中到高。

`模型分析` 需要接收从 `媒体管理` 带入的单个或批量重分析上下文，并展示选择对象摘要、当前 JSON 状态、预计重分析页数和可执行动作。自定义提示词重分析和 JSON 编辑/微调应归属到 `模型分析` 或其可信保存流程，不能由 `媒体管理` 或 `工作台` 直接执行。

#### Epic 4: BM25 索引与页面级搜索体验

影响级别：低到中。

搜索页如提供重分析或 JSON 编辑入口，应路由到模型分析/JSON 编辑流程，并复用可信保存管线。搜索页不得直接调用模型、写 SQLite、写 JSONL 或更新索引。

#### Epic 5: Localhost HTTP API 与外部自动化访问

影响级别：低。

本次变更主要影响 GUI 信息架构和前端 feature 边界。除非后续实现发现需要新增只读媒体管理 DTO，否则 HTTP API 可保持当前范围。

### Story Impact

建议新增一个 Correct Course 执行故事，而不是将已完成历史 story 改回未完成：

```md
### Change Story CC-2026-06-10: 媒体管理与工作台路由调整

Scope: Correct Course / Moderate Change
Impacted Epics: Epic 1, Epic 2, Epic 3, Epic 4
Primary FRs: FR-013, FR-014, FR-015, FR-016, FR-017, FR-018
```

该故事用于追踪：

- 导航命名和 tab 增补。
- 工作台职责收缩。
- 媒体导入页面调整。
- 新增媒体管理页面。
- 媒体管理到模型分析的重分析上下文路由。
- 搜索/JSON 入口边界。
- 相关测试与验收。

### Artifact Conflicts

#### PRD

需要更新：

- `5.1 信息架构`
- `5.2 工作台`
- `FR-002 文件导入`
- `FR-006 页面分析`
- 追加或同步导航命名、工作台分流、媒体管理、重分析上下文、自定义提示词重分析、JSON 编辑/微调相关功能需求。

#### Architecture

需要更新：

- 前端导航从 `Workbench / Search / Settings` 扩展为完整 feature tab。
- `features/workbench/` 从业务操作容器改为 overview 与 routing surface。
- 新增 `features/media-import/`、`features/media-management/` 边界。
- 增加 typed navigation context contract。

#### UX Design

需要更新：

- 定义性体验从“拖入工作台”调整为“工作台分流、媒体导入接收、媒体管理维护、模型分析修正”。
- Dropzone 归属从工作台改为 `媒体导入`。
- 增加 `MediaManagementList` 与 `ReanalysisContextSummary`。
- Primary Navigation 更新为完整 tab 列表。

#### Project Docs

需要后续同步：

- `docs/component-inventory.md`
- `docs/source-tree-analysis.md`
- `docs/project-overview.md`
- `docs/architecture.md`
- 其他仍把工作台描述为“导入、任务列表、页面浏览”的文档。

### Technical Impact

前端影响较大，后端影响较小：

- 需要修改 `src/app/navigation.ts` 与 `src/app/AppShell.tsx`。
- 需要新增或迁移 `features/media-management/`。
- 可能需要将 `features/image-import/` 重命名或调整为 `features/media-import/`。
- 需要从 `WorkbenchPage` 移除完整导入、管理、删除、重分析等具体操作。
- 需要定义 typed navigation context。
- 现有 `tauriClient`、SQLite、分析服务、索引服务、JSON 校验和保存管线应尽量复用。

## 3. Recommended Approach

### Selected Path

选择 Option 1：Direct Adjustment。

本次不建议回滚，也不需要缩减 MVP。底层导入、分析、索引、搜索、API 能力仍然有效；需要调整的是产品信息架构、前端职责边界、文档一致性和后续实现追踪。

### Why Not Rollback

回滚会破坏已完成 Epic 的历史记录，并增加实现风险。本次问题不是底层方案失败，而是职责边界和入口归属需要纠偏。

### Why Not MVP Reduction

MVP 核心目标不变：

- 本地工作区。
- 图片/文档媒体导入。
- 页面图片生成。
- 多模态分析生成 `page_analysis_v1` JSON。
- BM25 搜索。
- GUI 查看图片与 JSON。
- localhost HTTP API。

变化只是将操作放到正确 feature tab 中。

### Effort Estimate

努力评估：中等。

主要工作在前端页面拆分、上下文路由、文档同步和测试补齐。Rust 后端核心服务、SQLite、分析服务、索引服务和 HTTP API 大概率可以复用。

### Risk Assessment

风险评估：中等偏低。

主要风险：

- 工作台职责收缩不彻底，仍残留具体业务操作。
- 新增 `媒体管理` 时复制旧工作台逻辑而不是迁移归属。
- `重分析` 上下文丢失或依赖前端全局状态。
- 搜索页、媒体管理、模型分析之间 JSON 状态刷新不一致。

风险控制：

- 在架构文档中明确 feature boundaries。
- 为 cross-tab action 定义 typed navigation context。
- 实现验收中明确禁止工作台直接执行导入、删除、模型调用、JSON 保存、索引重建、搜索或导出。
- 增加 UI 和路由测试。

## 4. Detailed Change Proposals

### 4.1 PRD 信息架构与工作台职责

#### OLD

```md
主导航包含：

1. 工作台：导入、转换、分析、任务进度。
2. 搜索：关键词查询、结果列表、图片预览、JSON 查看。
3. 设置：工作目录、LibreOffice 路径、模型配置、图片参数、并发参数。
```

#### NEW

```md
主导航包含：

1. 工作台：工作区状态、媒体/页面/失败/索引摘要、最近任务摘要和功能跳转入口。
2. 媒体导入：图片和文档媒体的拖拽/选择、类型预检、导入提交和导入反馈。
3. 媒体管理：已导入媒体/文档/页面资产的列表、搜索、筛选、详情、源文件定位、删除和重分析选择。
4. 模型分析：默认分析、单个/批量重分析、自定义提示词重分析、JSON 编辑/微调和分析任务状态。
5. 一键导出：导出已处理媒体、页面图片、JSON 或索引相关 artifact。
6. BM25 索引：索引状态、索引构建/重建、失败恢复和 stale 状态提示。
7. 搜索：关键词查询、结果列表、图片预览、JSON 查看和上下文跳转。
8. 设置：工作目录、LibreOffice 路径、模型配置、图片参数、并发参数和本地 API 设置。
```

#### Rationale

把工作台从具体操作容器改为概览与分流层，避免用户在多个入口看到重复或冲突操作。

### 4.2 PRD 功能需求补丁

#### OLD

```md
FR-002 文件导入：用户必须能够通过拖拽或文件选择导入 PDF、PPT、PPTX、DOC、DOCX 文件。
```

#### NEW

```md
FR-002 媒体导入：用户必须能够在 `媒体导入` 中通过拖拽或文件选择导入一个或多个图片/文档媒体文件。支持类型至少包括 PNG、JPG/JPEG、WEBP、PDF、PPT、PPTX、DOC、DOCX。
```

#### OLD

```md
FR-006 页面分析：系统必须对新生成或标记为需重跑的页面图片调用多模态模型，并生成 `page_analysis_v1` JSON。
```

#### NEW

```md
FR-006 页面分析与重分析：系统必须对新生成或用户明确要求重跑的页面图片调用多模态模型，并生成 `page_analysis_v1` JSON。重分析可以从 `媒体管理`、`模型分析` 或搜索/JSON 查看上下文进入，但模型调用与 JSON 保存必须由 `模型分析` 相关服务和可信保存管线执行。
```

#### ADD

建议追加：

- FR-013 导航与命名一致性。
- FR-014 工作台概览与功能分流。
- FR-015 媒体管理。
- FR-016 重分析上下文路由。
- FR-017 自定义提示词重分析。
- FR-018 JSON 编辑/微调。

#### Rationale

让 PRD 的需求层覆盖已确认的新导航、媒体管理、重分析和 JSON 微调能力。

### 4.3 Epic 与 Story 调整

#### OLD

```yaml
development_status:
  epic-1: done
  epic-2: done
  epic-3: done
  epic-4: done
  epic-5: done
```

#### NEW

新增 Correct Course 执行故事：

```md
### Change Story CC-2026-06-10: 媒体管理与工作台路由调整

As a 本地媒体处理用户,
I want `图片导入` 被统一更名为 `媒体导入`，并新增 `媒体管理` tab，把工作台中的具体媒体管理操作迁移出去,
So that 工作台只负责状态概览与功能分流，而导入、管理、重分析和 JSON 微调都发生在职责清晰的功能模块中。
```

并在 `sprint-status.yaml` 中追加：

```yaml
cc-2026-06-10-media-management-workbench-routing: backlog
```

#### Rationale

保留历史 done 记录，同时给本次中等变更一个明确 backlog/sprint 追踪入口。

### 4.4 Architecture 前端边界与导航上下文

#### OLD

```md
Use a simple app-level tab layout:

- Workbench
- Search
- Settings
```

#### NEW

```md
Use a simple app-level tab layout with explicit feature ownership:

- Workbench
- Media Import
- Media Management
- Model Analysis
- Export
- BM25 Index
- Search
- Settings
```

#### ADD

新增 Navigation Context Contract：

```md
Cross-tab actions must use a typed navigation context. The context should include only routing and restoration data, such as `source_tab`, `return_to`, `action`, `selected_kind`, `selected_ids`, `filter`, `query`, `scroll_anchor`, and `selection_count`.
```

#### Rationale

防止实现只改导航文案，而没有建立真正的 feature 边界和上下文路由契约。

### 4.5 UX 旅程与组件归属

#### OLD

```md
slicer 的定义性体验是：用户把一批本地文档拖入工作台，系统立即接住这些文件，自动开始转换，并快速把它们显影为可预览、可追溯、可搜索的页面级资产。
```

#### NEW

```md
slicer 的定义性体验是：用户从工作台看到当前资料处理状态和下一步入口，进入 `媒体导入` 把本地图片/文档媒体交给系统处理，再在 `媒体管理` 中确认页面资产、筛选状态、定位失败项，并在需要时进入 `模型分析` 进行重分析或 JSON 微调。
```

#### ADD

新增或调整组件：

- `MediaImportDropzone`
- `MediaManagementList`
- `ReanalysisContextSummary`

#### Rationale

将导入、管理、重分析和 JSON 微调放到用户心智更清晰的位置。

### 4.6 Implementation Handoff

Developer agent 应按以下任务执行：

1. 导航命名与 tab 增补。
2. 工作台职责收缩。
3. 媒体导入页面调整。
4. 新增媒体管理页面。
5. 重分析上下文路由。
6. 搜索与 JSON 入口边界。
7. 测试与验收。

## 5. Implementation Handoff

### Scope Classification

分类：Moderate。

理由：

- 跨 PRD、Architecture、UX、Epic/Story、前端实现和测试。
- 不改变底层核心架构、数据库主模型、分析服务、索引服务或 HTTP API。
- 需要 backlog reorganization 和 Developer agent 执行。

### Handoff Recipients

#### Product Owner / Planning Owner

职责：

- 审批本 Sprint Change Proposal。
- 将 Correct Course story 加入 sprint/backlog。
- 同步 PRD、Epic、Architecture、UX 和 project docs。
- 确认后续 story 优先级。

#### Developer Agent

职责：

- 按本 proposal 的 Implementation Handoff Tasks 修改前端。
- 保持服务边界：工作台与路由层不得直接执行具体业务逻辑。
- 复用现有 Rust service、tauriClient、SQLite、schema validator 和可信保存管线。
- 添加或更新测试。

#### Architect, optional

职责：

- 如 Developer agent 在 typed navigation context、feature boundaries 或 shared client contract 上遇到设计冲突，由 Architect 介入补充架构决策。

### Success Criteria

1. 用户可见 UI 不再出现旧文案 `图片导入`。
2. 主导航包含 `媒体管理`，且 `媒体导入` 命名一致。
3. 工作台只展示 overview、摘要和快捷入口。
4. 工作台不直接承载完整导入 dropzone、媒体/文档管理列表、删除、模型调用、JSON 保存、索引重建、搜索执行或导出执行。
5. `媒体导入` 支持图片与文档媒体导入入口和导入反馈。
6. `媒体管理` 承载媒体/文档/页面列表、筛选、详情、源文件定位、删除和重分析选择。
7. `重分析` 从 `媒体管理` 跳转到 `模型分析`，并携带 typed navigation context。
8. `模型分析` 能展示重分析上下文摘要，并提供默认重分析、自定义提示词重分析和 JSON 编辑/微调入口。
9. 搜索页和媒体管理不直接调用模型或保存 JSON。
10. 测试覆盖导航文案、工作台职责、媒体管理重分析路由和上下文降级。

## 6. Checklist Summary

### Section 1: Understand Trigger and Context

- [N/A] 1.1 触发 story：无单一触发 story，触发源为用户变更请求。
- [x] 1.2 核心问题：新增需求与现有信息架构/实现边界冲突。
- [x] 1.3 证据：已收集 PRD、Epic、Architecture、UX 和源码证据。

### Section 2: Epic Impact Assessment

- [x] 2.1 当前 Epic 可否按原计划完成：需要追加变更执行项。
- [!] 2.2 Epic 级变更：新增 Correct Course story。
- [x] 2.3 剩余 Epic 影响：Epic 1/2/3 主要受影响，Epic 4 轻微影响，Epic 5 基本不受影响。
- [x] 2.4 是否废弃或新增 Epic：不废弃，新增 change story。
- [x] 2.5 顺序与优先级：应优先于 MVP 验收/打包收口。

### Section 3: Artifact Conflict and Impact Analysis

- [!] 3.1 PRD 冲突：需要更新信息架构、工作台和功能需求。
- [!] 3.2 Architecture 冲突：需要更新前端边界与导航上下文。
- [!] 3.3 UX 冲突：需要更新旅程、组件归属和主导航模式。
- [!] 3.4 其他 artifact：需要同步 docs、sprint-status、测试计划。

### Section 4: Path Forward Evaluation

- [x] 4.1 Direct Adjustment：可行，推荐。
- [x] 4.2 Potential Rollback：不建议。
- [x] 4.3 PRD MVP Review：不需要缩减 MVP。
- [x] 4.4 Recommended Path：直接调整 + 新增变更执行项。

### Section 5: Sprint Change Proposal Components

- [x] 5.1 Issue Summary 已完成。
- [x] 5.2 Impact Analysis 已完成。
- [x] 5.3 Recommended Approach 已完成。
- [x] 5.4 PRD MVP Impact 与行动计划已完成。
- [x] 5.5 Handoff Plan 已完成。

### Section 6: Final Review and Handoff

- [x] 6.1 Review checklist completion
- [x] 6.2 Verify Sprint Change Proposal accuracy
- [x] 6.3 Obtain explicit user approval
- [x] 6.4 Update sprint-status.yaml after approval
- [x] 6.5 Confirm next steps and handoff plan

## 7. Approval

Current status: Approved for implementation.

Approval result:

- Approved by xq on 2026-06-11.
- Change scope classification: Moderate.
- Handoff route: Product Owner / Planning Owner for backlog tracking, then Developer Agent for implementation.
