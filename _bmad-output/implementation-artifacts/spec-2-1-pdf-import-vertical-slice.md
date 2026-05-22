---
title: 'Story 2.1: PDF 导入到页面 PNG 的端到端纵切片'
type: 'feature'
created: '2026-05-18'
status: 'done'
baseline_commit: 'NO_VCS'
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** 工作台目前只有占位"导入文件"面板，没有真实的文档导入能力。需要建立从 PDF 文件到逐页 PNG 的端到端纵切片，验证 slicer 的核心文档处理流水线。

**Approach:** 新增 SQLite migration 创建 documents/page_records/image_assets 表，实现 PDF 渲染 provider trait + pdfium 实现，建立 import service 编排哈希/复制/渲染/入库流程，通过 Job Orchestrator 报告进度，前端在工作台添加文件选择入口并展示导入结果。

## Boundaries & Constraints

**Always:**
- `page_id` 是文档页面 occurrence identity（document_id + page_number）
- `image_hash` 是 PNG 图片内容哈希，用作文件名
- 页面图片路径：`pages/<document_id>/<image_hash>.png`
- 原文件复制到 `originals/<hash>_<sanitized_name>.pdf`
- 所有长任务通过 Job Orchestrator，Tauri command 只创建任务并返回
- SQLite 是权威账本，文件是 artifact
- 中文路径/文件名必须可用
- 原子写入：tmp 文件 + rename

**Ask First:**
- PDF 渲染库选择（pdfium-render vs 其他）
- 单页/多页 PDF 测试夹具来源

**Never:**
- 不实现 Office 转换、多文件批量导入、模型分析、BM25 索引或 localhost API
- 不把 page_id 和 image_hash 混为一谈
- 前端不得拼接内部路径
- Tauri command 不得直接执行 PDF 渲染

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| 单页 PDF 导入 | 1 页 PDF 文件 | 1 个 document 记录，1 个 page_record，1 个 PNG 文件 | N/A |
| 多页 PDF 导入 | 3 页 PDF 文件 | 1 个 document，3 个 page_records，3 个 PNG 文件 | N/A |
| 中文文件名 PDF | `测试 文档.pdf` | 原文件安全复制，页面正常渲染 | 文件名 sanitize |
| 损坏 PDF | 无效 PDF 内容 | 任务 failed，结构化错误，无半成品资产 | 清理 tmp 文件 |
| 加密 PDF | 需要密码的 PDF | 任务 failed，中文错误提示 | 记录 correlation_id |
| 重复导入同一 PDF | 已存在的文件哈希 | 检测到重复，返回已有 document 信息 | 不重复创建页面 |

</frozen-after-approval>

## Code Map

- `src-tauri/migrations/0003_documents_pages_images.sql` -- 新 migration：documents、page_records、image_assets 表
- `src-tauri/src/domain/document.rs` -- 扩展 DocumentDto
- `src-tauri/src/domain/page.rs` -- 扩展 PageRecordDto、ImageAssetDto
- `src-tauri/src/providers/mod.rs` -- 新模块：provider 边界
- `src-tauri/src/providers/pdf_renderer.rs` -- PDF 渲染 trait + pdfium 实现
- `src-tauri/src/services/import_service.rs` -- 导入编排：哈希/复制/渲染/入库
- `src-tauri/src/repositories/document_repository.rs` -- 文档/页面/图片 SQLite 读写
- `src-tauri/src/repositories/ledger_repository.rs` -- 扩展：document/page 查询
- `src-tauri/src/commands/import_commands.rs` -- 新 Tauri 命令：import_pdf
- `src-tauri/src/lib.rs` -- 注册新命令和模块
- `src-tauri/Cargo.toml` -- 添加 pdfium-render 依赖
- `src/lib/tauriClient.ts` -- 添加 importPdf、listDocuments 方法
- `src/types/app.ts` -- 添加 DocumentDto、PageRecordDto 类型
- `src/features/workbench/WorkbenchPage.tsx` -- 替换"导入文件"占位为真实入口
- `src/features/workbench/components/DocumentList.tsx` -- 新组件：文档/页面列表

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/Cargo.toml` -- 添加 pdfium-render 依赖
- [x] `src-tauri/migrations/0003_documents_pages_images.sql` -- 创建 documents、page_records、image_assets 表
- [x] `src-tauri/src/domain/document.rs` -- 添加 DocumentDto 结构体
- [x] `src-tauri/src/domain/page.rs` -- 添加 PageRecordDto、ImageAssetDto 结构体
- [x] `src-tauri/src/providers/mod.rs` -- 新建 provider 模块
- [x] `src-tauri/src/providers/pdf_renderer.rs` -- 定义 PdfRenderer trait，实现 pdfium 渲染
- [x] `src-tauri/src/repositories/document_repository.rs` -- 文档/页面/图片 CRUD
- [x] `src-tauri/src/services/import_service.rs` -- 编排：计算哈希、复制原文件、渲染页面、写入账本
- [x] `src-tauri/src/commands/import_commands.rs` -- import_pdf Tauri 命令
- [x] `src-tauri/src/lib.rs` -- 注册 import_pdf 命令和 providers 模块
- [x] `src/lib/tauriClient.ts` -- 添加 importPdf、listDocuments、listPages 方法
- [x] `src/types/app.ts` -- 添加 DocumentDto、PageRecordDto TypeScript 类型
- [x] `src/features/workbench/WorkbenchPage.tsx` -- 添加文件选择入口，调用 importPdf
- [x] `src/features/workbench/components/DocumentList.tsx` -- 展示导入文档和页面状态

**Acceptance Criteria:**
- Given 用户已选择可用工作区, when 用户在工作台选择一个 PDF 文件, then 应用创建导入任务并立即返回任务状态
- Given PDF 导入任务开始, when 系统处理文件, then 应计算原文件哈希、复制到 originals/、创建 document 记录
- Given PDF 已登记, when 系统渲染页面, then 每页生成 PNG 并创建 page_record 和 image_asset 记录
- Given 页面图片写入成功, when 系统生成身份字段, then page_id != image_hash，二者语义分离
- Given 导入成功完成, when 用户查看工作台, then 应看到文档名称、页数、成功状态和更新时间
- Given PDF 损坏或加密, when 导入失败, then 任务标记 failed，关联结构化错误，无半成品资产
- Given 开发者验证, when 检查代码边界, then PDF 渲染通过 provider trait，import service 通过 orchestrator 报告进度

## Verification

**Commands:**
- `cargo test --lib` -- expected: 15 测试通过
- `npx tsc --noEmit` -- expected: TypeScript 通过
- `npx vite build` -- expected: Vite 构建通过

## Suggested Review Order

1. `src-tauri/migrations/0003_documents_pages_images.sql` -- 数据模型是基础，先确认表结构和约束
2. `src-tauri/src/domain/document.rs` + `src-tauri/src/domain/page.rs` -- DTO 与表结构对齐
3. `src-tauri/src/providers/pdf_renderer.rs` -- PdfRenderer trait 边界和 pdfium 实现
4. `src-tauri/src/repositories/document_repository.rs` -- SQLite CRUD 操作
5. `src-tauri/src/services/import_service.rs` -- 核心编排逻辑、错误处理和清理
6. `src-tauri/src/commands/import_commands.rs` -- Tauri 命令层
7. `src/types/app.ts` + `src/lib/tauriClient.ts` -- 前端类型和客户端方法
8. `src/features/workbench/WorkbenchPage.tsx` + `DocumentList.tsx` -- 前端集成
