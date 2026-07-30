---
title: '修复 Office 文档导入的临时目录标识错误'
type: 'bugfix'
created: '2026-07-30'
status: 'done'
review_loop_iteration: 0
baseline_commit: '192b8a16959634c4e4b0d9d3f46df1f5e628753e'
context:
  - '{project-root}/_bmad-output/planning-artifacts/architecture.md'
  - '{project-root}/_bmad-output/implementation-artifacts/spec-2-3-office-to-pdf-conversion.md'
---

<frozen-after-approval reason="human-owned intent - do not modify unless human renegotiates">

## Intent

**Problem:** DOC、DOCX、PPT 和 PPTX 导入在调用 LibreOffice 前返回 `workspace_storage_id_invalid`。原因是 Office 导入把 `office-conversion-{job_id}` 传给只接受 UUID 的受管目录 API。

**Approach:** 保持工作区安全校验不变，直接使用 UUID `job_id` 作为转换临时目录标识，并添加无需 LibreOffice 的服务层回归测试。

## Boundaries & Constraints

**Always:** 保留严格 UUID 校验；转换目录位于 `tmp/`；所有退出路径继续由 `ArtifactDirectoryCleanup` 清理；测试不得依赖 LibreOffice 或真实 PDF 解析。

**Ask First:** 改变 ID 格式、放宽路径校验或更改用户可见错误契约。

**Never:** 不绕过受管目录 API；不修改 PDF、图片导入、LibreOffice 配置或 Office 内容兼容逻辑。

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Office 转换开始 | 有效工作区与 `.doc` 文件 | 创建 `tmp/<job_id>/` 并调用转换器 | 不再产生存储 ID 错误 |
| 转换器失败 | 假转换器主动报错 | 保持 `conversion_failed` | 转换目录被清理 |
| 非法存储 ID | 路径片段或非 UUID | 安全层继续拒绝 | `workspace_storage_id_invalid` |

</frozen-after-approval>

## Code Map

- `src-tauri/src/services/import_service.rs` -- Office 导入编排和服务层测试；当前构造了带前缀的非法目录 ID。
- `src-tauri/src/artifacts/workspace_layout.rs` -- 不可放宽的 UUID 与路径边界契约。
- `src-tauri/src/repositories/ledger_repository.rs` -- 确认 `job_id` 由 UUID v4 生成。

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/src/services/import_service.rs` -- 使用纯 UUID `job_id` 创建转换目录，保留现有清理流程。
- [x] `src-tauri/src/services/import_service.rs` -- 用主动报错的假 `DocumentConverter` 断言转换器被调用、输出目录末级为 UUID、目录位于 `tmp/` 且返回后已清理。

**Acceptance Criteria:**
- Given 有效工作区和 DOC 文件, when 开始 Office 转换, then 调用转换器且不返回 `workspace_storage_id_invalid`。
- Given 转换器失败, when 导入结束, then 返回 `conversion_failed`、记录失败状态且不残留转换目录。
- Given 路径片段或非 UUID, when 校验存储 ID, then 既有拒绝行为仍通过测试。

## Spec Change Log

## Verification

**Commands:**
- `cargo test --manifest-path src-tauri/Cargo.toml services::import_service::tests` -- expected: 导入服务测试通过。
- `cargo test --manifest-path src-tauri/Cargo.toml artifacts::workspace_layout::tests` -- expected: 路径边界测试通过。
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` -- expected: Rust 库测试通过。
- `npm.cmd run build` -- expected: TypeScript 与 Vite 构建通过。

## Suggested Review Order

**临时目录契约**

- 复用任务 UUID
  [`import_service.rs:576`](../../src-tauri/src/services/import_service.rs#L576)

**回归与清理**

- 覆盖 Office 失败路径
  [`import_service.rs:1741`](../../src-tauri/src/services/import_service.rs#L1741)

- 绑定实际任务 ID
  [`import_service.rs:1774`](../../src-tauri/src/services/import_service.rs#L1774)

- 验证非空目录清理
  [`import_service.rs:1787`](../../src-tauri/src/services/import_service.rs#L1787)

**后续风险**

- 记录通用清理缺口
  [`deferred-work.md:47`](deferred-work.md#L47)
