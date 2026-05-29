---
title: 'MVP 收尾：导出功能提交 + 异步安全 + 后端健壮性'
type: 'chore'
created: '2026-05-28'
status: 'done'
baseline_commit: 'b4b6d63'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/deferred-work.md'
  - '{project-root}/_bmad-output/implementation-artifacts/mvp-acceptance-plan.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** MVP 所有功能已完成，但有三类收尾工作未处理：(1) export_v1.0 分支上的导出功能未提交；(2) 前端存在异步竞态和 cleanup 缺失问题；(3) 后端 job 处理缺少边界校验和容错能力。

**Approach:** 分四阶段执行：先提交导出功能代码，再修复前端异步安全问题（generation counter + useEffect cleanup），然后修复后端健壮性问题（progress clamp + per-job 容错），最后按验收计划执行功能验收。

## Boundaries & Constraints

**Always:**
- 所有修改必须通过现有 Rust 编译和 TypeScript 类型检查
- 前端异步修复不得改变现有 UI 行为，仅消除竞态
- 后端修复不得改变 SQL schema（CHECK 约束已正确）
- 提交导出功能时保持原子提交，不混入其他修复

**Ask First:**
- 如果验收测试发现新问题，是否在本次收尾中修复还是延迟到下个版本
- 后端 recover_interrupted_jobs 选择事务方案还是 per-job 容错方案

**Never:**
- 不删除或修改 SQL 迁移文件
- 不引入新的外部依赖
- 不重构不相关的代码

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| 快速切换工作区 | 多次连续 refreshJobs 调用 | 仅最后一次调用的结果生效 | 旧调用的 setState 被忽略 |
| 组件卸载时异步进行中 | useEffect cleanup 触发 | 不执行过期的 setState | cancelled 标志守卫 |
| progress > 100 | orchestrator 传入 150 | SQL UPDATE 使用 100 | clamp 到 100 |
| 单个 job 数据损坏 | list_jobs 查询 | 跳过坏行，返回其余 job | 记录警告日志 |
| 恢复中断 job 部分失败 | recover_interrupted_jobs 循环中某行失败 | 已处理的 job 保持 failed，未处理的保持 running | per-job 容错继续循环 |

</frozen-after-approval>

## Code Map

- `src/features/workbench/WorkbenchPage.tsx` -- 工作台主页面，含 refreshJobs、useEffect hooks、export 集成
- `src-tauri/src/repositories/ledger_repository.rs` -- job CRUD + progress 更新 + 恢复逻辑
- `src-tauri/src/artifacts/media_exporter.rs` -- 媒体导出核心逻辑（新增文件）
- `src-tauri/src/commands/export_commands.rs` -- Tauri 导出命令桥接（新增文件）
- `src/app/AppShell.tsx` -- 应用 shell，新增 analysis/export/index 视图
- `src/app/navigation.ts` -- 导航配置，扩展 ViewId
- `src/lib/tauriClient.ts` -- 前端 Tauri 客户端，新增 exportMedia API
- `src/types/app.ts` -- MediaExportResultDto 类型定义

## Tasks & Acceptance

**Execution:**

- [x] `src-tauri/src/artifacts/media_exporter.rs` + `src-tauri/src/commands/export_commands.rs` + `src/app/AppShell.tsx` + `src/app/navigation.ts` + `src/lib/tauriClient.ts` + `src/types/app.ts` + `src/features/export/ExportPage.tsx` + `src/features/analysis/AnalysisPage.tsx` + `src/features/index/IndexPage.tsx` + `src/features/workbench/WorkbenchPage.tsx` -- git add 并提交所有未提交的导出功能变更，提交信息: "feat: add media export and dedicated analysis/export/index pages"

- [x] `src/features/workbench/WorkbenchPage.tsx` -- 为 refreshJobs 和 refreshDocuments 添加 generation counter，确保只有最新调用的 setState 生效；在两个 useEffect 中添加 cleanup 函数设置 cancelled 标志

- [x] `src/features/workbench/WorkbenchPage.tsx` -- 将 recoveredWorkspaceRef.current 的赋值移到 recoverInterruptedJobs 调用之前（乐观设置），防止并发恢复

- [x] `src-tauri/src/repositories/ledger_repository.rs` -- 在 update_job_progress 中添加 `progress = progress.min(100)` clamp

- [x] `src-tauri/src/repositories/ledger_repository.rs` -- 将 recover_interrupted_jobs 改为 per-job 容错：循环内 catch 单个 job 的错误，记录警告，继续处理其余 job

