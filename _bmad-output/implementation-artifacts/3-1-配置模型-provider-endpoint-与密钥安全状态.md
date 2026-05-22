---
title: 'Story 3.1: 配置模型 Provider、Endpoint 与密钥安全状态'
type: 'feature'
created: '2026-05-19'
status: 'done'
baseline_commit: 'NO_VCS'
context:
  - '_bmad-output/planning-artifacts/epics.md#story-31'
  - '_bmad-output/planning-artifacts/architecture.md'
  - '_bmad-output/implementation-artifacts/spec-1-5-error-model-diagnostics-secrets.md'
  - '_bmad-output/implementation-artifacts/spec-1-6-settings-workbench-polish.md'
---

<!-- Validation: optional — run `bmad-create-story` validate action before dev-story. -->

# Story 3.1: 配置模型 Provider、Endpoint 与密钥安全状态

**Status:** done

## Story

As a 本地文档处理用户,  
I want 在设置页配置用于页面图片分析的模型 provider、endpoint、model name 和 API key,  
So that 我可以用自己选择的云端 API 或自定义 HTTP endpoint 执行多模态分析，同时确保密钥不会泄露。

## Acceptance Criteria

1. **Given** 用户已选择可用工作区并打开设置页  
   **When** 用户配置 `model_provider`、`base_url`、`custom_endpoint`、`model_name`、`analysis_concurrency` 并保存  
   **Then** 非敏感配置应写入**工作区 SQLite `settings` 表**（key/value，`updated_at` RFC 3339）  
   **And** 重新打开应用后应恢复；DTO 与 JSON 序列化字段均为 `snake_case`

2. **Given** 用户输入或更新模型 API key  
   **When** 用户保存密钥  
   **Then** 必须通过 `keyring`（`security` 模块）写入 OS credential store  
   **And** SQLite、`app-settings.json`、JSONL artifact、前端 state、日志、`errors` 表不得含完整明文 key

3. **Given** 模型配置未完成（缺 `model_name`、缺 endpoint、或 `api_key_configured == false`）  
   **When** 用户在工作台查看分析入口  
   **Then** 分析操作应 disabled 或点击后提示需完成模型配置  
   **And** 提示含跳转设置页的入口（路由或内链）

4. **Given** 用户将使用云端模型或自定义 HTTP endpoint（`model_provider != "local_mock"` 或已填 `base_url`/`custom_endpoint`）  
   **When** 用户首次尝试启用/触发分析相关操作  
   **Then** 必须弹出可确认的隐私提示：页面图片将发往用户配置的模型服务  
   **And** 用户确认前不得标记 `privacy_notice_accepted`；未确认时不得启动真实模型调用（本 story 无真实调用，仅持久化确认状态）

5. **Given** 未来 AnalysisService 需要密钥  
   **When** Rust 侧构建模型请求  
   **Then** 仅 `security::read_api_key()`（或 `secret_store` 封装）可读明文 key  
   **And** Tauri command、前端、`settings_repository`、普通 repository 不得返回或记录明文 key

6. **Given** 配置保存、密钥读写失败  
   **When** 用户保存设置或密钥  
   **Then** 返回统一 `AppError`（中文 message、`correlation_id`、`snake_case`）  
   **And** `details` 与 tracing 日志经 `redact_secrets` 处理，不得含 API key / Authorization header

## Tasks / Subtasks

- [x] **工作区 SQLite settings 持久化**（AC: 1）
  - [x] 在 `ledger_repository` 或新建 `workspace_settings_repository` 实现 `settings` 表 CRUD（`key`, `value`, `updated_at`）
  - [x] 将 `model_provider`, `base_url`, `custom_endpoint`, `model_name`, `default_image_dpi`, `conversion_concurrency`, `analysis_concurrency`, `api_enabled`, `api_bind_address`, `api_port`, `privacy_notice_accepted` 存为 JSON 或分列 key
  - [x] **迁移**：首次打开工作区时，若 SQLite 无记录且存在全局 `app-settings.json`，导入非敏感字段后可选保留 JSON 只读备份（勿双写长期）
  - [x] `SettingsService::get_settings` / `save_settings` 改为读写**当前工作区** DB，而非仅 `config_dir/app-settings.json`
  - [x] `libreoffice_path` 可继续全局 JSON 或一并迁入 workspace — 优先与现有 `get_libreoffice_path` 行为一致，避免破坏 Epic 2 导入

