---
title: 'Epic 4: BM25 检索与搜索体验'
status: 'done'
---

# Epic 4 实现摘要

## 范围（Story 4.1–4.7）

- `SearchProvider` trait + `MockSearchProvider` + `TantivyBm25SearchProvider`
- SQLite `index_versions` / `index_active` + `active.json` 指针
- `SearchService`：状态、异步重建、查询 DTO、失败不覆盖旧索引
- 中文 CJK 2-gram analyzer（`cjk_bigram_v1`）
- Tauri：`get_index_status`、`search_pages`、`start_index_rebuild`
- 前端：`SearchPage`、工作台 `IndexStatusPanel`

## 验证

- `cargo test --lib`：73 passed
- `npx tsc --noEmit`：通过

## Code Review（自动）

| 级别 | 项 | 处置 |
|------|-----|------|
| Defer | `index_rebuild` 任务未显式标记 `succeeded`（仅 progress 100） | 与分析任务一致，后续可对齐 ledger |
| Defer | `building_job_id` 未回填到 `IndexStatusDto` | 可通过 `list_jobs` 过滤，UI 已轮询状态 |
| Defer | 重建失败时 build 目录未自动清理 | 保留诊断，可后续 GC |
| OK | 原子 `active.json` + 仅 ready 后切换 | 已实现 |
| OK | 重建中仍可使用旧索引搜索 | `can_search` + `search_uses_stale_index` |
