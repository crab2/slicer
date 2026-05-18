# Story 1.3: 建立 SQLite 权威账本与核心状态枚举

Status: review

## Story

As a 本地文档处理用户,
I want slicer 使用本地 SQLite 账本记录工作区、文档、页面、任务和错误的核心状态,
so that 应用重启、失败恢复和后续导入分析流程都能依赖一致、可追踪的本地状态来源。

## Acceptance Criteria

1. Given 用户已选择并初始化本地工作区, when 应用首次打开该工作区的 `app.db`, then 应用应执行版本化 SQLite migration，创建当前阶段所需的最小权威账本 schema, and migration 应可重复运行且不会破坏已有数据。
2. Given 后续功能需要记录文档与页面生命周期, when 开发者检查数据库 schema 与领域类型, then 系统应定义文档状态枚举，至少覆盖 `pending`、`importing`、`ready`、`failed` 等基础状态, and 系统应定义页面状态枚举，至少覆盖 `pending`、`rendered`、`analysis_pending`、`analyzed`、`failed` 等基础状态。
3. Given 系统需要区分页面发生位置与图片内容身份, when schema 定义文档、页面记录和图片资产关系, then `page_id` 应表示某个文档中的页面 occurrence identity, and `image_hash` 应表示页面图片内容 identity 与文件命名依据，二者不得被定义为同一个字段或同一个语义。
4. Given 应用需要支撑后续导入、分析、索引和错误恢复, when migration 创建当前阶段核心表, then schema 应只创建本故事直接需要的最小账本结构，例如 `settings`、`jobs`、`errors`、migration metadata 或等价基础表, and `documents`、`page_records`、`image_assets`、`analysis_results`、`index_versions` 等业务表应在首次真正写入它们的后续故事中创建或扩展，不应在 Epic 1 一次性提前实现全部未来表。
5. Given Rust 代码需要访问 SQLite 账本, when 开发者查看数据访问实现, then 应存在 repository/service 边界，Tauri command 和前端不得直接访问数据库细节, and 数据库读写应通过 Rust path-safe 工作区上下文定位 `app.db`。
6. Given 应用写入状态、时间戳或跨层 DTO, when 数据从 Rust service 返回给前端或未来 API, then 状态值应使用 `snake_case` 字符串, and 时间戳应使用 RFC 3339 字符串，字段命名应保持 `snake_case`。
7. Given migration 或数据库初始化失败, when 应用启动或切换工作区时遇到该失败, then 应用不得继续把该工作区显示为可用, and 用户应看到中文可恢复错误，并保留重新选择工作区或查看诊断信息的入口。

## Tasks / Subtasks

- [x] 建立版本化 SQLite migration 基础 (AC: 1, 4)
  - [x] 新增 `src-tauri/migrations/0001_initial.sql`，创建 `schema_migrations` 外的最小业务账本表：`settings`、`jobs`、`errors`。
  - [x] migration 必须可重复运行，不提前创建 `documents`、`page_records`、`image_assets`、`analysis_results`、`index_versions`。

- [x] 建立 Rust SQLite repository 边界 (AC: 1, 5, 7)
  - [x] 新增 `src-tauri/src/repositories/db.rs`，集中处理 workspace `app.db` 连接、migration 执行和数据库错误映射。
  - [x] 更新 `src-tauri/src/repositories/ledger_repository.rs`，将任务与错误记录写入 SQLite，而不是 JSON sidecar 文件。
  - [x] 保持 Tauri commands 只调用 service/orchestrator/repository 边界，不直接访问 SQLite。

- [x] 接入工作区选择与恢复流程 (AC: 1, 5, 7)
  - [x] 工作区初始化成功后运行 SQLite migration。
  - [x] 工作区恢复时运行 SQLite migration；migration 失败不得返回 `ready`。
  - [x] 数据库失败通过结构化 `AppError` 映射为中文可恢复错误。

- [x] 定义核心状态枚举和 DTO 输出 (AC: 2, 6)
  - [x] 新增 `src-tauri/src/domain/document.rs`，定义文档基础生命周期状态 `pending`、`importing`、`ready`、`failed`。
  - [x] 新增 `src-tauri/src/domain/page.rs`，定义页面基础生命周期状态 `pending`、`rendered`、`analysis_pending`、`analyzed`、`failed`。
  - [x] 更新 `get_core_status_catalog` 输出基础文档/页面状态与任务状态，保持 `snake_case`。

