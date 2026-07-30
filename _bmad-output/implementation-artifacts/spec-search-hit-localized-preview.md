---
title: '搜索命中局部预览'
type: 'bugfix'
created: '2026-07-30'
status: 'done'
baseline_commit: 'a7f2146'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/spec-unified-document-format-viewer.md'
---

<frozen-after-approval reason="human-owned intent - do not modify unless human renegotiates">

## Intent

**Problem:** 搜索结果选中后，右侧查看器展示全量 Annot 标注和整份文档 JSON，无法直观看出具体召回模块；例如点击第 4 页的段落命中时，页面上其他模块也被标色，JSON 仍是全文数据。

**Approach:** 搜索页使用专用命中预览：左栏显示原始目标页，只用蓝色背景覆盖当前命中的 bbox；右栏只展示当前召回项自带的模块或页面 JSON。历史页图优先复用，结构化 PDF 缺少页图时复用现有 Pdfium 在请求内存中渲染单个目标页；媒体管理继续使用六格式全文查看器。

## Boundaries & Constraints

**Always:** 预览以当前选中 `hit_id` 为边界；页面仅用蓝色背景标出该命中的有效 bbox，不显示其他 ODL 标注；模块命中优先显示 `module_json`，旧版页面命中回退到 `page_json`；同文档切换命中必须立即更新页面、bbox 和 JSON；保持 JSON 美化、空态、错误态、缓存上限和请求过期保护。

**Ask First:** 若实现需要新增 PDF 渲染依赖、持久化新页图、重新解析 ODL 制品，或改变媒体管理的全文查看行为，必须先确认。

**Never:** 搜索预览不得使用全量 Annot PDF；右栏不得回退展示整份文档 JSON；不得由前端扫描或裁剪整份 ODL JSON 来猜测命中模块；不得批量生成或持久化整页 PNG；不得让旧请求覆盖新选中的命中。

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| 模块命中 | `module_json` 和有效 bbox，第 N 页 | 左栏显示第 N 页且只有目标 bbox 为蓝色背景；右栏只显示该模块 JSON | 非法 JSON 按原字符串安全显示 |
| 旧版页面命中 | `module_json` 为空，`page_json` 有值 | 右栏显示该召回页面数据 | 空内容显示明确空态，不加载全文 JSON |
| 结构化 PDF | 目标页无持久化页图 | 内存中仅渲染被请求页并叠加目标蓝色 bbox | 制品缺失或渲染失败时显示预览错误，不落盘 |
| 同文档切换命中 | 连续选择不同 `hit_id` / 页码 | 页面、蓝色 bbox 和右栏同步换成新命中 | 丢弃旧命中的异步响应 |
| 媒体管理查看 | 未提供命中上下文 | 保持双栏六格式全文查看 | 沿用现有错误与重试行为 |

</frozen-after-approval>

## Code Map

- `src-tauri/src/providers/pdf_renderer.rs` -- 校验目标页并使用 Pdfium 在内存中渲染单页 PNG。
- `src-tauri/src/services/search_service.rs` -- 搜索页图读取；结构化页面缺图时从已登记 canonical PDF 内存渲染目标页。
- `src/features/search/SearchPage.tsx` -- 专用命中预览、受限页图缓存、蓝色 bbox 与命中 JSON。
- `src/features/search/searchPageCopy.ts` -- 搜索预览超时、坏图与重试文案。
- `src/features/search/SearchPage.test.tsx` -- 搜索结果到命中上下文的模块/页面回退测试。
- `src/styles/globals.css` -- 命中预览双栏、蓝色 bbox 和响应式布局样式。

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/src/services/search_service.rs` -- 扩展现有页图预览读取，在无历史页图时校验已登记 canonical PDF 并用 Pdfium 内存渲染单个目标页。
- [x] `src/features/search/SearchPage.tsx` -- 恢复受限页图缓存和过期响应保护，构建原始页 + 单一蓝色 bbox + 命中 JSON 的专用双栏预览。
- [x] `src/styles/globals.css` -- 定义命中双栏、原始页面、纯蓝色高亮与窄屏布局，不影响媒体管理查看器。
- [x] `src/features/search/SearchPage.test.tsx`, `src-tauri/src/services/search_service.rs` -- 覆盖模块/页面 JSON 回退、bbox 边界、缓存、结构化页按需渲染和非法页码。

**Acceptance Criteria:**
- Given 搜索命中属于多页文档的第 N 页, when 用户选中结果, then 左栏显示第 N 页且仅目标 bbox 有蓝色背景，右栏只显示该 `hit_id` 对应的数据块。
- Given 用户在同一文档的不同命中间切换, when 选中项变化, then 页面、蓝色背景区域和 JSON 均对应最后一次选择。
- Given 用户从媒体管理打开文档, when 使用左右任一格式标签, then 六格式全文查看能力与当前版本一致。

## Spec Change Log

- 2026-07-30 审查修复：模块命中不再回退页级 JSON；补充页码上限与 32 位转换保护、Pdfium 并发限制、预览超时/重试、坏图错误态、缓存替换边界、弹窗焦点管理和服务层 canonical PDF 回退测试。

## Design Notes

命中 JSON 和 bbox 直接来自搜索 DTO，不读取全文 ODL JSON。现有 `get_page_image_preview` IPC 继续作为唯一页图入口：先读历史页图；没有时从已登记且通过工作区边界校验的 canonical PDF 渲染一个目标页为临时 data URL。搜索不再打开全量 Annot 制品，媒体管理仍使用独立的全文格式查看器。

## Verification

**Commands:**
- `npm.cmd run test:frontend` -- 4 个测试文件、15 项测试通过。
- `npm.cmd run build` -- TypeScript 与 Vite 生产构建通过。
- `npm.cmd run test:media-boundaries` -- 10 项媒体边界检查通过。
- `cargo test --offline --manifest-path src-tauri/Cargo.toml` -- 227 项单元测试与 8 项集成测试通过，包含实际 PDF 单页渲染和非法页码测试。

**Manual checks:**
- 已在 Tauri 窗口用“大学”搜索：第 4 页与第 6 页命中依次切换时，加载阶段不显示旧页，加载完成后仅当前 bbox 为蓝色，JSON 仅含对应模块快照；媒体管理仍展示 PDF、Annot、Preview、HTML、MD、JSON 六格式全文查看入口。

## Suggested Review Order

**命中上下文绑定**

- 专用双栏预览入口
  [`SearchPage.tsx:380`](../../src/features/search/SearchPage.tsx#L380)

- 页图绑定当前页面
  [`SearchPage.tsx:492`](../../src/features/search/SearchPage.tsx#L492)

- 限定命中数据回退
  [`SearchPage.tsx:638`](../../src/features/search/SearchPage.tsx#L638)

**按需单页渲染**

- 优先复用历史页图
  [`search_service.rs:175`](../../src-tauri/src/services/search_service.rs#L175)

- 内存渲染目标页
  [`pdf_renderer.rs:106`](../../src-tauri/src/providers/pdf_renderer.rs#L106)

**视觉与容错**

- 单一蓝色命中层
  [`globals.css:901`](../../src/styles/globals.css#L901)

- 双栏响应式布局
  [`globals.css:1235`](../../src/styles/globals.css#L1235)

- 明确超时重试文案
  [`searchPageCopy.ts:37`](../../src/features/search/searchPageCopy.ts#L37)

**回归保障**

- 覆盖数据块边界
  [`SearchPage.test.tsx:47`](../../src/features/search/SearchPage.test.tsx#L47)

- 验证 canonical 回退
  [`search_service.rs:1367`](../../src-tauri/src/services/search_service.rs#L1367)
