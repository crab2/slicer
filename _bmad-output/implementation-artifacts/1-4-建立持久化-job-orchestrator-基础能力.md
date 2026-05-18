# Story 1.4: 建立持久化 Job Orchestrator 基础能力

Status: in-progress

## Story

As a 本地文档处理用户,
I want slicer 将导入、分析、索引等长时间操作抽象为可持久化、可查询、可恢复的任务,
so that 应用关闭、失败或重启后仍能清楚知道哪些工作已完成、失败或需要继续处理。

## Acceptance Criteria

1. Given 用户已选择可用工作区且 SQLite 账本已初始化, when Rust service 创建一个后台任务, then 任务应写入本地 `jobs` 账本记录，包含任务类型、状态、进度、创建时间、更新时间和可选错误关联, and 任务状态至少覆盖 `queued`、`running`、`succeeded`、`failed`、`cancelled` 等基础状态。
2. Given 后续导入、分析和索引功能都需要执行长任务, when 开发者查看任务编排代码, then 应存在统一 Job Orchestrator 或等价服务边界，用于创建、查询、更新和恢复任务, and Tauri command 不得直接承载长时间业务流程，只能调用 service/orchestrator 并返回任务状态或任务标识。
3. Given 用户在工作台查看当前处理状态, when 应用查询任务列表或任务详情, then 前端应能显示任务类型、状态、进度、最近更新时间和失败摘要, and 在本故事中可以使用示例/占位任务类型验证展示，但必须为后续真实导入、分析、索引任务复用同一结构。
4. Given 应用在任务运行期间关闭或崩溃, when 用户重新打开同一工作区, then Job Orchestrator 应识别上次遗留的 `running` 或不确定状态任务, and 系统应将其标记为可恢复状态，例如 `failed` 或 `queued`，并保留清晰的恢复/失败原因记录，避免界面永久显示运行中。
5. Given 后续功能需要更新任务进度, when service 报告任务阶段、百分比或当前处理项, then Job Orchestrator 应提供统一进度更新接口, and 进度值应可被前端轮询或订阅展示，且不得要求前端理解具体业务内部步骤。
6. Given 任务执行失败, when Orchestrator 记录失败状态, then 任务记录应关联结构化错误或失败摘要, and 用户界面应展示中文可理解失败信息，并为后续重试入口预留位置。
7. Given 任务状态从 Rust service 返回给前端或未来 HTTP API, when 状态 DTO 被序列化, then 字段命名应使用 `snake_case`, and 时间戳应使用 RFC 3339 字符串，状态值应使用稳定的 `snake_case` 枚举字符串。

## Tasks / Subtasks

- [ ] 扩展 SQLite 任务账本 schema (AC: 1, 4, 5, 6, 7)
  - [ ] 新增 `src-tauri/migrations/0002_jobs_and_events.sql`，创建 `job_events` 表并保持幂等。
  - [ ] 保持 `jobs` 表兼容 Story 1.3，不提前引入导入、分析或索引业务表。

- [ ] 扩展 Job Orchestrator 边界 (AC: 1, 2, 4, 5, 6)
  - [ ] 提供统一的 `create_job`、`list_jobs`、`update_progress`、`mark_failed`、`recover_interrupted_jobs` 或等价接口。
  - [ ] Tauri command 只调用 orchestrator，不直接写 SQLite 或执行业务长流程。
  - [ ] `running` 任务恢复时应转为 `failed` 并保留中文失败摘要。

- [ ] 补充前端任务列表基础展示 (AC: 3, 7)
  - [ ] 工作台在工作区 ready 时查询任务列表并显示任务类型、状态、进度、更新时间和失败摘要。
  - [ ] 保留示例/占位任务创建入口用于验证展示结构，不启动真实业务任务。
  - [ ] UI 文案保持中文，前端不理解业务内部步骤，只渲染 DTO。

- [ ] 补充验证覆盖 (AC: 1, 2, 4, 5, 6, 7)
  - [ ] Rust 测试覆盖任务创建、进度更新、失败关联、running 任务恢复和 job_events 写入。
  - [ ] 前端 TypeScript 与 Vite build 通过。
  - [ ] 本机 `cargo test --lib` 通过。

## Dev Notes

### Scope Boundaries

- 本故事只建立持久化任务编排基础能力；不执行真实导入、转换、分析、索引或重试流程。
- 示例/占位任务只能用于验证任务 DTO、SQLite 持久化和 UI 展示结构，不能伪装成真实业务能力。
- Job events 是 live/progress hints 和审计线索，不是前端权威状态来源；前端必须通过显式查询恢复任务状态。

### Architecture Compliance

- `jobs/` 承载 orchestration boundary；`repositories/` 负责 SQLite；commands 只能调用 orchestrator/service。
- 所有状态值和 DTO 字段使用 `snake_case`；时间戳使用 RFC 3339。
- 任务恢复不能依赖 React 内存状态，必须从 SQLite `jobs` 表读取。

## Dev Agent Record

### Agent Model Used

GPT-5 Codex

### Debug Log References

- 2026-05-18T17:25:00+08:00 - Story 1.4 started after Story 1.3 reached `review` with local `cargo test --lib` passing 7 tests.

### Completion Notes List

- Pending implementation.

### File List

- `_bmad-output/implementation-artifacts/1-4-建立持久化-job-orchestrator-基础能力.md`
- `_bmad-output/implementation-artifacts/sprint-status.yaml`