- [x] 补充验证覆盖 (AC: 1, 3, 4, 5, 6, 7)
  - [x] migration 测试覆盖幂等执行、最小表存在、未来业务表未提前创建。
  - [x] ledger 测试覆盖 job/error 写入 SQLite，且不再生成 `jobs.json` / `errors.json`。
  - [x] workspace 测试通过选择/恢复路径间接覆盖 migration 接入。

## Dev Notes

### Scope Boundaries

- 本故事只建立 SQLite 权威账本基础、migration 机制、最小 `settings/jobs/errors` 表、核心状态枚举和 repository/service 边界。
- 不实现导入、页面记录、图片资产、分析结果、索引版本或完整 Job Orchestrator 恢复状态机；这些属于后续故事。
- `page_id` 与 `image_hash` 的分离只在本故事中作为架构边界和反提前实现约束保留，不创建业务表也不定义页面身份算法。

### Architecture Compliance

- SQLite 是工作区权威账本；文件和 JSONL 是可校验或可重建 artifact。
- `repositories/` 只负责 SQLite 读写和 migration；`services/`/`jobs/` 调用 repository；Tauri commands 不直接操作数据库。
- 状态值、DTO 字段和数据库字段保持 `snake_case`；时间戳使用 RFC 3339 字符串。
- migration 文件位于 `src-tauri/migrations/`，数据库路径由 `WorkspaceLayout::app_db_path()` 生成。

## Dev Agent Record

### Agent Model Used

GPT-5 Codex

### Debug Log References

- 2026-05-18T17:05:00+08:00 - Story 1.2 was already implementation-complete; synchronized story and sprint status to `review` before starting Story 1.3.
- 2026-05-18T17:10:00+08:00 - Confirmed `sqlx = 0.8.6` with SQLite/runtime features is present in `Cargo.toml` and `Cargo.lock`.
- 2026-05-18T17:10:00+08:00 - Added versioned SQLite migration file and DB helper for connection, migration metadata, idempotent migration execution, and database error mapping.
- 2026-05-18T17:10:00+08:00 - Replaced JSON sidecar ledger with SQLite-backed `jobs` and `errors` operations.
- 2026-05-18T17:10:00+08:00 - Connected workspace initialization/restoration to SQLite migration execution so migration failures prevent `ready` workspace status.
- 2026-05-18T17:15:00+08:00 - Frontend validation passed with bundled Node: TypeScript check and Vite production build both succeeded.
- 2026-05-18T17:15:00+08:00 - Rust validation is still blocked in Codex by `windows sandbox: setup refresh failed with status exit code: 1`; escalation request for `cargo test` was rejected because `/codex: codex-auto-review` is unavailable. User-reported `cargo test` output showed `running 0 tests`, so Story 1.3 remains `in-progress` pending a local `cargo test --lib` run that executes the newly added library tests.
- 2026-05-18T17:20:00+08:00 - User ran `cargo test --lib` locally from `D:\AIProject\slicer\src-tauri`; validation passed with `7 passed; 0 failed`, including SQLite ledger and workspace tests.

### Completion Notes List

- Implemented the Story 1.3 SQLite ledger foundation with minimal schema and migration metadata.
- Added core document/page lifecycle status enums and surfaced them through the existing status catalog command.
- Preserved future business table creation for later stories; no `documents`, `page_records`, `image_assets`, `analysis_results`, or `index_versions` tables are created in this story.
- Validation complete: TypeScript check passed, Vite production build passed, and local `cargo test --lib` passed with 7 tests.

### File List

- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`
- `src-tauri/migrations/0001_initial.sql`
- `src-tauri/src/domain/document.rs`
- `src-tauri/src/domain/job.rs`
- `src-tauri/src/domain/mod.rs`
- `src-tauri/src/domain/page.rs`
- `src-tauri/src/repositories/db.rs`
- `src-tauri/src/repositories/ledger_repository.rs`
- `src-tauri/src/repositories/mod.rs`
- `src-tauri/src/services/workspace_service.rs`
- `src/types/app.ts`
- `_bmad-output/implementation-artifacts/1-3-建立-sqlite-权威账本与核心状态枚举.md`
- `_bmad-output/implementation-artifacts/sprint-status.yaml`
