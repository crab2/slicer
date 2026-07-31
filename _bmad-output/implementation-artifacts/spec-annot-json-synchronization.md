---
title: 'Annot 标注块与 JSON 联动'
type: 'feature'
created: '2026-07-31'
status: 'done'
baseline_commit: '7f990b34be474e0c4d59a9c172b132b9a2c10ffb'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/spec-unified-document-format-viewer.md'
---

<frozen-after-approval reason="human-owned intent - do not modify unless human renegotiates">

## Intent

**Problem:** 媒体管理的 Annot 栏由浏览器原生 PDF iframe 渲染，彩色标注块与右侧完整 ODL JSON 没有关联；用户无法从页面块快速确认对应 JSON，也无法点击标注后定位到 JSON 对象。

**Approach:** 将 Annot 栏改为基于已登记 Annot PDF 的按页交互视图，并使用同一文档的 ODL JSON 建立块级关联。悬停或键盘聚焦标注块时显示浮动 JSON，点击时将另一栏切换为 JSON、滚动到对应对象并高亮；同时提供页码与缩放控制。

## Boundaries & Constraints

**Always:** 页面图来自通过边界、哈希和大小校验的已登记 Annot PDF；关联由同一 ODL JSON 的对象路径和 bounding box 确定；悬停与键盘聚焦显示块 JSON；点击在另一栏显示完整 JSON 并定位高亮；文档切换清空缓存和交互状态；保留六格式及 PDF 标签行为；页图按需加载并限制缓存。

**Ask First:** 引入新的 PDF 前端渲染依赖；修改 ODL 生成格式或持久化 schema；把按页预览图片写入工作区；改变搜索页专用命中预览行为。

**Never:** 不访问浏览器 PDF 插件内部 DOM；不靠文本搜索猜块关系；不在查看时运行 ODL；不以单块 JSON 替代完整 JSON；不让过期请求覆盖当前页；不因单个坏 bbox 阻断页面。

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| 标注块悬停 | Annot 与有效 ODL JSON、bbox | 指针附近显示该对象的格式化 JSON；离开后关闭 | 超长内容在浮层内滚动，不撑破窗口 |
| 标注块点击 | 另一栏不是 JSON | 另一栏切到完整 JSON，滚动并高亮同一路径对象 | 对象不存在时保留完整 JSON，并显示非阻塞提示 |
| 翻页与缩放 | 多页文档 | 仅加载当前页 Annot PNG，热点等比缩放 | 丢弃过期响应；渲染失败可重试 |
| 不完整结构 | JSON 非法或缺页码/bbox | 页面仍可看，仅有效块可交互 | 回退原始 JSON 源码，不生成坏热点 |

</frozen-after-approval>

## Code Map

