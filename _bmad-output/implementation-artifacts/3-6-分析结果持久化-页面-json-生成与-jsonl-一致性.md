---
title: 'Story 3.6: 分析结果持久化、页面 JSON 生成与 JSONL 一致性'
type: 'feature'
created: '2026-05-19'
status: 'in-progress'
baseline_commit: 'NO_VCS'
context:
  - '_bmad-output/planning-artifacts/epics.md#story-36'
  - '_bmad-output/planning-artifacts/prd.md#fr-006'
  - '_bmad-output/planning-artifacts/prd.md#fr-007'
  - '_bmad-output/planning-artifacts/prd.md#fr-017'
  - '_bmad-output/planning-artifacts/architecture.md'
  - '_bmad-output/implementation-artifacts/3-5-分析失败处理-单页重试与安全诊断.md'
  - '_bmad-output/implementation-artifacts/spec-2-6-jsonl-export.md'
---

# Story 3.6: 分析结果持久化、页面 JSON 生成与 JSONL 一致性

**Status:** done

## Story

As a 本地文档处理用户,  
I want 已校验的页面分析结果被持久化，并生成可审查的页面 JSON/JSONL artifact,  
So that 后续搜索索引和外部诊断能使用一致、可追踪且不泄露密钥的页面元数据。

## Acceptance Criteria

1. **Given** 页面分析结果通过 `page_analysis_v1` schema 校验  
   **When** 系统保存结果  
   **Then** 应写入 SQLite `analysis_results` 账本  
   **And** 记录包含 `page_id`、schema version、provider、model name、分析状态、结果 JSON、时间戳与可选 `error_id`

2. **Given** 同一页面被重新分析  
   **When** 新分析结果通过校验  
   **Then** `analysis_results` 的 `UNIQUE(page_id)` 应作为当前有效结果指针，新成功结果覆盖旧失败/旧成功  
   **And** 重新分析成功时更新 `analysis_id` 与 `updated_at`，不得让旧失败结果阻止新有效结果

3. **Given** 已有页面基础记录、图片资产和成功分析结果  
   **When** 应用生成页面级 JSON artifact  
   **Then** `metadata/pages.jsonl` 对应行应为完整 `page_analysis_v1` JSON  
   **And** 字段包含 `page_id`、`image_hash`、`image_path`、`source`、`analysis`、`retrieval`、`model`、`schema_version`，命名 `snake_case`，时间戳 RFC 3339

4. **Given** 页面尚未分析或仅有失败结果  
   **When** exporter 重建 `metadata/pages.jsonl`  
   **Then** 该行导出 baseline 页面记录（含 `page_id`、`document_id`、`page_number`、`image_hash`、`image_path`、状态与时间戳）  
   **And** 不得写入未校验模型输出或 secret

5. **Given** 应用更新 `metadata/pages.jsonl`  
   **When** exporter 写入  
   **Then** 从 SQLite 与图片资产路径重建  
   **And** 使用 tmp + rename 原子写入；失败不得破坏上一版可用 artifact

6. **Given** 后续 Epic 4 需要 BM25 索引输入  
   **When** 索引服务请求已校验分析  
   **Then** 可通过 repository 列出 `status = succeeded` 的 current result 并解析为 `PageAnalysisV1`（含 `retrieval.bm25_text`）  
   **And** Epic 3 不实现 BM25 构建或搜索 UI

7. **Given** 分析成功（单页或批量）  
   **When** 结果已提交到 SQLite  
   **Then** 应触发 pages JSONL 重建（失败仅记警告，不阻断分析主流程）

## Tasks / Subtasks

- [x] **强化 analysis_results 当前结果指针**（AC: 1, 2）
  - [x] `save_success_result` 在 `ON CONFLICT` 时更新 `analysis_id` 与全部 current 字段
  - [x] 新增 `list_current_succeeded_analyses` / `find_succeeded_page_analysis` 供索引准备

- [x] **实现页面 JSON/JSONL 重建**（AC: 3–5, 7）
  - [x] 新增 `artifacts/page_json_exporter.rs`：从 SQLite 组装 `PageJsonlLine`（`page_analysis_v1` 或 baseline）
  - [x] 扩展 `ArtifactExporter::export_all` 使用新 pages 行构建逻辑；保留 documents/jobs 导出
  - [x] 导出前对 JSON 字符串执行 `redact_secrets` 防线

