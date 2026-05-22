---
title: 'Story 2.3: Office 文档转换为中间 PDF 并接入页面渲染'
type: 'feature'
created: '2026-05-19'
status: 'done'
baseline_commit: 'NO_VCS'
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Story 2-2 仅支持 PDF 文件导入，用户无法导入 PPT、PPTX、DOC、DOCX 等 Office 文档。需要通过 LibreOffice headless 将 Office 文档转换为中间 PDF，再接入已有的 PDF 渲染流水线。

**Approach:** 新增 `DocumentConverter` trait 和 `LibreOfficeConverter` 实现，通过 `libreoffice --headless --convert-to pdf` 命令完成转换。Import service 根据文件扩展名判断：PDF 直接渲染，Office 文件先转换再渲染。中间 PDF 默认删除，转换失败时保留供排查。LibreOffice 路径从 settings 读取，未配置或不可用时返回可恢复错误。

## Boundaries & Constraints

**Always:**
- LibreOffice 通过 `std::process::Command` 调用，不使用 FFI
- 转换在 `tmp/` 目录进行，结果 PDF 写入 `tmp/<document_id>_converted.pdf`
- 中间 PDF 在页面渲染成功后删除，转换失败时保留
- LibreOffice 缺失或转换失败是可恢复错误（`retryable: true`）
- 支持的 Office 类型：`.doc`, `.docx`, `.ppt`, `.pptx`
- LibreOffice 路径来自 settings 的 `libreoffice_path` 字段
- 文件类型检测基于扩展名，LibreOffice 自行处理格式细节
- `file_type` 字段记录原始类型（如 `"pptx"`），不是 `"pdf"`

**Ask First:**
- 无

**Never:**
- 不实现 XLS/XLSX 转换（不在需求范围内）
- 不实现 PDF 密码输入解密
- 不修改已有的 PDF 渲染逻辑
- 不在转换过程中显示逐页进度（整体进度即可）

</frozen-after-approval>

## Code Map

- `src-tauri/src/providers/mod.rs` -- 注册新模块
- `src-tauri/src/providers/converter.rs` -- 新文件：`DocumentConverter` trait 定义
- `src-tauri/src/providers/libreoffice_converter.rs` -- 新文件：`LibreOfficeConverter` 实现
- `src-tauri/src/services/import_service.rs` -- 扩展：根据文件类型分流（PDF 直接渲染 / Office 先转换）
- `src-tauri/src/services/settings_service.rs` -- 读取 `libreoffice_path` 配置
- `src-tauri/src/commands/import_commands.rs` -- 扩展：接受 Office 文件扩展名
- `src/lib/fileValidation.ts` -- 扩展 `SUPPORTED_EXTENSIONS` 添加 Office 类型
- `src/lib/tauriClient.ts` -- 更新 `openMultiPdfDialog` 为通用文件对话框
- `src/features/workbench/WorkbenchPage.tsx` -- 更新文件对话框和 UI 文案

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/src/providers/converter.rs` -- 定义 `DocumentConverter` trait：`fn convert_to_pdf(&self, input_path: &Path, output_dir: &Path) -> AppResult<PathBuf>`
- [x] `src-tauri/src/providers/libreoffice_converter.rs` -- 实现 `LibreOfficeConverter`：接收 libreoffice_path，调用 `--headless --convert-to pdf`，返回生成的 PDF 路径
- [x] `src-tauri/src/providers/mod.rs` -- 添加 `pub mod converter; pub mod libreoffice_converter;`
- [x] `src-tauri/src/services/import_service.rs` -- 新增 `import_document` 方法：检测扩展名，PDF 走已有流程，Office 文件先调用 converter 再渲染，成功后删除中间 PDF
- [x] `src-tauri/src/services/settings_service.rs` -- 添加 `get_libreoffice_path() -> AppResult<String>` 方法，未配置时返回可恢复错误
- [x] `src-tauri/src/commands/import_commands.rs` -- 扩展 `import_pdf`：PDF 走直接渲染，Office 文件调用 converter + renderer
- [x] `src/lib/fileValidation.ts` -- 扩展 `SUPPORTED_EXTENSIONS` 添加 `.doc`, `.docx`, `.ppt`, `.pptx`
- [x] `src/lib/tauriClient.ts` -- 新增 `openImportDialog`，filters 添加 Office 类型
- [x] `src/features/workbench/WorkbenchPage.tsx` -- 更新 UI 文案：「导入文档」

**Acceptance Criteria:**
- Given 用户选择一个 .pptx 文件, when LibreOffice 已配置且可用, then 文件被转换为 PDF 并逐页渲染为 PNG，文档记录的 file_type 为 "pptx"
- Given 用户选择一个 .docx 文件, when 转换成功, then 中间 PDF 在渲染完成后被自动删除
- Given LibreOffice 未配置, when 用户尝试导入 Office 文件, then 返回可恢复错误「请在设置中配置 LibreOffice 路径」
- Given LibreOffice 路径配置错误（不存在的路径）, when 转换执行, then 返回可恢复错误并保留中间文件供排查
- Given 用户选择 .pdf 文件, when 导入, then 走已有的直接渲染流程，不调用 LibreOffice
- Given 用户同时选择 PDF 和 PPTX 文件, when 批量导入, then PDF 直接渲染，PPTX 经转换后渲染，各自独立成功或失败
- Given 转换失败（如文件损坏）, when 系统处理, then 中间 PDF 保留在 tmp/ 目录，任务标记 failed 并显示中文错误信息

## Verification

**Commands:**
- `cargo test --lib` -- expected: 15 测试通过
- `npx tsc --noEmit` -- expected: TypeScript 通过
- `npx vite build` -- expected: Vite 构建通过

## Suggested Review Order

1. `src-tauri/src/providers/converter.rs` -- DocumentConverter trait 和辅助函数
2. `src-tauri/src/providers/libreoffice_converter.rs` -- LibreOffice 调用实现
3. `src-tauri/src/services/settings_service.rs` -- get_libreoffice_path 方法
4. `src-tauri/src/services/import_service.rs` -- import_document 编排逻辑
5. `src-tauri/src/commands/import_commands.rs` -- 命令层分流逻辑
6. `src/lib/fileValidation.ts` + `src/lib/tauriClient.ts` -- 前端类型校验和对话框
7. `src/features/workbench/WorkbenchPage.tsx` -- UI 文案更新