- [x] **模型配置完备性 API**（AC: 3, 5）
  - [x] `SettingsService::is_model_configuration_complete(&self) -> AppResult<bool>` — 校验 provider、model_name、endpoint（`base_url` 或 `custom_endpoint` 至少其一非空）、`has_api_key()`
  - [x] 新增 Tauri command `get_model_configuration_status` → `{ configured: bool, missing: string[] }`（不含密钥）
  - [x] 禁止新增返回明文 key 的 command

- [x] **隐私确认状态**（AC: 4）
  - [x] SQLite 键 `privacy_notice_accepted`（bool，默认 false）
  - [x] Tauri command `accept_privacy_notice` / `get_privacy_notice_status`
  - [x] 新建 `src/features/settings/components/PrivacyNotice.tsx`（或 `common/`）— 模态确认文案（中文），确认后调用 command
  - [x] 持久化确认；重启后仍为 true（除非用户重置设置）

- [x] **设置页收口**（AC: 1, 2, 6）
  - [x] `SettingsPage.tsx`：`model_provider` 改为受控 select（至少 `custom`、后续可扩展；保留输入扩展点）
  - [x] 保存前校验：`analysis_concurrency`/`conversion_concurrency` 1–8，`default_image_dpi` 72–300
  - [x] 保存失败展示 `ErrorMessage` + `correlation_id`（已有模式）
  - [x] API key 输入框：`type="password"`，保存后清空；仅显示 `api_key_configured`

- [x] **工作台分析入口门禁**（AC: 3, 4）
  - [x] `WorkbenchPage.tsx` 增加「页面分析」区域（占位按钮即可，不调用模型）
  - [x] 加载 `get_model_configuration_status`；未配置 → disabled + 说明 + 链到设置
  - [x] 已配置但未 `privacy_notice_accepted` → 点击先弹 `PrivacyNotice`，确认后才 enabled（或显示「已就绪，待 Story 3.3 接入」）
  - [x] **不要**在本 story 实现 `AnalysisService` 或 HTTP 模型请求

- [x] **测试**（AC: 1–6）
  - [x] Rust：`settings` 表读写 round-trip；`is_model_configuration_complete` 边界；keyring 与 settings 分离（mock keyring 或集成测试标记 `#[ignore]` on CI）
  - [x] Rust：`redact_secrets` 对含 fake api_key 的 details 仍通过（回归）
  - [x] 现有 `cargo test --lib`、`npx tsc --noEmit`、`npx vite build` 通过

### Review Findings

- [x] [Review][Patch] 拒绝空白 API key，否则模型配置会被误判为已完成 [`src-tauri/src/security/mod.rs`:7]
- [x] [Review][Patch] 工作区 settings JSON 损坏时错误文案声称已恢复默认值但实际会阻断设置页 [`src-tauri/src/repositories/workspace_settings_repository.rs`:20]
- [x] [Review][Patch] `local_mock` 填写远端 endpoint 时会跳过隐私确认，违反 AC4 的 OR 条件 [`src-tauri/src/services/settings_service.rs`:169]
- [x] [Review][Patch] `accept_privacy_notice` 可在模型未配置或无需提示时直接写入确认状态 [`src-tauri/src/services/settings_service.rs`:115]
- [x] [Review][Patch] 保存设置仍会把完整 `AppSettingsDto` 写回全局 `app-settings.json`，形成与工作区 SQLite 权威源冲突的长期残留 [`src-tauri/src/services/settings_service.rs`:56]
- [x] [Review][Patch] 服务端缺少 `api_port` 与 `api_bind_address` 校验，后续启用 API 时可能保存非 localhost 或非法端口 [`src-tauri/src/services/settings_service.rs`:193]
- [x] [Review][Patch] 切换工作区时未关闭隐私弹窗或清空分析就绪文案，可能把上一工作区的确认写入当前工作区 [`src/features/workbench/WorkbenchPage.tsx`:204]
- [x] [Review][Patch] Story 标记 keyring/settings 分离测试已完成，但缺少断言密钥不会进入 SQLite/JSON 的回归测试 [`src-tauri/src/services/settings_service.rs`:222]

