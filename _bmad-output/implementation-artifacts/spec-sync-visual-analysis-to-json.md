---
title: '将视觉模块分析结果同步到 JSON 查看内容'
type: 'bugfix'
created: '2026-07-31'
status: 'done'
baseline_commit: '7f990b34be474e0c4d59a9c172b132b9a2c10ffb'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/spec-structured-pdf-ingestion-localized-retrieval.md'
  - '{project-root}/_bmad-output/implementation-artifacts/spec-annot-json-synchronization.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** 图片模型结果只写入 SQLite `enrichment_json`，查看器仍返回原始 ODL JSON，导致 Annot 浮层和 JSON 栏看不到解析信息。

**Approach:** JSON 查看接口校验制品后，将成功 enrichment 按导入时的稳定 block id 投影为节点的 `slicer_visual_analysis`；原始文件不变。

## Boundaries & Constraints

**Always:** SQLite 是权威来源；只投影当前文档的成功结果；关联复用 `document_id + id/path` 标识；无结果时逐字返回原文；异常项不影响原始 JSON；浮层与右栏消费同一内容。

**Ask First:** 改写 ODL 文件或哈希、新增迁移、改变导出语义，或覆盖 ODL 原生字段。

**Never:** 不按 bbox、文本或仅 source id 猜节点；不串文档或失败结果；不覆盖原字段；不绕过制品校验；不在前端另建合并逻辑。

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| 已分析 image | 有合法 enrichment | 节点新增完整 `slicer_visual_analysis` | N/A |
| 无分析结果 | 文档没有 enrichment | 返回内容与原始制品逐字一致 | N/A |
| enrichment 异常 | 非法、孤立或跨文档 | 跳过异常项 | 原始 JSON 仍可查看 |
| 制品异常 | 缺失、越界、超限、非法或哈希不符 | 不投影 | 沿用查看错误 |

</frozen-after-approval>

## Code Map

- `src-tauri/src/providers/pdf_structure.rs` -- 导入与查看共用稳定 block id。
- `src-tauri/src/repositories/document_viewer_repository.rs` -- 查询文档视觉 enrichment。
- `src-tauri/src/services/document_viewer_service.rs` -- 生成 JSON 投影并承载测试。
- `src/components/document-viewer/DocumentFormatViewer.test.ts` -- 验证浮层与右栏使用的对象保留模型结果。
- `src/features/media-management/MediaManagementPage.tsx` -- 列表/任务刷新时让已打开查看器重新读取 enrichment。

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/src/providers/pdf_structure.rs` -- 提取 crate 内稳定 id helper。
- [x] `src-tauri/src/repositories/document_viewer_repository.rs` -- 按文档读取成功 enrichment。
- [x] `src-tauri/src/services/document_viewer_service.rs` -- 仅对 `json` 投影并保留无结果快路径；测试映射、嵌套、异常、隔离及文件不可变。
- [x] `src/components/document-viewer/DocumentFormatViewer.test.ts` -- 验证联动 JSON 对象保留完整模型结果。
- [x] `src/features/media-management/MediaManagementPage.tsx` -- 媒体页重新激活或手动刷新后使查看器缓存失效。

**Acceptance Criteria:**
- Given image 分析成功, when 重新打开 Annot/JSON, then 浮层和右栏显示同一份含描述、可见文字、关键词和模型信息的 `slicer_visual_analysis`。
- Given ODL JSON 已登记哈希, when 投影, then 文件和哈希不变且读取仍先校验。
- Given 有嵌套、重复 source id 或其他文档结果, when 投影, then 只更新稳定 block id 对应节点。

## Spec Change Log

## Verification

**Commands:**
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- `cargo test --manifest-path src-tauri/Cargo.toml document_viewer`
- `cargo test --manifest-path src-tauri/Cargo.toml pdf_structure`
- `npm.cmd run test:frontend -- DocumentFormatViewer`
- `git diff --check`

## Suggested Review Order

**安全投影入口**

- 原始制品校验后才查询并物化当前文档 enrichment。
  [`document_viewer_service.rs:106`](../../src-tauri/src/services/document_viewer_service.rs#L106)

- 查询锁定最新成功 parse run，并限制类型、单项与总量预算。
  [`document_viewer_repository.rs:118`](../../src-tauri/src/repositories/document_viewer_repository.rs#L118)

- 重新校验账本结果，越界或无法关联时原样返回源 JSON。
  [`document_viewer_service.rs:275`](../../src-tauri/src/services/document_viewer_service.rs#L275)

**稳定身份与刷新**

- 导入和查看共用 `document_id + id/path` block id 算法。
  [`pdf_structure.rs:793`](../../src-tauri/src/providers/pdf_structure.rs#L793)

- 媒体页刷新同步失效查看器缓存，重新读取最新 enrichment。
  [`MediaManagementPage.tsx:111`](../../src/features/media-management/MediaManagementPage.tsx#L111)

- Annot 浮层和右栏从同一投影 JSON 生成对象路径。
  [`DocumentFormatViewer.tsx:195`](../../src/components/document-viewer/DocumentFormatViewer.tsx#L195)

**回归验证**

- 后端覆盖嵌套、跨文档、坏类型、命名冲突和文件不可变。
  [`document_viewer_service.rs:738`](../../src-tauri/src/services/document_viewer_service.rs#L738)

- 前端确认格式化与定位范围完整保留模型分析对象。
  [`DocumentFormatViewer.test.ts:23`](../../src/components/document-viewer/DocumentFormatViewer.test.ts#L23)
