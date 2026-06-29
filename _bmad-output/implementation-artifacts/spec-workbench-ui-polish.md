---
title: '工作台 UI 打磨：导入接收、资产摘要与页面预览'
type: 'feature'
created: '2026-06-09'
status: 'done'
baseline_commit: '24457e489312feca15ec1a22ff84e69165386127'
context:
  - 'D:/AIProject/slicer/_bmad-output/planning-artifacts/ux-design-specification.md'
  - 'D:/AIProject/slicer/_bmad-output/implementation-artifacts/spec-2-7-workbench-import-polish.md'
  - 'D:/AIProject/slicer/_bmad-output/implementation-artifacts/spec-batch-image-import.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** MVP 工作台已经能导入、展示文档、查看任务和触发分析，但视觉上仍偏面板/表格堆叠，用户最关心的“批量导入是否被接住”“页面图片是否已经生成”“下一步能做什么”不够突出。

**Approach:** 在现有 React/Tauri 前端上打磨工作台首屏与文档列表：增加更明确的导入接收区和拖拽反馈、批次/资产就绪摘要、页面首图/占位缩略图、可查看/可分析/失败可恢复的状态语言，并用更克制的黑白灰生产力软件视觉重整样式。

## Boundaries & Constraints

**Always:** 保留现有导入、重试、分析、删除、导出和索引命令；沿用现有本地组件与 CSS，不引入 MUI/Ant/Chakra 等 UI 库；页面图片必须来自 `PageWorkbenchDto.image_path` 的真实资产路径，不能伪造预览；拖拽入口与“选择文件”复用同一文档导入校验与导入流程；空、错、加载、失败重试入口必须比现在更清楚；长文件名、长路径和错误原因不能撑破布局。

**Ask First:** 如果需要修改 Rust 后端、DTO、Tauri 命令、索引自动触发语义，或把工作台与图片导入页重新合并，先停下来确认。

**Never:** 不重做搜索页、设置页或暗色模式；不改变图片导入独立 tab；不删除现有高级操作入口；不把工作台做成营销 hero、后台 dashboard 或卡片堆叠装饰页；不使用假缩略图代替真实页面资产。

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| 首次无工作区 | `workspaceStatus.status !== "ready"` | 工作台显示干净的工作区选择区，主操作是选择工作区，导入区不可误操作 | 工作区错误使用常驻错误块说明原因 |
| 拖拽文档 | 用户把 PDF/Office 文件拖到工作台 | 导入区进入明显接收态，松开后按现有导入流程逐文件显示结果并刷新文档/任务 | 不支持文件显示逐项原因，不吞掉已成功文件 |
| 已有页面图片 | 文档至少有一页 `image_path` | 文档行显示稳定首图缩略图和“查看页面”入口，点击仍调用现有打开图片逻辑 | 图片缺失时显示非空占位和不可用说明 |
| 批量处理混合状态 | 文档包含 ready/importing/failed，页面包含 rendered/analyzed/failed | 顶部摘要显示文档数、页面数、已生成页面、待分析、失败和处理中，状态语言回答下一步可做什么 | 失败数量和重试入口在对应文档行保留 |

</frozen-after-approval>

## Code Map

- `src/features/workbench/WorkbenchPage.tsx` -- 工作台主页面、导入流程、页面/分析统计计算、各面板排序。
- `src/features/workbench/components/DocumentList.tsx` -- 文档资产列表、首图缩略图、状态语言、行级操作、页面详情。
- `src/features/workbench/components/WorkspacePicker.tsx` -- 工作区状态与选择入口，服务无工作区首屏。
- `src/features/workbench/components/ImportResultList.tsx` -- 逐文件导入结果，需适配新的导入区视觉。
- `src/features/workbench/components/JobList.tsx` -- 任务队列展示，保留但降低首屏主导感。
- `src/components/common/Button.tsx` / `StatusBadge.tsx` / `ErrorMessage.tsx` -- 复用现有基础组件，不新增库。
- `src/styles/globals.css` -- 全局视觉 tokens、工作台布局、导入接收态、资产摘要、缩略图、响应式样式。
- `src/lib/fileValidation.ts` / `src/lib/tauriClient.ts` -- 只在拖拽需要复用现有文档导入 helper 时轻触，不改变后端契约。

## Tasks & Acceptance

**Execution:**
 - [x] `src/features/workbench/WorkbenchPage.tsx` -- 抽出共享 `importDocumentFiles(filePaths)`，让按钮选择与拖拽导入复用；新增拖拽状态、工作台摘要数据和更聚焦的首屏结构。
 - [x] `src/features/workbench/components/DocumentList.tsx` -- 将表格化文档行改为中等紧凑的资产行：稳定首图/占位、文件信息、页面/分析状态、失败原因和操作入口同屏可扫。
 - [x] `src/features/workbench/components/WorkspacePicker.tsx` / `ImportResultList.tsx` -- 对齐新的工作台语言与视觉密度，确保无工作区、导入中、导入结果都清楚。
 - [x] `src/styles/globals.css` -- 引入黑白灰为主的 tokens，补充工作台 hero-free 布局、拖拽 overlay/zone、摘要条、缩略图、行级状态、窄屏降级和焦点样式。
 - [x] `src/lib/fileValidation.ts` / `src/lib/tauriClient.ts` -- 仅在需要时复用既有文档类型判断和多文档导入，不扩展新业务能力。

