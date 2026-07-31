---
title: '补齐分析失败原因与失败项重试'
type: 'bugfix'
created: '2026-07-31'
status: 'in-review'
review_loop_iteration: 0
baseline_commit: '37ae02d15af04799f82a9a6e6cb1faa31e4d04bb'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/3-5-分析失败处理-单页重试与安全诊断.md'
  - '{project-root}/_bmad-output/implementation-artifacts/spec-fix-model-analysis-and-model-fetch-feedback.md'
  - '{project-root}/_bmad-output/planning-artifacts/ux-design-specification.md'
---

<frozen-after-approval reason="human-owned intent - do not modify unless human renegotiates">

## Intent

**Problem:** 模型分析页只显示失败数量，不显示失败对象、持久化原因和诊断号；“分析新内容”不会处理已失败项，导致普通页面或结构化 PDF 视觉模块失败后无法在当前页面恢复。

**Approach:** 复用 SQLite 中已脱敏的 `AppError` 和现有按文档 `reanalyze_failed_pages` 能力，把每个失败页面的代表性原因带入工作台 DTO，在模型分析页按文档展示失败清单并提供失败项重试。

## Boundaries & Constraints

**Always:** 同时覆盖普通失败页和结构化 PDF 失败视觉模块；原因必须来自权威账本且保持 `code`、`stage`、`retryable`、安全 `details`、`correlation_id`；重试后刷新统计与失败清单；部分成功结果不得被覆盖或删除。

**Ask First:** 需要新增数据库迁移、改变 Provider 协议、改变分析状态枚举，或改为自动无限重试时先征求用户确认。

**Never:** 不向 UI 返回密钥、Authorization、完整 endpoint query、图片数据、prompt 或原始模型响应；不复制新的分析执行路径；不把瞬时 toast 作为唯一失败反馈；不让一个文档的失败阻止其他文档继续重试。

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|---------------|---------------------------|----------------|
| 普通页面分析失败 | `page_records.status = failed` 且 current result 关联 `errors` | 失败区显示文档、页码、原因和诊断号，可重试该文档失败项 | 错误记录缺失时显示明确的“未记录具体原因”降级文案，仍保留重试入口 |
| 视觉模块分析失败 | 页面含一个或多个 `visual_module_analysis.status = failed` | 显示失败模块数量和最新代表性错误，可重试该文档全部失败模块 | 多个错误不拼接原始详情，只展示最新代表性安全错误 |
| 重试部分成功 | 同一文档存在多个失败项 | 已成功项保留，失败计数和原因按刷新后的账本更新 | 批次返回代表性错误时继续显示安全诊断，按钮恢复可用 |
| 多文档重试 | 多个文档均有失败项 | 用户可逐文档重试；一个请求失败不影响其他文档或已有状态 | 当前行显示请求错误并可再次操作 |

</frozen-after-approval>

## Code Map

- `src-tauri/src/domain/analysis.rs` -- `PageWorkbenchDto` 的失败诊断契约。
- `src-tauri/src/repositories/analysis_repository.rs` -- 从 current page result 或失败视觉模块关联的 `errors` 读取最新代表性错误。
- `src/types/app.ts` -- Rust DTO 对应的前端类型。
- `src/features/analysis/AnalysisPage.tsx` -- 失败项派生、持久展示、按文档重试和刷新反馈。
- `src/features/analysis/AnalysisPage.test.ts` -- 失败分组、原因降级和统计回归测试。
- `src/styles/globals.css` -- 紧凑、响应式的失败列表样式。

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/src/domain/analysis.rs`, `src-tauri/src/repositories/analysis_repository.rs` -- 为工作台页面附加可选 `AppError`，按更新时间读取普通页或视觉模块的最新失败，并覆盖查询测试。
- [x] `src/types/app.ts`, `src/features/analysis/AnalysisPage.tsx` -- 建立按文档分组的失败视图，展示原因/详情/诊断号，并调用现有 `reanalyzeFailedPages` 后刷新账本状态。
- [x] `src/features/analysis/AnalysisPage.test.ts`, `src/styles/globals.css` -- 覆盖普通页、视觉模块、缺失错误和多文档分组，补齐桌面与窄窗口布局。

**Acceptance Criteria:**
- Given 权威账本中存在分析失败，when 用户打开模型分析页，then 无需重新发起批次即可看到失败文档、页码或模块数量、用户可读原因及可用诊断信息。
- Given 某文档有普通页面或视觉模块分析失败，when 用户点击“重试失败项”，then 仅调用该文档既有失败重试路径，成功项保持不变，并在完成后更新失败列表与汇总。
- Given 错误详情含敏感字段，when DTO 到达前端，then UI 只收到持久化后的脱敏安全内容，不出现密钥、请求正文或原始模型响应。
- Given 重试请求本身失败，when 命令返回错误，then 当前页面持续显示可操作原因，重试控件恢复可用且其他文档不受影响。

## Spec Change Log

## Verification

**Commands:**
- `cargo test --manifest-path .\src-tauri\Cargo.toml --lib analysis_repository` -- 工作台查询返回正确且安全的代表性错误。
- `cargo test --manifest-path .\src-tauri\Cargo.toml --lib analysis_service` -- 现有普通页/视觉模块失败重试语义不回归。
- `npm run test:frontend -- AnalysisPage.test.ts` -- 失败清单派生、降级原因和批次反馈通过。
- `npm run build` -- TypeScript 与生产构建通过。
- `cargo fmt --manifest-path .\src-tauri\Cargo.toml --check` -- Rust 格式通过。
- `git diff --check` -- 无空白错误。
