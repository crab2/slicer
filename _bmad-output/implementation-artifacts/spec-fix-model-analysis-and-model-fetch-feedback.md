---
title: '修复模型分析失败反馈与模型列表按钮无响应'
type: 'bugfix'
created: '2026-07-30'
status: 'done'
review_loop_iteration: 0
baseline_commit: '4c2f2b32c25668a3d5ede8615fab71986a5832cf'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/spec-openai-model-list-fetch.md'
  - '{project-root}/_bmad-output/implementation-artifacts/3-5-分析失败处理-单页重试与安全诊断.md'
---

<frozen-after-approval reason="human-owned intent - do not modify unless human renegotiates">

## Intent

**Problem:** “获取模型”失败后，错误显示在弹窗遮罩后的页面顶部，用户看不到反馈。批量分析即使全部失败也只返回计数，丢失真实 Provider 原因；当前工作区实际收到 `kkcoder.com` 的 HTTP 403 `SUBSCRIPTION_NOT_FOUND`。

**Approach:** 批次结果附带首个已脱敏错误，同时保留逐项执行和成功项；模型弹窗就地显示请求错误，OpenAI 常见 HTTP 状态使用可操作中文提示。

## Boundaries & Constraints

**Always:** 不向前端、日志或错误暴露密钥、Authorization、图片及完整响应；批次继续处理所有单元；错误沿用 `AppError`；空批次仍成功。

**Ask First:** 破坏已有 DTO、迁移数据库、改变 Provider 请求协议或修改第三方账号/订阅。

**Never:** 不伪造成功、不绕过订阅权限、不用真实密钥测试。

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|---------------|----------------------------|----------------|
| 获取模型成功 | 配置有效 | 弹窗显示加载态和模型候选 | 清除旧错误 |
| 获取模型失败 | 401/403/404/429/网络错误 | 弹窗保持打开，按钮恢复 | 就地显示原因、脱敏详情、诊断号 |
| 批次失败 | 全部或部分单元失败 | 保留准确计数与成功结果 | 显示首个代表性错误，失败项可重试 |
| 空批次 | 无待处理单元 | 显示无内容 | 不显示旧错误 |

</frozen-after-approval>

## Code Map

- `src-tauri/src/providers/model/openai_provider.rs` -- HTTP 状态解析。
- `src-tauri/src/services/analysis_service.rs` -- 批处理与错误汇总。
- `src-tauri/src/domain/analysis.rs`, `src/types/app.ts` -- 批次 DTO。
- `src/features/analysis/AnalysisPage.tsx` -- 分析反馈。
- `src/features/settings/SettingsPage.tsx` -- 模型弹窗交互。
- `src/features/analysis/AnalysisPage.test.ts` -- 前端回归测试。

## Tasks & Acceptance

**Execution:**
- [x] `analysis_service.rs`, `domain/analysis.rs` -- 将首个代表性 `AppError` 加入批次 DTO，不改变并发、持久化和部分成功语义。
- [x] `openai_provider.rs` -- 复用安全的 401/403/404/429 错误分类，覆盖订阅缺失。
- [x] `app.ts`, `AnalysisPage.tsx` -- 同时显示批次统计与消息、详情、诊断号，并在新请求前清除旧错误。
- [x] `SettingsPage.tsx` -- 使用弹窗专用错误状态；请求成功或相关输入变化时清除。
- [x] Rust/前端测试 -- 覆盖矩阵并验证脱敏。

**Acceptance Criteria:**
- Given 服务返回 `SUBSCRIPTION_NOT_FOUND`, when 获取模型或分析, then 当前弹窗/面板提示账号或分组无可用订阅并显示诊断号。
- Given 批次部分失败, when 完成, then 成功项已保存、失败项可重试，UI 显示准确计数和代表性错误。
- Given 错误含敏感请求字段, when 到达 UI/持久化层, then 密钥、Authorization、图片和原始请求体不可见。

## Spec Change Log

## Verification

**Commands:**
- `cargo test --manifest-path .\src-tauri\Cargo.toml --lib openai_provider` -- HTTP 分类通过。
- `cargo test --manifest-path .\src-tauri\Cargo.toml --lib analysis_service` -- 全失败、部分失败、空批次通过。
- `npm run test:frontend -- AnalysisPage.test.ts` -- 前端反馈通过。
- `npm run build` -- 前端构建通过。
- `cargo fmt --manifest-path .\src-tauri\Cargo.toml --check` -- 格式通过。
- `git diff --check` -- 无空白错误。

**Manual:** 使用模拟 Tauri HTTP 403 `SUBSCRIPTION_NOT_FOUND` 验证错误位于模型配置弹窗内，详情已脱敏、诊断号可见，弹窗可滚动且无内容重叠。

## Suggested Review Order

**批次错误传播**

- 从批次执行入口理解稳定首错、计数和部分成功语义。
  [`analysis_service.rs:1103`](../../src-tauri/src/services/analysis_service.rs#L1103)

- 批次 DTO 以结构化 AppError 跨越 Rust 边界。
  [`analysis.rs:126`](../../src-tauri/src/domain/analysis.rs#L126)

- 前端同时保留统计反馈并呈现代表性诊断。
  [`AnalysisPage.tsx:702`](../../src/features/analysis/AnalysisPage.tsx#L702)

**Provider 诊断与安全**

- HTTP 状态映射为可操作提示且不携带响应正文。
  [`openai_provider.rs:396`](../../src-tauri/src/providers/model/openai_provider.rs#L396)

- 多字段、直接密钥和图片数据统一进入脱敏防线。
  [`errors.rs:166`](../../src-tauri/src/errors.rs#L166)

- 弹窗请求采用序列号阻止过期结果回写。
  [`SettingsPage.tsx:332`](../../src/features/settings/SettingsPage.tsx#L332)

- 模型配置错误在当前模态框内就地显示。
  [`SettingsPage.tsx:639`](../../src/features/settings/SettingsPage.tsx#L639)

**测试与契约**

- Rust 测试覆盖输入顺序稳定性和页面部分失败。
  [`analysis_service.rs:2755`](../../src-tauri/src/services/analysis_service.rs#L2755)

- Provider 测试覆盖订阅、404、重试性和正文隔离。
  [`openai_provider.rs:645`](../../src-tauri/src/providers/model/openai_provider.rs#L645)

- 前端测试覆盖完整失败与部分完成标题。
  [`AnalysisPage.test.ts:135`](../../src/features/analysis/AnalysisPage.test.ts#L135)

- TypeScript DTO 与 Rust 批次契约保持一致。
  [`app.ts:241`](../../src/types/app.ts#L241)

- 既有持久化与多文档编排风险已登记为后续工作。
  [`deferred-work.md:55`](deferred-work.md#L55)
