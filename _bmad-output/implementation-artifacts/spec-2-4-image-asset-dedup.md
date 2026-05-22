---
title: 'Story 2.4: 页面图片资产写入、内容哈希命名与冲突保护'
type: 'feature'
created: '2026-05-19'
status: 'done'
baseline_commit: 'NO_VCS'
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** 当前导入流程对每个页面都写入 PNG 文件并创建 image_asset 记录，即使不同文档的页面内容完全相同（相同 image_hash）。这导致磁盘上存在重复文件，且失败清理时可能误删其他文档引用的图片。

**Approach:** 在写入页面 PNG 前，先查询 image_assets 表是否已存在该 hash。若已存在，跳过文件写入，仅创建 page_record 引用已有 image_hash。失败清理时，仅删除本次导入新创建的图片文件，不影响其他文档的引用。

## Boundaries & Constraints

**Always:**
- image_hash 是 PNG 内容的 SHA-256，用作文件名和去重键
- 写入前先查 `find_image_asset_by_hash`，已存在则跳过文件写入
- 失败清理只删除本次新写入的图片，不删除其他文档引用的图片
- `image_assets.file_path` 格式：`pages/<document_id>/<image_hash>.png`
- `page_records` 通过 `image_hash` 引用 `image_assets`，不要求一一对应
- 原子写入（tmp + rename）保留

**Ask First:**
- 无

**Never:**
- 不修改 image_hash 的计算方式（SHA-256 of PNG bytes）
- 不改变 page_id 的定义（document_id + page_number）
- 不引入硬链接或符号链接（保持简单）

</frozen-after-approval>

## Code Map

- `src-tauri/src/repositories/document_repository.rs` -- 新增 `find_image_asset_by_hash` 方法
- `src-tauri/src/services/import_service.rs` -- 修改页面写入逻辑：先查后写，跳过已有；安全清理只删新文件
- `src-tauri/src/providers/pdf_renderer.rs` -- 无需修改（image_hash 计算不变）

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/src/repositories/document_repository.rs` -- 新增 `find_image_asset_by_hash(conn, hash) -> AppResult<Option<ImageAssetDto>>`
- [x] `src-tauri/src/services/import_service.rs` -- `import_pdf` 和 `import_document` 中：写入前查询已有 asset，存在则跳过文件写入；失败清理时仅删除本次新写入的文件

**Acceptance Criteria:**
- Given 两个不同文档的第 1 页内容完全相同, when 依次导入, then 磁盘上只存在一份 PNG 文件，两个 page_records 引用同一个 image_hash
- Given 导入一个 3 页 PDF, when 第 2 页写入失败, then 仅删除本次导入新创建的图片文件，不影响其他文档已引用的图片
- Given image_assets 中已存在某 hash 的记录, when 新文档引用相同 hash, then 不执行文件写入，page_record 正常创建
- Given 开发者验证, when 检查 import_service, then 页面写入前均有 `find_image_asset_by_hash` 查询

## Verification

**Commands:**
- `cargo test --lib` -- expected: 15 测试通过
- `npx tsc --noEmit` -- expected: TypeScript 通过
- `npx vite build` -- expected: Vite 构建通过

## Suggested Review Order

1. `src-tauri/src/repositories/document_repository.rs` -- `find_image_asset_by_hash` 和 `ImageAssetRow`
2. `src-tauri/src/services/import_service.rs` -- 去重逻辑（写入前查询）和 `fail_with_cleanup_safe` 安全清理
