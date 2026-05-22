---
title: 'Story 5.6: API Contract Tests, Settings Visibility & External Access Finalization'
type: 'feature'
created: '2026-05-20'
status: 'done'
baseline_commit: 'b901e8f'
context: []
---

# Story 5.6: API Contract Tests, Settings Visibility & External Access Finalization

**Status:** done

## Story

As a 本地自动化用户,
I want slicer 的 localhost API 端点有合约测试覆盖，设置页清晰展示 API 状态和端点信息,
so that 我可以可靠地从本地工具调用 API 并了解哪些端点需要 token。

## Scope Boundaries

**In scope (本故事必须完成):**

- API contract 集成测试覆盖全部 5 个端点。
- 127.0.0.1 绑定验证已有测试覆盖（`rejects_non_loopback_bind_address`）。
- Settings 页面启用 token reset 按钮，显示截断 token。
- Settings 页面添加端点摘要表格。

**Out of scope:**

- ❌ 独立的 API 使用文档（端点摘要已内嵌在 Settings UI 中）。

## Tasks & Acceptance

- [x] **T1: API contract 集成测试** — 7 个测试覆盖 health、search、page、document、rebuild 端点
- [x] **T2: 127.0.0.1 绑定验证测试** — 已存在于 api_server_service 单元测试
- [x] **T3: 前端 token reset** — tauriClient.resetApiToken() + 启用按钮 + 截断显示
- [x] **T4: 前端端点摘要** — ApiServerSettings 中添加 details/summary 表格
- [x] **T5: 状态更新** — sprint-status.yaml + spec 文件

## Acceptance Criteria

1. `GET /health` 返回 200 + `{ "data": { "api_version", "workspace" } }`。
2. `GET /search?q=test` 返回 200 + `{ "data": { "items", "query", "limit" } }` 或 500 错误合约。
3. `GET /pages/{id}` 不存在时返回 404/500 + `{ "error": { "code", "message", "correlation_id" } }`。
4. `GET /documents/{id}` 不存在时返回 404/500 + 错误合约。
5. `POST /indexes/rebuild` 无 token 返回 401 + `{ "error": { "code": "missing_authorization" } }`。
6. `POST /indexes/rebuild` 错误 token 返回 401 + `{ "error": { "code": "invalid_token" } }`。
7. `POST /indexes/rebuild` 有效 token 通过认证，返回 200 或 500 错误合约。
8. Settings 页面 token reset 按钮可点击，确认后调用 `reset_api_token` 并显示截断 token。
9. Settings 页面显示端点摘要表格（方法、路径、认证要求）。
10. 默认绑定 127.0.0.1，`0.0.0.0` 被拒绝（已有测试覆盖）。

## Test Results

- 105 lib 单元测试通过
- 7 integration 测试通过：
  - `health_route_returns_200_with_expected_shape`
  - `search_endpoint_returns_200_with_data_field`
  - `page_not_found_returns_error`
  - `document_not_found_returns_error`
  - `rebuild_without_token_returns_401`
  - `rebuild_with_invalid_token_returns_401`
  - `rebuild_with_valid_token_returns_success_or_error`
