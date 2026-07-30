---
title: '搜索与媒体管理统一六格式文档查看器'
type: 'feature'
created: '2026-07-30'
status: 'done'
baseline_commit: '3ad4e1e34b0aecd344b76005200831150250423f'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/planning-artifacts/architecture.md'
---

<frozen-after-approval reason="human-owned intent - do not modify unless human renegotiates">

## Intent

**Problem:** 搜索命中只显示页图和模块 JSON，结构化 PDF 无整页 PNG 时预览缺失；媒体管理还暴露批量选择、重分析和逐页操作，不能直接浏览 ODL 制品。

**Approach:** 搜索与媒体管理复用双栏查看器，提供 `PDF / Annot / Preview / HTML / MD / JSON` 六个独立标签。导入时一次生成并登记查看制品，查看时按需安全读取，默认 `Annot + JSON`。

## Boundaries & Constraints

**Always:** PDF=原 PDF，Annot=ODL 标注 PDF，Preview=沙箱 HTML，后三项显示源码；搜索命中定位页码；文件须经 typed Tauri command、SQLite 登记、工作区边界和哈希校验读取，并限大小、懒加载；媒体管理移除重分析、批量选择、源文件和逐页按钮，保留筛选、查看、失败重试和删除。

**Ask First:** 主动重新解析历史导入；启用 hybrid/OCR；移除删除能力。

**Never:** 查看时运行 ODL；恢复整页 PNG；扫描工作区猜测制品；让 Preview 执行脚本或读取未登记文件；用命中模块 JSON 冒充完整文档 JSON。

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| 新 PDF/Office | ODL 成功 | 登记五类制品，六标签可用 | 输出缺失则导入失败 |
| 搜索命中 | 第 N 页模块 | 加载完整文档并定位第 N 页 | 单格式失败不清空结果 |
| 文档切换 | 连续选择 | 仅显示最后所选文档 | 丢弃过期响应 |
| 旧/非法制品 | 缺失、越界、篡改或超限 | 缺失为空态，非法则拒绝 | 不隐式重解析或回退页图 |

</frozen-after-approval>

## Code Map

- `src-tauri/src/{providers,services,repositories,commands}` -- 生成、登记和安全读取制品。
- `src/components/document-viewer/DocumentFormatViewer.tsx` -- 共享查看器。
- `src/features/{search,media-management}` -- 搜索联动与媒体精简。

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/migrations/0008_document_view_artifacts.sql`, `src-tauri/src/{providers,services,repositories}` -- 生成 `json,markdown,html,pdf` 并原子登记。
- [x] `src-tauri/src/{domain,services,commands,lib.rs}` -- 增加 viewer DTO/命令与格式、路径、哈希、大小校验。
- [x] `src/{types,lib,components/document-viewer}` -- 增加 typed client、双栏缓存、沙箱 Preview 和空/错态。
- [x] `src/features/search/SearchPage.tsx` -- 以共享查看器替换旧页图/JSON面板并传递命中页。
- [x] `src/features/media-management/` -- 移除批量/重分析/源文件/逐页操作，接入查看器并保留重试、删除、筛选。
- [x] Rust/TS 测试与 `src/styles/globals.css` -- 覆盖边界、映射、过期响应和窄窗布局。

**Acceptance Criteria:**
- Given 新导入 PDF/Office，when 在媒体管理点击“查看文档”，then 双栏默认显示 `Annot + JSON` 且六标签可切换。
- Given 搜索命中第 N 页，when 选择结果，then PDF/Annot 定位第 N 页，JSON 显示完整 ODL JSON。
- Given Preview 含外置图片，when 渲染，then 仅登记图片内联且脚本不能执行。
- Given 文档缺格式，when 选择该标签，then 显示空态且不触发 ODL、模型或整页渲染。

## Verification

- `npm.cmd run build`
- `npm.cmd run test:frontend`
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- `cargo test --manifest-path src-tauri/Cargo.toml`

## Suggested Review Order

**共享查看器**

- 双栏六格式入口
  [`DocumentFormatViewer.tsx:53`](../../src/components/document-viewer/DocumentFormatViewer.tsx#L53)

- Preview 沙箱净化
  [`DocumentFormatViewer.tsx:371`](../../src/components/document-viewer/DocumentFormatViewer.tsx#L371)

**制品与安全**

- 六格式生成契约
  [`pdf_structure.rs:236`](../../src-tauri/src/providers/pdf_structure.rs#L236)

- 原子制品登记
  [`import_service.rs:1337`](../../src-tauri/src/services/import_service.rs#L1337)

- 制品账本扩展
  [`0008_document_view_artifacts.sql:1`](../../src-tauri/migrations/0008_document_view_artifacts.sql#L1)

- 可信读取边界
  [`document_viewer_service.rs:25`](../../src-tauri/src/services/document_viewer_service.rs#L25)

- Typed 命令入口
  [`document_viewer_commands.rs:8`](../../src-tauri/src/commands/document_viewer_commands.rs#L8)

**页面接入**

- 搜索命中定位
  [`SearchPage.tsx:239`](../../src/features/search/SearchPage.tsx#L239)

- 媒体精简接入
  [`MediaManagementPage.tsx:232`](../../src/features/media-management/MediaManagementPage.tsx#L232)

- 响应式双栏布局
  [`globals.css:1057`](../../src/styles/globals.css#L1057)

**契约与回归**

- 前端类型契约
  [`app.ts:158`](../../src/types/app.ts#L158)

- v7 升级保障
  [`db.rs:466`](../../src-tauri/src/repositories/db.rs#L466)

- 缺失制品空态
  [`document_viewer_service.rs:547`](../../src-tauri/src/services/document_viewer_service.rs#L547)

- 失败统计去重
  [`MediaAssetList.test.ts:52`](../../src/features/media-management/components/MediaAssetList.test.ts#L52)
