---
title: 'Story 2.2: 多文件导入、文件类型校验与重复识别'
type: 'feature'
created: '2026-05-19'
status: 'done'
baseline_commit: 'NO_VCS'
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Story 2-1 只支持单个 PDF 导入，用户无法一次选择多个文件批量导入，也没有文件类型校验——选择非 PDF 文件会导致后端报错而非前端友好提示。

**Approach:** 前端文件对话框允许多选，逐个调用已有的 `importPdf` 命令，汇总显示每个文件的结果（成功/重复/失败）。前后端均增加文件类型校验（当前仅支持 `.pdf`），不支持的文件类型在导入前拦截并给出中文提示。重复文件已由 2-1 的哈希检测处理，前端需展示"已存在"状态。

## Boundaries & Constraints

**Always:**
- 多文件导入是前端循环调用单文件命令，不新增后端批量命令
- 文件类型校验：仅 `.pdf` 通过，其他扩展名在导入前拦截
- 前端校验为第一道防线，后端校验为安全兜底
- 重复检测基于文件哈希（2-1 已实现），返回已有 document 视为成功
- 部分失败不影响其他文件继续导入
- 每个文件独立显示结果状态

**Ask First:**
- 无

**Never:**
- 不实现 Office 文件转换（属于 Story 2-3）
- 不添加批量进度条（属于 Story 2-5）
- 不修改已有的单文件 import_pdf 命令签名
- 不新增后端批量导入命令

</frozen-after-approval>

## Code Map

- `src/lib/tauriClient.ts` -- 添加 `importMultiplePdf` 方法，循环调用 `importPdf`
- `src/types/app.ts` -- 新增 `ImportResultDto` 类型（单文件导入结果）
- `src/features/workbench/WorkbenchPage.tsx` -- 多选文件对话框，批量导入逻辑，结果展示
- `src/features/workbench/components/ImportResultList.tsx` -- 新组件：每个文件的导入结果状态
- `src-tauri/src/commands/import_commands.rs` -- 添加文件扩展名校验兜底
- `src-tauri/src/providers/pdf_renderer.rs` -- `sanitize_filename` 和 `compute_file_hash` 已有，无需修改

## Tasks & Acceptance

**Execution:**
- [x] `src/types/app.ts` -- 添加 `ImportResultDto` 接口（file_name, status, document?, error?）
- [x] `src/lib/tauriClient.ts` -- 添加 `importMultiplePdf` 方法：接收文件路径数组，逐个调用 `importPdf`，捕获错误，返回 `ImportResultDto[]`
- [x] `src/lib/fileValidation.ts` -- 新建文件类型校验工具：`isSupportedFileType(path)` 检查扩展名，`SUPPORTED_EXTENSIONS` 常量
- [x] `src-tauri/src/commands/import_commands.rs` -- `import_pdf` 中添加 `.pdf` 扩展名校验，非 PDF 返回明确错误
- [x] `src/features/workbench/WorkbenchPage.tsx` -- 修改 `handleImportPdf`：允许多选，校验类型，调用批量导入，展示结果
- [x] `src/features/workbench/components/ImportResultList.tsx` -- 新组件：显示每个文件的导入结果（成功/重复/失败/不支持）

**Acceptance Criteria:**
- Given 用户已选择可用工作区, when 用户在文件对话框中选择 3 个 PDF 文件, then 3 个文件均被导入，工作台显示 3 个文档
- Given 用户选择 2 个 PDF 和 1 个 .docx 文件, when 系统处理导入, then 2 个 PDF 正常导入，.docx 被拦截并显示"不支持的文件类型"
- Given 用户选择一个已导入过的 PDF 文件, when 系统检测到重复, then 返回已有文档信息并显示"文件已存在"状态
- Given 批量导入中有 1 个文件失败, when 其余文件成功, then 成功的文件正常入库，失败的文件显示错误原因，不影响其他文件
- Given 用户选择多个文件但全部为不支持的类型, when 系统校验, then 不调用后端命令，直接显示所有文件不支持的提示
- Given 开发者验证, when 检查后端 import_pdf 命令, then 非 .pdf 扩展名的文件被安全拒绝并返回结构化错误

## Verification

**Commands:**
- `cargo test --lib` -- expected: 15 测试通过
- `npx tsc --noEmit` -- expected: TypeScript 通过
- `npx vite build` -- expected: Vite 构建通过

## Suggested Review Order

1. `src/lib/fileValidation.ts` -- 校验工具逻辑
2. `src/types/app.ts` -- ImportResultDto 类型定义
3. `src-tauri/src/commands/import_commands.rs` -- 后端扩展名校验兜底
4. `src/lib/tauriClient.ts` -- importMultiplePdf 和 openMultiPdfDialog 方法
5. `src/features/workbench/components/ImportResultList.tsx` -- 结果展示组件
6. `src/features/workbench/WorkbenchPage.tsx` -- 多文件导入集成和重复检测逻辑