- `src/components/document-viewer/DocumentFormatViewer.tsx` -- 六格式双栏状态、Annot 页视图、JSON 结构化源码和块联动入口。
- `src/styles/globals.css` -- Annot 工具栏、页面热点、浮动 JSON、JSON 定位高亮及窄窗口布局。
- `src/types/app.ts`, `src/lib/tauriClient.ts` -- 按页预览 DTO 与 Tauri client。
- `src-tauri/src/{domain,services,providers,commands}` -- Annot PDF 安全读取、Pdfium 按页渲染、几何 DTO 与命令。
- `src/components/document-viewer/DocumentFormatViewer.test.ts` -- JSON 路径、bbox、选择定位、过期请求和缓存边界测试。

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/src/{providers,domain,services,commands}`, `src-tauri/src/lib.rs` -- 增加仅允许 Annot 的单页 PNG + 几何命令，复用页码校验并保持制品安全边界。
- [x] `src/types/app.ts`, `src/lib/tauriClient.ts` -- 对齐页图 DTO 和命令参数。
- [x] `src/components/document-viewer/DocumentFormatViewer.tsx` -- 解析块路径、按页加载 Annot，提供翻页缩放、浮层、键盘焦点和 JSON 定位高亮。
- [x] `src/styles/globals.css` -- 添加稳定尺寸、可见焦点、热点状态、浮层防溢出和响应式样式，兼容文件中的现有未提交修改。
- [x] `src/components/document-viewer/DocumentFormatViewer.test.ts`, `src-tauri/src/services/document_viewer_service.rs` -- 覆盖联动、坏 JSON/bbox、页码、制品校验、缓存和过期响应。

**Acceptance Criteria:**
- Given 媒体管理默认打开 `Annot + JSON`，when 用户悬停、聚焦或点击任一有效标注块，then 浮层与右侧高亮对象均来自同一 ODL JSON 路径。
- Given 用户连续切换页码、缩放或文档，when 异步页图返回顺序与请求顺序不同，then 只显示当前文档当前页，热点仍与页面内容对齐。
- Given JSON 无法解析或某些块 bbox 无效，when 打开 Annot，then 页面图和其他格式仍可使用，且界面不出现错误热点或崩溃。

## Spec Change Log

## Design Notes

原生 PDF iframe 没有可靠块 DOM。后端把已验证 Annot PDF 当前页渲染为临时 data URL 并返回几何；前端从完整 ODL JSON 生成稳定对象路径和热点，JSON 源码用同一路径生成锚点。页图不落盘，小容量 LRU 在文档切换时失效。

## Verification

**Commands:**
- `npm.cmd run test:frontend` -- 文档查看器及现有前端测试全部通过。
- `npm.cmd run build` -- TypeScript 与 Vite 生产构建通过。
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` -- Rust 格式检查通过。
- `cargo test --manifest-path src-tauri/Cargo.toml document_viewer` -- 文档查看服务、页码、几何和渲染测试通过。
- `cargo test --manifest-path src-tauri/Cargo.toml pdf_renderer` -- PDF 单页渲染与页码边界测试通过。
- `git diff --check` -- 改动无空白错误。

**Manual checks (if no CLI):**
- 在媒体管理打开真实多页 ODL 文档，检查标注块悬停浮层、键盘焦点、点击 JSON 跳转、高亮、翻页、缩放和窄窗口无重叠。

## Suggested Review Order

**块级联动**

- 点击块保存稳定路径，并将对侧切为完整 JSON。
  [`DocumentFormatViewer.tsx:199`](../../src/components/document-viewer/DocumentFormatViewer.tsx#L199)

- 当前页按需加载，并用代际与 LRU 隔离过期响应。
  [`DocumentFormatViewer.tsx:513`](../../src/components/document-viewer/DocumentFormatViewer.tsx#L513)

- 同源热点提供悬停、焦点、滚动浮层和点击入口。
  [`DocumentFormatViewer.tsx:679`](../../src/components/document-viewer/DocumentFormatViewer.tsx#L679)

- 完整 JSON 仅包裹目标字符范围并自动定位。
  [`DocumentFormatViewer.tsx:733`](../../src/components/document-viewer/DocumentFormatViewer.tsx#L733)

**安全页图**

- 服务层校验制品，并在读取前淘汰同窗格旧请求。
  [`document_viewer_service.rs:113`](../../src-tauri/src/services/document_viewer_service.rs#L113)

- Pdfium 单页渲染同时返回 CropBox 与旋转几何。
  [`pdf_renderer.rs:118`](../../src-tauri/src/providers/pdf_renderer.rs#L118)

- Tauri 命令保持轻量参数与结构化错误边界。
  [`document_viewer_commands.rs:37`](../../src-tauri/src/commands/document_viewer_commands.rs#L37)

**路径与坐标**

- 一次解析生成稳定对象路径、格式化源码和字符范围。
  [`DocumentFormatViewer.tsx:800`](../../src/components/document-viewer/DocumentFormatViewer.tsx#L800)

- PDF 坐标按裁剪框与页面旋转归一化。
  [`DocumentFormatViewer.tsx:910`](../../src/components/document-viewer/DocumentFormatViewer.tsx#L910)

**呈现与验证**

- 工具栏、热点、浮层和高亮遵循现有查看器视觉语言。
  [`globals.css:1275`](../../src/styles/globals.css#L1275)

- 单元测试覆盖路径、坏 bbox、旋转、缓存与过期响应。
  [`DocumentFormatViewer.test.ts:22`](../../src/components/document-viewer/DocumentFormatViewer.test.ts#L22)

- 客户端和 DTO 固化页图命令的跨端契约。
  [`tauriClient.ts:300`](../../src/lib/tauriClient.ts#L300)
