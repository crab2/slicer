---
title: '批量导入图片'
type: 'feature'
created: '2026-06-09'
status: 'done'
route: 'one-shot'
baseline_commit: '1d513dc11a610f82a1dd351764460b00b8ef6281'
context: []
---

# 批量导入图片

## Intent

**Problem:** 工作台导入只支持 PDF 和 Office 文档，用户无法一次选择多张图片并让它们进入现有页面分析流程。

**Approach:** 将 PNG/JPG/JPEG 作为单页文档导入：原图进入 `originals/`，规范化页面 PNG 进入现有 `image_assets`/`page_records` 账本；前端在左侧导航新增“图片导入”tab，独立承载图片选择、导入结果、图片文档列表、预览定位、重试与删除管理。工作台导入入口恢复为 PDF/Office 文档导入，避免图片导入与文档导入混在同一个入口。

## Suggested Review Order

1. [src-tauri/src/services/import_service.rs](../../src-tauri/src/services/import_service.rs) -- 后端图片导入、重试自复制保护、共享图片资产删除回归测试。
2. [src-tauri/src/commands/import_commands.rs](../../src-tauri/src/commands/import_commands.rs) -- Tauri 导入命令按扩展名路由 PDF、Office 与图片。
3. [src-tauri/src/lib.rs](../../src-tauri/src/lib.rs) -- `import_image` 命令注册。
4. [src/app/navigation.ts](../../src/app/navigation.ts) -- 左侧导航新增“图片导入”tab，并保持在“工作台”之后。
5. [src/app/AppShell.tsx](../../src/app/AppShell.tsx) -- 注册图片导入视图标题与页面挂载。
6. [src/features/image-import/ImageImportPage.tsx](../../src/features/image-import/ImageImportPage.tsx) -- 图片导入专属页面，管理图片导入、图片文档筛选、重试、定位与删除。
7. [src/features/workbench/WorkbenchPage.tsx](../../src/features/workbench/WorkbenchPage.tsx) -- 工作台导入入口恢复为 PDF/Office 文档导入。
8. [src/lib/fileValidation.ts](../../src/lib/fileValidation.ts) -- 前端支持扩展名拆分为文档与图片两组，并提供各自的不支持类型提示。
9. [src/lib/tauriClient.ts](../../src/lib/tauriClient.ts) -- 图片导入客户端方法、图片选择对话框和批量 duplicate 结果判断。

## Verification

**Commands:**
- `npm run build` -- 通过。
- `cargo test services::import_service::tests --lib` -- 6 个导入服务测试通过。
- `cargo check` -- 通过；保留既有 unused/dead_code warning。
- `git diff --check` -- 通过。
- Playwright CLI + Tauri mock DOM 验证 -- 通过：左侧导航顺序为“工作台 > 图片导入 > 模型分析...”；点击“图片导入”后顶部标题为“图片导入”，页面显示“管理图片文档”和“选择图片”，图片 tab 只列出图片文档；回到工作台后导入入口显示“导入文档”“选择文件”和“PDF 或 Office 文档”。

**Known environment note:**
- `cargo test --lib` 仍有一个既有环境相关失败：`services::settings_service::tests::model_configuration_status_detects_missing_fields`。该测试假设当前系统没有已配置 API key；本机凭据存在时单独运行也失败，和本次图片导入改动无关。

## Review Notes

- 独立审查指出 retry 从 `originals/` 内部路径重试时可能复制文件到自身；已通过 `copy_original_file` 跳过同源同目标复制，并补 `copying_original_to_itself_is_a_noop_for_workspace_retry`。
- 独立审查指出共享图片资产路径依赖全局 `image_assets` 行；当前架构会保留仍被引用的图片文件，不递归删非空文档页目录。已补 `deleting_first_document_preserves_shared_image_for_second_document` 锁定删除首个引用后第二个文档仍可访问图片的行为。
- 独立审查指出 `tauriClient.importMultipleFiles` 会把导入前已存在的文档标为成功；已在 helper 内导入前读取现有文档并用 document id 判 duplicate。
