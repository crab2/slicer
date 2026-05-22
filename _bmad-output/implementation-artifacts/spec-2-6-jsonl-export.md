---
title: 'Story 2.6: 页面、文档与任务 JSONL artifact 导出和一致性校验'
type: 'feature'
created: '2026-05-19'
status: 'done'
baseline_commit: 'NO_VCS'
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** 当前页面、文档和任务记录仅存在于 SQLite 中，没有可移植的 artifact 副本。需要将 SQLite 中的权威数据导出为 JSONL 文件，供外部工具消费和一致性校验。

**Approach:** 新增 `ArtifactExporter` 服务，在每次导入完成后将 page_records、documents、jobs 表导出为 JSONL 文件到 `metadata/` 目录。使用原子写入（tmp + rename）。JSONL 是 artifact，SQLite 是权威源。

## Boundaries & Constraints

**Always:**
- JSONL 从 SQLite 重建，不是主状态源
- 原子写入：先写 tmp 文件，再 rename
- 三个 JSONL 文件：`metadata/pages.jsonl`、`metadata/documents.jsonl`、`metadata/jobs.jsonl`
- 每行一个 JSON 对象，字段与 DTO 一致
- 导入完成后自动触发导出
- 导出失败不影响导入主流程（记录警告日志）

**Ask First:**
- 无

**Never:**
- 不从 JSONL 读取数据（只写不读）
- 不在 JSONL 中存储敏感信息（API key 等）
- 不修改已有的 DTO 结构

</frozen-after-approval>

## Code Map

- `src-tauri/src/artifacts/mod.rs` -- 注册新模块
- `src-tauri/src/artifacts/jsonl_exporter.rs` -- 新文件：`ArtifactExporter` 服务
- `src-tauri/src/services/import_service.rs` -- 导入完成后调用 exporter
- `src-tauri/src/repositories/document_repository.rs` -- 已有 `list_documents`、`list_pages_by_document`
- `src-tauri/src/repositories/ledger_repository.rs` -- 已有 `list_jobs`

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/src/artifacts/jsonl_exporter.rs` -- 实现 `ArtifactExporter`：`export_all(workspace)` 导出三个 JSONL 文件
- [x] `src-tauri/src/artifacts/mod.rs` -- 添加 `pub mod jsonl_exporter;`
- [x] `src-tauri/src/services/import_service.rs` -- `import_pdf` 和 `import_document` 成功后调用 `ArtifactExporter::export_all`

**Acceptance Criteria:**
- Given 用户成功导入一个 PDF, when 导入完成, then `metadata/pages.jsonl`、`metadata/documents.jsonl`、`metadata/jobs.jsonl` 文件存在且每行是合法 JSON
- Given 导入了 3 页的 PDF, when 查看 pages.jsonl, then 文件包含 3 行，每行有 page_id、document_id、page_number、image_hash 字段
- Given JSONL 导出失败（如磁盘满）, when 导入流程继续, then 导入仍然成功完成，失败记录为警告日志
- Given 连续导入两个文档, when 查看 JSONL 文件, then 文件包含两个文档的所有记录（累积导出，非增量）

## Verification

**Commands:**
- `cargo test --lib` -- expected: 所有测试通过
- `npx tsc --noEmit` -- expected: TypeScript 通过
- `npx vite build` -- expected: Vite 构建通过

## Suggested Review Order

**JSONL 导出核心逻辑**

- `ArtifactExporter::export_all` 查询三表数据并原子写入 JSONL
  [`jsonl_exporter.rs:17`](../../src-tauri/src/artifacts/jsonl_exporter.rs#L17)

- `atomic_write_jsonl` 通用原子写入函数（tmp + rename）
  [`jsonl_exporter.rs:37`](../../src-tauri/src/artifacts/jsonl_exporter.rs#L37)

**数据查询层**

- `list_all_pages` 新增全量页面查询方法
  [`document_repository.rs:234`](../../src-tauri/src/repositories/document_repository.rs#L234)

**导入流程集成**

- `import_pdf` 成功后调用 `ArtifactExporter::export_all`，失败仅警告
  [`import_service.rs:186`](../../src-tauri/src/services/import_service.rs#L186)

- `import_document` 成功后同样调用导出
  [`import_service.rs:413`](../../src-tauri/src/services/import_service.rs#L413)
