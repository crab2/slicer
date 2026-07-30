---
title: 'PDF 直接结构化与图片模块解析'
type: 'feature'
created: '2026-07-30'
status: 'done'
baseline_commit: 'a2e3aea3ef4394eaa18131ca32653dab4256d687'
review_loop_iteration: 1
context: ['{project-root}/_bmad-output/planning-artifacts/architecture.md']
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** PDF/Office 导入虽已接入 OpenDataLoader，仍逐页渲染 PNG，浪费时间和磁盘；空结构页还可能退回整页模型分析。

**Approach:** 规范 PDF 直接由 ODL 提取文本、表格、bbox 和内嵌图片，不生成整页 PNG；仅非装饰性 ODL 图片调用多模态模型。直接图片和历史页图保持兼容。

## Boundaries & Constraints

**Always:** SQLite 为权威账本；PDF、ODL JSON/图片和版本可追溯；结构化页允许图片引用为空；页几何只读取不栅格化；聚合真实 `rows/cells/kids`；模型输入必须是登记且哈希匹配的 ODL 图片；新 enrichment 成功后才替换旧值；“分析新内容”排除失败图片。

**Ask First:** 启用 hybrid/OCR、重解析历史文档、删除历史页图或改变直接图片兼容行为。

**Never:** 不生成或发送 PDF 整页图；空页 fallback 不得作为图片；ODL/图片失败不得整页回退；单块失败不得删除旧 enrichment、文本或索引。

## I/O & Edge-Case Matrix

| Scenario | Input | Expected | Error Handling |
|---|---|---|---|
| 文本 PDF | 段落/表格 | 模块入库，无页图和模型调用 | 非法 ODL 输出使导入失败 |
| 图文 PDF | 多图片 | 仅提取图片调用模型 | 单块独立失败、显式重试 |
| Office | DOCX/PPTX | 转 PDF 后直接 ODL | 分阶段记录错误 |
| 空结构页 | 无块/图片源 | 页可追溯，图片路径为空 | 提示 hybrid/OCR，不回退 |
| 历史/图片 | 旧页图或 PNG/JPEG | 保持 `page_analysis_v1` | 迁移不丢数据 |

</frozen-after-approval>

## Code Map

- `src-tauri/migrations/0007_previewless_pdf_pages.sql` -- 无页图页记录迁移。
- `src-tauri/src/providers/{pdf_renderer,pdf_structure}.rs` -- PDF 元数据和 ODL 解析。
- `src-tauri/src/services/{import_service,analysis_service}.rs` -- 导入与图片分析主链路。
- `src-tauri/src/repositories/{document_repository,pdf_structure_repository}.rs` -- 页和 enrichment 账本。
- `src/features/{settings,analysis,media-management,search}` -- 无页图 UI。

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/migrations/0007_previewless_pdf_pages.sql`, `src-tauri/src/{domain,repositories}/` -- 页图片引用可空并增加 `structured` 状态；迁移保留旧行和外键。
- [x] `src-tauri/src/providers/pdf_renderer.rs`, `src-tauri/src/services/import_service.rs` -- 用页元数据替代 `render_pdf`，PDF/Office 不写整页 PNG。
- [x] `src-tauri/src/providers/pdf_structure.rs` -- 聚合真实表格层级；空页 fallback 不可分析；限制块数量。
- [x] `src-tauri/src/services/analysis_service.rs`, `src-tauri/src/repositories/pdf_structure_repository.rs` -- 校验图片哈希；移除整页 fallback；保留旧 enrichment；默认排除失败块。
- [x] `src/features/`, `README.md`, `README.en.md` -- 显示结构化无页图状态并移除 PDF DPI/整页预览承诺。
- [x] 上述 Rust/TS 文件内测试 -- 覆盖迁移、表格、空页零调用、篡改、旧结果和失败选择。

**Acceptance Criteria:**
- Given PDF/Office 导入成功, when 检查工作区, then 有 PDF 和 ODL 制品，无新整页 PNG，页图片路径为空。
- Given 文本 PDF 未配置模型, when 建索引, then 表格/文本可检索且模型调用为 0。
- Given 图文 PDF, when 分析新内容, then 仅哈希匹配的 ODL 图片调用模型，失败块只显式重试。
- Given 空页或图片缺失, when 分析, then 不发送整页图且已有文本不丢失。
- Given 成功块重分析失败, when 查询/建索引, then 上次 enrichment 仍可用。

## Spec Change Log

- 2026-07-30 loop 1：按用户确认移除整页 PNG；补充真实表格、空页、失败选择和旧 enrichment 约束。KEEP：规范 PDF、ODL JSON/bbox/图片、v1 兼容和原子索引。
- 2026-07-30 review：修复失败块重试可达性、结构化筛选、Office 并发、缺图保文、MIME/大小边界和索引链接保护。

## Verification

- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- `cargo test --manifest-path src-tauri/Cargo.toml`
- `npm.cmd run build`
- `npm.cmd run test:frontend`
- `npm.cmd run test:media-boundaries`

## Suggested Review Order

**导入与结构化**

- 从入口确认 PDF 只读页几何，不再渲染整页图片。
  [`import_service.rs:413`](../../src-tauri/src/services/import_service.rs#L413)

- ODL 输出外置图片并明确关闭 hybrid/OCR。
  [`pdf_structure.rs:232`](../../src-tauri/src/providers/pdf_structure.rs#L232)

- 真实表格按 rows、cells、kids 层级聚合文本。
  [`pdf_structure.rs:644`](../../src-tauri/src/providers/pdf_structure.rs#L644)

**模型与失败隔离**

- 模型输入只读取登记制品并复核内容哈希。
  [`analysis_service.rs:622`](../../src-tauri/src/services/analysis_service.rs#L622)

- 默认批次仅选择从未分析的视觉块。
  [`pdf_structure_repository.rs:199`](../../src-tauri/src/repositories/pdf_structure_repository.rs#L199)

- 成功提交时才原子替换旧 enrichment。
  [`pdf_structure_repository.rs:406`](../../src-tauri/src/repositories/pdf_structure_repository.rs#L406)

**存储与检索**

- 迁移允许结构化页面没有 image_hash，并保留旧外键关系。
  [`0007_previewless_pdf_pages.sql:1`](../../src-tauri/migrations/0007_previewless_pdf_pages.sql#L1)

- 文本、表格和视觉 enrichment 统一生成模块级索引项。
  [`search_service.rs:461`](../../src-tauri/src/services/search_service.rs#L461)

**交互与重试**

- 导航上下文显式区分全量重分析和失败项重试。
  [`navigation.ts:24`](../../src/app/navigation.ts#L24)

- 结构化文档提供可达的失败项重试入口和模块筛选。
  [`MediaAssetList.tsx:544`](../../src/features/media-management/components/MediaAssetList.tsx#L544)

- 多文档完成文案分别汇总页面和视觉模块。
  [`AnalysisPage.tsx:650`](../../src/features/analysis/AnalysisPage.tsx#L650)

**回归覆盖**

- 缺失 ODL 图片仍保留可检索结构文本。
  [`pdf_structure.rs:1679`](../../src-tauri/src/providers/pdf_structure.rs#L1679)

- 失败重分析保留旧 enrichment，失败重试仅处理目标块。
  [`analysis_service.rs:2942`](../../src-tauri/src/services/analysis_service.rs#L2942)

- 状态筛选覆盖无整页图的待分析和失败模块。
  [`MediaAssetList.test.ts:56`](../../src/features/media-management/components/MediaAssetList.test.ts#L56)
