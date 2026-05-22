# Epic 2 Context: 文档导入与页面图片生成流水线

<!-- Compiled from planning artifacts. Edit freely. Regenerate with compile-epic-context if planning docs change. -->

## Goal

用户可以拖拽或选择 PDF、PPT、PPTX、DOC、DOCX，系统识别重复和不支持文件，将原文件纳入工作区，并把文档可恢复地转换为逐页 PNG 页面资产，在工作台中看到任务进度、失败原因和重试入口。

## Stories

- Story 2.1: PDF 导入到页面 PNG 的端到端纵切片
- Story 2.2: 多文件导入、文件类型校验与重复识别
- Story 2.3: Office 文档转换为中间 PDF 并接入页面渲染
- Story 2.4: 页面图片资产写入、内容哈希命名与冲突保护
- Story 2.5: 导入任务进度、失败恢复与重试能力
- Story 2.6: 页面、文档与任务 JSONL artifact 导出和一致性校验
- Story 2.7: 工作台导入体验完善：任务列表、失败原因和处理入口

## Requirements & Constraints

- FR2: 多文件导入、重复识别、类型拒绝和 originals 登记
- FR3: PDF/Office 转换与逐页 PNG 渲染
- FR4: 页面图片内容哈希命名与冲突避免
- FR7: SQLite 与 JSONL 页面、任务、文档记录一致性
- FR14: 工作台文件导入、任务列表、状态、失败原因和处理入口
- 原始文件复制到 `originals/`，计算内容哈希，识别重复
- PDF 逐页渲染为 PNG，每页创建 page_records 和 image_assets
- Office 文档通过 LibreOffice headless 转 PDF 后再渲染
- `page_id` 是文档页面 occurrence identity，`image_hash` 是图片内容 identity
- 页面图片使用内容哈希命名，原子写入（tmp + rename）
- JSONL 从 SQLite 重建，不作为主状态源
- 所有长任务通过 Job Orchestrator，Tauri command 不直接执行
- LibreOffice 缺失或转换失败必须是可恢复错误
- 中间 PDF 默认删除，失败时保留
- 中文路径/文件名必须可用
- 30 页 PPTX 应能完成转换，300 页 PDF 不卡死 GUI

## Technical Decisions

- SQLite 是权威账本，文件和 JSONL 是 artifact
- `page_id` 与 `image_hash` 必须分离：`page_id` = `document_id + page_number`，`image_hash` = PNG 内容哈希
- 页面图片路径：`pages/<document_id>/<image_hash>.png`
- 原文件路径：`originals/<hash>.<ext>` 或 `<hash>_<original_name>`
- 文档状态枚举扩展：`importing`, `converted`, `conversion_failed` 等
- 页面状态枚举扩展：`image_created` 等
- `providers/` 负责 PDF 渲染和 LibreOffice 调用，通过 trait 定义边界
- `artifacts/` 负责 workspace 文件路径、临时文件、原子写入
- JSONL artifact 通过 atomic write 生成
- 前端通过 `tauriClient` 调用，不直接处理文件

## Cross-Story Dependencies

- 2.1 建立 PDF 导入纵切片，2.2 扩展为多文件，2.3 添加 Office 转换
- 2.4 强化图片资产写入安全，2.5 添加进度/重试，2.6 添加 JSONL 导出
- 2.7 完善工作台体验
- Epic 2 依赖 Epic 1 的 SQLite、Job Orchestrator、错误模型和工作区基础
- Epic 3 依赖 Epic 2 的页面记录和图片资产
- Epic 4 依赖 Epic 3 的分析结果