- [x] **接入分析完成后的 artifact 刷新**（AC: 7）
  - [x] `persist_success_result` 提交后调用 exporter（warn-only）
  - [x] 批量分析结束后调用 exporter（warn-only）

- [x] **测试与回归**（AC: 1–7）
  - [x] Rust：成功分析后 pages.jsonl 含 `page_analysis_v1` 行；失败/未分析页为 baseline
  - [x] Rust：重分析成功后 JSONL 行更新且 `analysis_id` 变化
  - [x] Rust：原子写入与 redaction；`list_current_succeeded_analyses` 返回 bm25_text
  - [x] `cargo test --lib`、`cargo fmt --check`

### Review Findings

- [x] [Review][Patch] `redact_secrets` 会截断超过 800 字符的安全 JSONL 行 — 已改为结构化 `serde_json::to_string`，并加 `serialize_line_does_not_truncate_long_safe_content` 测试
- [x] [Review][Patch] 批量分析逐页全量重建 JSONL — `analyze_page_core` 增加 `refresh_jsonl_after_success`；批量路径传 `false`，仅在 `run_batch_pages` 结束导出一次
- [x] [Review][Patch] 重分析失败后 JSONL 陈旧 — `record_page_failure` 结束后 warn-only 调用 `refresh_page_jsonl_artifact`
- [x] [Review][Patch] `list_current_succeeded_analyses` 路径陈旧 — 列表查询内联 `lookup_image_path_for_page` 同步 `image_path`
- [x] [Review][Patch] 缺少原子写入测试 — 新增 `jsonl_exporter::atomic_write_replaces_target_and_leaves_no_tmp_file`
- [x] [Review][Decision] `pages.jsonl` 混用两种 JSON 形状 — **接受**（选项 1）：有 `analysis` 块为 `page_analysis_v1`，否则为 baseline；Story 2.6 消费者可按字段存在性区分

## Dev Notes

| 能力 | 当前实现 | Story 3.6 动作 |
|------|----------|----------------|
| analysis_results | `UNIQUE(page_id)` current result；`result_json` 存已校验 `PageAnalysisV1` | 冲突时刷新 `analysis_id`；新增 succeeded 列表查询 |
| pages.jsonl | 导出 `PageRecordDto` 基础字段 | 成功页导完整 `page_analysis_v1`；其余 baseline + `image_path` |
| 原子写入 | `atomic_write_jsonl` tmp+rename | 复用 |
| 分析后导出 | 仅 import 后 `export_all` | 分析成功/批量结束也触发 |

### 实现边界

- 不实现 BM25 索引、搜索 UI、HTTP API。
- 不新增 `analysis_history` 表；当前有效结果以 `analysis_results` + `analysis_id` 表达。
- 不从 JSONL 读取业务状态。

## Dev Agent Record

### Agent Model Used

Composer

### Debug Log References

### Completion Notes List

- 2026-05-19：实现 `PageJsonExporter`，成功页导出完整 `page_analysis_v1`，未分析/失败页导出 baseline（含 `image_path`）。
- 2026-05-19：`save_success_result` 冲突更新时刷新 `analysis_id`；新增 `find_succeeded_page_analysis` 与 `list_current_succeeded_analyses`。
- 2026-05-19：分析成功与批量分析（有成功页）后 warn-only 刷新 `metadata/pages.jsonl`；`cargo test --lib` 65 项全绿。
- 2026-05-19：Code review 修复 5 项 patch + 接受混合格式；`cargo test --lib` 67 项全绿。

### File List

- `src-tauri/src/artifacts/page_json_exporter.rs`
- `src-tauri/src/artifacts/jsonl_exporter.rs`
- `src-tauri/src/artifacts/mod.rs`
- `src-tauri/src/repositories/analysis_repository.rs`
- `src-tauri/src/services/analysis_service.rs`
- `_bmad-output/implementation-artifacts/3-6-分析结果持久化-页面-json-生成与-jsonl-一致性.md`
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

### Change Log

- 2026-05-19：Story 3.6 实现分析结果 JSONL artifact 导出、当前结果指针与 Epic 4 索引数据查询基础。