**Acceptance Criteria:**
- Given 未选择工作区, when 打开工作台, then 页面优先展示工作区选择与当前状态，不出现可误点的导入承诺。
- Given 用户把 PDF/Office 拖到工作台, when 悬停和松开, then 导入区出现明显接收态，并在松开后逐文件显示成功、重复、不支持或失败结果。
- Given 文档已有页面图片, when 查看文档列表, then 每行显示真实首图缩略图或稳定占位，并提供“查看页面”入口。
- Given 页面已生成但尚未分析, when 查看工作台, then 状态语言能表达“页面可查看/可分析”，不会误称“可搜索”。
- Given 有失败文档或失败页面, when 查看工作台, then 失败原因和重试入口在对应文档上下文中可见。
- Given 窗口宽度降到窄桌面, when 查看工作台, then 导入区、摘要和文档行重排而不发生文字重叠或横向破版。

## Design Notes

工作台应更接近“资料资产显影台”，而不是后台任务仪表盘。首屏优先级为：工作区状态、导入接收、资产就绪摘要、文档/页面资产列表；任务队列、分析、导出和索引状态可以保留，但不应压过页面资产确认。视觉以中性色、8px 圆角、紧凑间距和清晰焦点环为主，少量状态色只用于成功、处理中、警告和失败。

## Verification

**Commands:**
- `npm run build` -- expected: TypeScript 与 Vite 构建通过。
- `git diff --check` -- expected: 无空白错误。

**Results:**
- `npx tsc --noEmit --pretty false` -- passed.
- `npm run build` -- passed.
- `git diff --check` -- passed.
- Browser sanity check on `http://127.0.0.1:5173` -- passed for workbench first-screen render; ordinary browser preview still reports existing Tauri `invoke` unavailability for workspace status.
- Review patch pass -- fixed import lock/finally safety, native/DOM drop duplicate handling, real `PageWorkbenchDto.image_path` thumbnail source, visible failed-page summary, top-level failure count, scoped workbench styling, list semantics, pathless drop feedback, and thumbnail containment.
- Tauri asset protocol config -- enabled with empty static scope so `convertFileSrc` can display runtime-scoped workspace page assets without adding a new command.

**Manual checks (if no CLI):**
- 在桌面宽度和窄窗口下检查工作台首屏、拖拽态、文档行长文件名、失败原因、缩略图占位和按钮文字不重叠。
- 在 Tauri 桌面壳中选择工作区后检查页面缩略图是否从 `PageWorkbenchDto.image_path` 对应的真实页面文件加载；普通浏览器无法验证 Tauri asset scope。

## Suggested Review Order

**导入接收与并发安全**

- 统一按钮与拖拽导入，逐文件保持结果可见。
  [`WorkbenchPage.tsx:159`](../../src/features/workbench/WorkbenchPage.tsx#L159)

- 导入中再次拖入也给明确反馈。
  [`WorkbenchPage.tsx:234`](../../src/features/workbench/WorkbenchPage.tsx#L234)

- Tauri 原生拖拽只注册一次并防重复 drop。
  [`WorkbenchPage.tsx:677`](../../src/features/workbench/WorkbenchPage.tsx#L677)

- HTML drop 保留无路径文件的失败结果。
  [`WorkbenchPage.tsx:275`](../../src/features/workbench/WorkbenchPage.tsx#L275)

- 路径去重和类型过滤收在入口边界。
  [`WorkbenchPage.tsx:1108`](../../src/features/workbench/WorkbenchPage.tsx#L1108)

**工作台首屏与摘要**

- 首屏从应用名转向工作台价值表达。
  [`WorkbenchPage.tsx:755`](../../src/features/workbench/WorkbenchPage.tsx#L755)

- 导入区突出拖入后自动转换。
  [`WorkbenchPage.tsx:780`](../../src/features/workbench/WorkbenchPage.tsx#L780)

- 摘要条回答文档、页面、待分析和失败。
  [`WorkbenchPage.tsx:804`](../../src/features/workbench/WorkbenchPage.tsx#L804)

- 失败计数与行级分析失败口径一致。
  [`WorkbenchPage.tsx:1042`](../../src/features/workbench/WorkbenchPage.tsx#L1042)

**文档资产行**

- 表格视觉换成语义化资产列表。
  [`DocumentList.tsx:131`](../../src/features/workbench/components/DocumentList.tsx#L131)

- 每行保留真实操作入口与状态扫描。
  [`DocumentList.tsx:185`](../../src/features/workbench/components/DocumentList.tsx#L185)

- 失败页原因默认在文档上下文可见。
  [`DocumentList.tsx:207`](../../src/features/workbench/components/DocumentList.tsx#L207)

- 缩略图直接来自 `PageWorkbenchDto.image_path`。
  [`DocumentList.tsx:517`](../../src/features/workbench/components/DocumentList.tsx#L517)

- 最小开启 asset protocol 支撑真实页面图片显示。
  [`tauri.conf.json:22`](../../src-tauri/tauri.conf.json#L22)

**样式作用域与响应式**

- 新视觉 tokens 限定在工作台页面内。
  [`globals.css:301`](../../src/styles/globals.css#L301)

- 拖拽区、摘要和导入结果只打磨工作台。
  [`globals.css:343`](../../src/styles/globals.css#L343)

- 文档行和缩略图使用稳定尺寸。
  [`globals.css:1114`](../../src/styles/globals.css#L1114)

- 页面缩略图完整显示而不裁切。
  [`globals.css:1153`](../../src/styles/globals.css#L1153)

- 窄屏下导入区、摘要和资产行重排。
  [`globals.css:1476`](../../src/styles/globals.css#L1476)
