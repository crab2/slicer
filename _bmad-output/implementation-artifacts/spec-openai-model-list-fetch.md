---
title: 'OpenAI 模型列表获取按钮'
type: 'feature'
created: '2026-06-09'
status: 'done'
route: 'one-shot'
baseline_commit: '1d513dc11a610f82a1dd351764460b00b8ef6281'
context: []
---

# OpenAI 模型列表获取按钮

## Intent

**Problem:** 设置页的模型配置需要用户手动填写 `Model Name`，无法从当前 OpenAI 配置中直接获取可用模型。

**Approach:** 仅在 Provider 为 OpenAI 时，在 `Model Name` 行显示“获取模型”按钮。按钮调用后端 Tauri 命令，由后端使用当前表单的 Base URL/自定义 Endpoint 和系统密钥存储中的 OpenAI 活跃 API Key 请求 OpenAI 兼容的 `/models` 接口；前端将返回结果作为 `datalist` 候选，保留手动输入能力。如果 `Model Name` 为空，获取成功后自动填入第一个模型。

## Suggested Review Order

1. [src-tauri/src/providers/model/openai_provider.rs](../../src-tauri/src/providers/model/openai_provider.rs) -- OpenAI `/models` endpoint 推导、请求头、HTTP 错误诊断、响应解析和单测。
2. [src-tauri/src/domain/settings.rs](../../src-tauri/src/domain/settings.rs) -- `ModelInfoDto` 与 `ModelListDto`，包含模型显示名。
3. [src-tauri/src/security/mod.rs](../../src-tauri/src/security/mod.rs) -- 保存/读取 API Key 时规范化 `Authorization:` 与 `Bearer` 前缀。
4. [src-tauri/src/services/settings_service.rs](../../src-tauri/src/services/settings_service.rs) -- 从当前工作区 API Key 列表读取 OpenAI active key，并同步 provider keyring 槽位。
5. [src-tauri/src/commands/settings_commands.rs](../../src-tauri/src/commands/settings_commands.rs) -- `list_openai_models` Tauri 命令使用工作区 active key。
6. [src-tauri/src/lib.rs](../../src-tauri/src/lib.rs) -- 命令注册。
7. [src/types/app.ts](../../src/types/app.ts) -- 前端模型列表 DTO 类型。
8. [src/lib/tauriClient.ts](../../src/lib/tauriClient.ts) -- `listOpenAIModels` client 方法。
9. [src/features/settings/SettingsPage.tsx](../../src/features/settings/SettingsPage.tsx) -- OpenAI-only 获取按钮、候选列表、加载/成功消息，并优先展示 `display_name`。
10. [src/styles/globals.css](../../src/styles/globals.css) -- Model Name 输入框与按钮的响应式布局。

## Verification

**Commands:**
- `npm run build` -- 通过。
- `cargo test providers::model::openai_provider::tests --lib` -- 9 个 OpenAI provider 测试通过。
- `cargo test security::tests --lib` -- 3 个安全层 key 规范化/映射测试通过。
- `cargo test services::settings_service::tests::workspace_active_api_key_read_normalizes_bearer_prefix --lib` -- 通过。
- `cargo check` -- 通过；保留既有 unused/dead_code warning。
- `git diff --check` -- 通过；仅有 Windows 行尾提示。
- `Invoke-WebRequest`/`curl.exe` 直连 `https://www.su8.codes/codex/v1/models` -- 未带授权时稳定返回 HTTP 401 `Invalid API key`，说明 endpoint 路径与网络连通性正常，失败时应优先核对应用内 OpenAI 活跃 API Key。

**UI validation:**
- Playwright CLI + Tauri mock 验证通过：Provider 为 OpenAI 时，`Model Name` 行显示“获取模型”；点击后展示“已获取 2 个 OpenAI 模型。”，并在 `Model Name` 为空时自动填入第一个模型 `gpt-4.1-mini`。

## Notes

- API Key 不暴露给前端；后端通过 `SettingsService::read_active_api_key_for_provider("openai")` 读取系统密钥存储中的活跃 key。
- `/models` endpoint 推导支持官方 OpenAI 默认地址、普通 `/v1` 代理，以及 `https://www.su8.codes/codex/v1` 这类带路径前缀的 OpenAI 兼容 Base URL。
- 非成功响应会在用户可见错误 message 中包含 HTTP 状态码，并在诊断 details 中保留已脱敏的响应片段，方便区分 401 key 问题、403 权限问题和 404 endpoint 问题。
- 401 排查后修复两个 key 一致性问题：一是保存/读取时剥离用户可能粘贴的 `Authorization: Bearer` 前缀，避免发送 `Bearer Bearer <key>`；二是 `list_openai_models` 优先使用设置页 API Key 列表中的当前 OpenAI active key，而不是只读旧 provider keyring 槽位。
