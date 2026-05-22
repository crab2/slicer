---
title: 'Story 5.4: GET /search、GET /pages/{page_id}、GET /documents/{document_id} 读取接口'
type: 'feature'
created: '2026-05-20'
status: 'done'
baseline_commit: 'b901e8f'
context: []
---

# Story 5.4: GET /search、GET /pages/{page_id}、GET /documents/{document_id} 读取接口

**Status:** review

## Story

As a 本地自动化用户,
I want 通过 HTTP API 查询搜索结果、页面详情和文档详情,
so that 我的脚本可以程序化访问 slicer 中的数据。

## Scope Boundaries

**In scope (本故事必须完成):**

- `GET /search?q={query}&limit={n}` — 调用 `SearchService::search`，返回 `SearchResponseDto`。
- `GET /pages/{page_id}` — 调用 `DocumentRepository::find_page_by_id`，返回 `PageRecordDto` 或 404。
- `GET /documents/{document_id}` — 调用 `DocumentRepository::find_document_by_id`，返回 `DocumentDto` 或 404。
- 所有 handler 使用 `spawn_blocking` 包裹同步 DB/索引操作。
- 404 响应使用 `AppError` → `not_found` 映射。

**Out of scope:**

- ❌ `POST /indexes/rebuild`（留给 5.5）。
- ❌ token 认证（留给 5.5）。
- ❌ API contract tests（留给 5.6）。

## Tasks & Acceptance

- [x] **T1: 新建 api/endpoints.rs** — 三个 handler 函数
- [x] **T2: 更新 api/mod.rs** — 导出 endpoints 模块
- [x] **T3: 更新 api/server.rs** — 注册三个路由
- [x] **T4: cargo build + cargo test** — 106 tests pass

## Acceptance Criteria

1. `GET /search?q=test` 返回 200 + `{ "data": { "items": [...], "query": "test", "limit": 20 } }`。
2. `GET /pages/{page_id}` 存在时返回 200 + `{ "data": { "page_id", "document_id", ... } }`。
3. `GET /pages/{page_id}` 不存在时返回 404 + `{ "error": { "code": "page_not_found", ... } }`。
4. `GET /documents/{document_id}` 存在时返回 200 + `{ "data": { "document_id", ... } }`。
5. `GET /documents/{document_id}` 不存在时返回 404 + `{ "error": { "code": "document_not_found", ... } }`。
6. 所有 handler 使用 `spawn_blocking` 避免阻塞 tokio runtime。