## Dev Notes

### 当前代码基线（必读，避免重复造轮子）

| 能力 | 现状 | 本 story 动作 |
|------|------|----------------|
| `AppSettingsDto` | `src-tauri/src/domain/settings.rs` 已含模型字段 | 扩展 `privacy_notice_accepted`（可选放 DTO 或独立 status） |
| keyring | `src-tauri/src/security/mod.rs` 完整 | **复用**，勿新造 secret 层 |
| 设置 UI | `src/features/settings/SettingsPage.tsx` 已有表单 + API key | **增强**校验与 provider select |
| 非敏感持久化 | `settings_repository.rs` → 全局 `app-settings.json` | **迁到工作区 SQLite**（与 AC/架构对齐） |
| SQLite `settings` 表 | `migrations/0001_initial.sql` 已建表 | **开始实际使用** |
| 工作台分析 | 无分析按钮 | **新增门禁占位** |
| 隐私弹窗 | 设置页仅静态说明 | **实现可确认模态 + 持久化** |

### 架构合规（必须遵守）

- 业务逻辑在 `services/`，SQLite 仅在 `repositories/`，密钥仅在 `security/`。[Source: `architecture.md` — Project Structure]
- `ModelProvider` trait 与真实 HTTP 调用属于 Story 3.2/3.3 — **本 story 不创建 `providers/model/` 实现**。
- 字段命名 `snake_case`；时间戳 RFC 3339；错误用 `AppError`。[Source: `architecture.md` — Naming & Format Patterns]
- 日志禁止明文 key；使用现有 `tracing` + `redact_secrets`。[Source: `spec-1-5-error-model-diagnostics-secrets.md`]

### References

- [Source: `_bmad-output/planning-artifacts/epics.md` — Epic 3, Story 3.1]
- [Source: `src-tauri/src/security/mod.rs`]

## Dev Agent Record

### Agent Model Used

Composer

### Debug Log References

- 修复并行测试下临时目录冲突：`test_workspace` 使用纳秒后缀保证唯一路径。

### Completion Notes List

- 新增 `WorkspaceSettingsRepository`：工作区 SQLite `settings` 表读写、`app_settings` JSON blob、`privacy_notice_accepted` 键。
- `SettingsService`：工作区设置持久化、从全局 `app-settings.json` 一次性迁移、保存校验、`get_model_configuration_status` / 隐私确认 API；`libreoffice_path` 仍存全局 JSON。
- 前端：`PrivacyNotice` 模态、设置页 provider select + 校验、工作台分析占位与配置/隐私门禁。
- 验证：`cargo test --lib`（20 passed）、`npx tsc --noEmit`、`npx vite build` 通过。

### File List

- `src-tauri/src/repositories/workspace_settings_repository.rs` (new)
- `src-tauri/src/repositories/mod.rs`
- `src-tauri/src/domain/settings.rs`
- `src-tauri/src/services/settings_service.rs`
- `src-tauri/src/commands/settings_commands.rs`
- `src-tauri/src/lib.rs`
- `src/features/settings/components/PrivacyNotice.tsx` (new)
- `src/features/settings/SettingsPage.tsx`
- `src/features/workbench/WorkbenchPage.tsx`
- `src/app/AppShell.tsx`
- `src/lib/tauriClient.ts`
- `src/types/app.ts`
- `src/styles/globals.css`

## Change Log

- 2026-05-19: Story 3.1 实现 — 工作区 SQLite 模型设置、密钥边界、隐私确认与工作台分析门禁占位。
