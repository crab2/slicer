---
title: 'Story 2.7: 工作台导入体验完善 — 任务列表、失败原因和处理入口'
type: 'feature'
created: '2026-05-19'
status: 'done'
baseline_commit: 'NO_VCS'
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** 工作台导入流程功能完整但体验粗糙：导入结果仅在当次操作后显示，刷新或切换页面后丢失；任务列表文案仍为 placeholder（"不启动真实导入"）；文档卡片缺少总量统计；多文件导入时无逐文件进度反馈。

**Approach:** 完善工作台 UI 层：移除 placeholder 文案，添加文档/页面统计摘要，多文件导入时逐文件显示进度和结果，导入完成后结果持久显示在文档列表中（通过刷新 documents 实现）。

## Boundaries & Constraints

**Always:**
- 不修改后端 Rust 代码，仅调整前端 React 组件
- 导入结果通过刷新 documents/jobs 状态获取，不引入新的持久化机制
- 保留现有的重试、进度、失败原因显示功能
- 统计数据从已加载的 documents/pagesByDocument 计算，不新增后端查询

**Ask First:**
- 无

**Never:**
- 不引入新的状态管理库（如 Redux）
- 不修改 DTO 结构或 Tauri 命令
- 不添加后端 API

</frozen-after-approval>

## Code Map

- `src/features/workbench/WorkbenchPage.tsx` -- 主页面：移除 placeholder、添加统计、优化导入流程
- `src/features/workbench/components/DocumentList.tsx` -- 文档列表：添加统计摘要头
- `src/features/workbench/components/JobList.tsx` -- 任务列表：更新文案，移除 placeholder 描述
- `src/features/workbench/components/ImportResultList.tsx` -- 导入结果：无变更（已完善）
- `src/styles/globals.css` -- 添加统计摘要样式

## Tasks & Acceptance

**Execution:**
- [x] `src/features/workbench/WorkbenchPage.tsx` -- 移除 `placeholderTasks` 和底部 placeholder 面板；多文件导入时逐文件更新 importResults 状态（导入一个 push 一个结果）
- [x] `src/features/workbench/components/DocumentList.tsx` -- 在文档列表顶部添加统计摘要行：文档总数、总页面数、失败数
- [x] `src/features/workbench/components/JobList.tsx` -- 更新 panel 描述文案，移除"不启动真实导入"等 placeholder 措辞
- [x] `src/styles/globals.css` -- 添加 `.doc-summary` 统计摘要样式

**Acceptance Criteria:**
- Given 用户导入多个文件, when 导入进行中, then 每完成一个文件立即在结果列表中显示该文件的状态（不再等全部完成）
- Given 工作区有 3 个文档共 10 页, when 查看文档列表, then 顶部显示"3 个文档 · 10 页"统计摘要
- Given 有一个失败的文档, when 查看统计摘要, then 显示失败数量（如"1 个失败"）
- Given 用户刷新页面后, when 查看工作台, then 之前导入的文档和任务仍然可见（数据来自 SQLite），底部无 placeholder 面板
- Given 任务列表面板, when 查看描述文案, then 不包含"不启动真实导入"等 placeholder 措辞

## Verification

**Commands:**
- `npx tsc --noEmit` -- expected: TypeScript 通过
- `npx vite build` -- expected: Vite 构建通过

## Suggested Review Order

**多文件导入逐文件反馈**

- 导入循环中每完成一个文件立即 `setImportResults`，不再等全部完成
  [`WorkbenchPage.tsx:116`](../../src/features/workbench/WorkbenchPage.tsx#L116)

**文档统计摘要**

- 从 documents/pagesByDocument 计算总数和失败数，渲染摘要行
  [`DocumentList.tsx:33`](../../src/features/workbench/components/DocumentList.tsx#L33)

**移除 placeholder**

- 删除 `placeholderTasks` 常量和底部两块 placeholder 面板
  [`WorkbenchPage.tsx:12`](../../src/features/workbench/WorkbenchPage.tsx#L12)

- 任务列表描述文案更新为真实内容
  [`JobList.tsx:32`](../../src/features/workbench/components/JobList.tsx#L32)

**样式**

- `.doc-summary` 统计摘要样式
  [`globals.css:572`](../../src/styles/globals.css#L572)
