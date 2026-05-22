---
title: 'Story 1.6: 完成设置页基础体验与工作台空状态'
type: 'feature'
created: '2026-05-18'
status: 'done'
baseline_commit: '4fa1346'
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** 设置页目前是纯静态占位，不从后端加载真实配置，也不能编辑和保存。工作台的空状态在有工作区但无文档时缺少区分。需要将设置页接入后端数据并完善工作台空状态。

**Approach:** 扩展 settings_service 和 settings_repository 支持完整的 settings 读写（SQLite），前端设置页改为表单式布局，支持 LibreOffice 路径、模型配置、并发设置的编辑与保存，API key 通过 keyring 管理。工作台根据工作区/文档/任务状态显示不同的空状态。

## Boundaries & Constraints

**Always:**
- 非敏感配置保存到 SQLite settings 表
- API key 通过 keyring 保存，不回显完整明文
- React 只持有表单草稿、加载状态等视图状态
- 持久状态来源必须是 Rust service + SQLite
- 中文 UI 文案，桌面工具风格

**Ask First:**
- 设置表单布局细节

**Never:**
- 不实现真实模型调用、Office 转换或 HTTP API 启动
- 不在前端保存工作区、settings、job 等持久状态到 React 内存
- 不使用营销式 hero 或过度装饰

</frozen-after-approval>

## Code Map

- `src-tauri/src/domain/settings.rs` -- AppSettingsDto 定义
- `src-tauri/src/repositories/settings_repository.rs` -- BootstrapSettings 读写
- `src-tauri/src/services/settings_service.rs` -- SettingsService (get/save)
- `src-tauri/src/commands/settings_commands.rs` -- Tauri commands
- `src/features/settings/SettingsPage.tsx` -- 设置页组件
- `src/features/settings/components/WorkspaceSettings.tsx` -- 工作区设置
- `src/features/workbench/WorkbenchPage.tsx` -- 工作台页
- `src/components/common/ErrorMessage.tsx` -- 错误展示
- `src/lib/tauriClient.ts` -- Tauri 客户端
- `src/types/app.ts` -- 类型定义

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/src/repositories/settings_repository.rs` -- 扩展 settings 读写：支持完整 AppSettingsDto 持久化到 JSON 配置文件
- [x] `src-tauri/src/services/settings_service.rs` -- 实现 load_settings 和 save_settings 方法
- [x] `src-tauri/src/commands/settings_commands.rs` -- 添加 save_app_settings Tauri 命令
- [x] `src-tauri/src/lib.rs` -- 注册新命令
- [x] `src/features/settings/SettingsPage.tsx` -- 改为表单式布局，加载/编辑/保存设置
- [x] `src/features/settings/components/WorkspaceSettings.tsx` -- 保持现有功能
- [x] `src/features/workbench/WorkbenchPage.tsx` -- 完善空状态：有工作区无文档时显示导入引导
- [x] `src/lib/tauriClient.ts` -- 添加 saveAppSettings 方法

**Acceptance Criteria:**
- Given 用户打开设置页, when 加载完成, then 应显示当前工作区路径、LibreOffice 路径、模型配置状态、并发设置和隐私提示
- Given 用户编辑非敏感设置并保存, when 保存成功, then 配置应写入本地账本，重启后可恢复
- Given 用户输入 API key 并保存, when 保存成功, then 应通过 keyring 保存，设置页只显示已配置状态
- Given 用户尚未选择工作区, when 进入工作台, then 应显示中文空状态和选择工作区入口
- Given 用户已选择工作区但无文档, when 进入工作台, then 应显示工作区可用状态和导入引导
- Given 工作区/设置/日志发生错误, when 前端展示, then 使用统一 ErrorMessage 组件显示中文摘要和 correlation_id

## Verification

**Commands:**
- `cargo test --lib` -- expected: 所有测试通过
- `npx tsc --noEmit` -- expected: TypeScript 通过
- `npx vite build` -- expected: Vite 构建通过

## Suggested Review Order

1. `src-tauri/src/repositories/settings_repository.rs` -- load_app_settings / save_app_settings 原子写入
2. `src-tauri/src/services/settings_service.rs` -- get_settings / save_settings / API key 管理
3. `src-tauri/src/commands/settings_commands.rs` -- save_app_settings / save_api_key / delete_api_key
4. `src/features/settings/SettingsPage.tsx` -- 表单式设置页，NaN 验证，correlation_id 错误展示
5. `src/features/workbench/WorkbenchPage.tsx` -- 空状态区分与 correlation_id 错误展示
6. `src/features/workbench/components/JobList.tsx` -- 任务列表与 correlation_id 错误传递
7. `src/components/common/ErrorMessage.tsx` -- correlation_id 展示
8. `src/styles/globals.css` -- .api-key-field, .settings-actions, .error-correlation
