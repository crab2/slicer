---
title: 'Story 2.5: 导入任务进度、失败恢复与重试能力'
type: 'feature'
created: '2026-05-19'
status: 'done'
baseline_commit: 'NO_VCS'
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** 导入失败后用户无法重试，只能重新选择文件。中断的导入任务（如应用崩溃）没有自动恢复机制。导入过程中的进度信息仅在 job 列表中显示，文档卡片上看不到进度。

**Approach:** 在失败的文档卡片上添加「重试」按钮，使用文档记录的 `original_path` 重新触发导入。应用启动时自动调用 `recover_interrupted_jobs` 将中断任务标记为 failed。在文档列表中为 importing 状态的文档显示关联 job 的进度。

## Boundaries & Constraints

**Always:**
- 重试使用文档记录的 `original_path` 字段，不需要用户重新选择文件
- 重试前检查原文件是否仍然存在，不存在则显示错误
- 中断恢复在工作区就绪时自动执行（已由 WorkbenchPage 的 `recoverInterrupted` 机制处理）
- 进度显示通过关联 job_id 查询 job 状态实现
- 重试创建新 job，不影响原失败 job 的记录

**Ask First:**
- 无

**Never:**
- 不修改 JobOrchestrator 核心逻辑
- 不实现后台队列或异步重试（同步执行即可）
- 不删除失败的旧 job 记录（保留历史）

</frozen-after-approval>

## Code Map

- `src-tauri/src/commands/import_commands.rs` -- 新增 `retry_import` Tauri 命令
- `src-tauri/src/services/import_service.rs` -- 新增 `retry_import` 方法：查找文档、检查原文件、重新导入
- `src-tauri/src/repositories/document_repository.rs` -- 新增 `find_document_by_id` 方法
- `src/lib/tauriClient.ts` -- 添加 `retryImport` 方法
- `src/types/app.ts` -- 无变更（DocumentDto 已有 original_path 和 job_id）
- `src/features/workbench/components/DocumentList.tsx` -- 添加重试按钮、进度显示
- `src/features/workbench/WorkbenchPage.tsx` -- 添加 retry 处理函数

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/src/repositories/document_repository.rs` -- 新增 `find_document_by_id(conn, document_id) -> AppResult<Option<DocumentDto>>`
- [x] `src-tauri/src/services/import_service.rs` -- 新增 `retry_import` 方法：查找文档、验证原文件存在、删除旧页面目录、重新执行导入
- [x] `src-tauri/src/commands/import_commands.rs` -- 新增 `retry_import` Tauri 命令
- [x] `src-tauri/src/lib.rs` -- 注册 `retry_import` 命令
- [x] `src/lib/tauriClient.ts` -- 添加 `retryImport(documentId)` 方法
- [x] `src/features/workbench/components/DocumentList.tsx` -- 失败文档显示重试按钮；importing 状态显示进度
- [x] `src/features/workbench/WorkbenchPage.tsx` -- 添加 `handleRetryImport` 函数

**Acceptance Criteria:**
- Given 一个导入失败的文档, when 用户点击重试按钮, then 系统使用 original_path 重新导入，创建新 job
- Given 原文件已被删除, when 用户尝试重试, then 显示错误「原文件不存在，无法重试」
- Given 一个 importing 状态的文档关联了 job, when 用户查看文档列表, then 显示当前进度百分比和进度消息
- Given 应用启动时有 interrupted 状态的 job, when 工作区就绪, then 自动标记为 failed 并可重试
- Given 重试成功, when 查看文档列表, then 文档状态更新为 ready，旧 failed job 记录保留

## Verification

**Commands:**
- `cargo test --lib` -- expected: 15 测试通过
- `npx tsc --noEmit` -- expected: TypeScript 通过
- `npx vite build` -- expected: Vite 构建通过

## Suggested Review Order

1. `src-tauri/src/repositories/document_repository.rs` -- `find_document_by_id` 方法
2. `src-tauri/src/services/import_service.rs` -- `retry_import` 方法（清理旧记录、重新导入）
3. `src-tauri/src/commands/import_commands.rs` -- `retry_import` 命令
4. `src/lib/tauriClient.ts` -- `retryImport` 方法
5. `src/features/workbench/components/DocumentList.tsx` -- 重试按钮和进度显示
6. `src/features/workbench/WorkbenchPage.tsx` -- `handleRetryImport` 集成