- [x] `src-tauri/src/repositories/ledger_repository.rs` -- 将 list_jobs 中的 collect 改为 filter_map + 静默跳过坏行，记录被跳过的 row ID

**Acceptance Criteria:**

- Given 用户快速切换工作区，When 多次 refreshJobs 并发执行，Then 仅最后一次的结果正确显示在 UI 中
- Given 组件卸载时异步请求进行中，When cleanup 触发，Then 不执行已过期的 setState
- Given orchestrator 传入 progress=150，When update_job_progress 执行，Then SQL UPDATE 使用 progress=100 且不报错
- Given 某个 job 行数据损坏，When list_jobs 执行，Then 返回其余正常 job 并记录警告日志
- Given recover_interrupted_jobs 循环中第 2 个 job 恢复失败，When 函数继续执行，Then 第 3 个及后续 job 仍被正常恢复

## Spec Change Log

### Iteration 1 (2026-05-28)

**Triggering finding:** 共享 `dataGenerationRef` 导致 `refreshJobs` 和 `refreshDocuments` 同时调用时，前者的 gen 检查总是失败（Critical）

**What was amended:**
- 拆分 `dataGenerationRef` 为 `jobsGenRef` 和 `docsGenRef`，各自独立管理
- `refreshDocuments` 改为先完成所有异步操作再批量更新 `setDocuments` + `setPagesByDocument`
- 移除 useEffect 中多余的 generation 递增
- `media_exporter.rs` 预创建 `media-export` 目录
- `atomic_write_str` 失败时清理 `.tmp` 文件

**Known-bad state avoided:** jobs 状态永远为空、isJobsLoading 永远为 true、文档列表与页面信息不一致

**KEEP instructions:** generation counter 模式本身正确，关键是要为独立的数据源使用独立的 counter

## Suggested Review Order

**前端异步安全（核心修复）**

- 独立 generation counter 防止 refreshJobs 结果被丢弃
  [`WorkbenchPage.tsx:78`](../../src/features/workbench/WorkbenchPage.tsx#L78)

- refreshDocuments 批量更新防止 documents/pagesByDocument 不一致
  [`WorkbenchPage.tsx:102`](../../src/features/workbench/WorkbenchPage.tsx#L102)

- useEffect workspace 切换时递增两个 counter 并 cleanup cancelled 标志
  [`WorkbenchPage.tsx:408`](../../src/features/workbench/WorkbenchPage.tsx#L408)

**后端健壮性**

- progress clamp 防止 SQL CHECK 约束违规
  [`ledger_repository.rs:123`](../../src-tauri/src/repositories/ledger_repository.rs#L123)

- per-job 容错：单个 job 恢复失败不中断批量处理
  [`ledger_repository.rs:195`](../../src-tauri/src/repositories/ledger_repository.rs#L195)

- filter_map 跳过损坏行，返回其余正常 job
  [`ledger_repository.rs:101`](../../src-tauri/src/repositories/ledger_repository.rs#L101)

**导出功能**

- 预创建 media-export 目录避免写入失败
  [`media_exporter.rs:116`](../../src-tauri/src/artifacts/media_exporter.rs#L116)

- atomic_write_str 失败时清理 .tmp 文件
  [`media_exporter.rs:276`](../../src-tauri/src/artifacts/media_exporter.rs#L276)

**新增文件与类型**

- 导出命令桥接
  [`export_commands.rs:8`](../../src-tauri/src/commands/export_commands.rs#L8)

- MediaExportResultDto 类型定义
  [`app.ts:242`](../../src/types/app.ts#L242)

## Design Notes

**Generation Counter 模式：** 使用 `useRef<number>` 作为 generation counter，每次 refreshJobs 调用时递增。异步回调中检查当前 generation 是否与调用时一致，不一致则跳过 setState。这比 AbortController 更轻量，因为 Tauri IPC 不支持 abort。

**Per-job 容错 vs 事务：** 选择 per-job 容错而非事务，因为 recover_interrupted_jobs 的目的是尽可能恢复更多 job，事务的 all-or-nothing 语义反而不利。坏行跳过同理——一个损坏的 job 不应阻止用户查看其他正常 job。

## Verification

**Commands:**
- `cd D:/AIProject/slicer && npm run check` -- expected: TypeScript 类型检查通过
- `cd D:/AIProject/slicer/src-tauri && cargo check` -- expected: Rust 编译检查通过

**Manual checks:**
- 快速切换工作区两次，确认 UI 正确显示新工作区的 job 列表
- 关闭应用时观察控制台无 React setState 警告
