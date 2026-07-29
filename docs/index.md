# SLICER 项目文档入口

> 本入口面向项目交接、开发协作、部署发布、问题排查和 AI 辅助开发。最后更新：2026-07-02。

## 1. 项目速览

| 属性 | 值 |
| --- | --- |
| 项目类型 | 单体仓库桌面应用 |
| 核心技术 | Tauri 2、React 19、TypeScript、Rust、SQLite、Tantivy BM25、Axum |
| 前端入口 | `src/main.tsx`、`src/App.tsx`、`src/app/AppShell.tsx` |
| 后端入口 | `src-tauri/src/main.rs`、`src-tauri/src/lib.rs` |
| 数据位置 | 用户选择的本地工作区 |
| 本地 API | 可选启用，默认 `127.0.0.1:17321` |
| 主链路 | 文档/图片导入 -> 页面图片 -> 多模态分析 -> JSONL -> BM25 索引 -> 搜索/API |

## 2. 推荐阅读路径

首次接手请按以下顺序阅读：

1. [项目交接文档](./handoff-document.md) - 接手范围、核心链路、数据位置、风险点
2. [部署手册](./deployment-guide.md) - 本地构建、CI 发布、版本、签名、回滚
3. [运维与故障处理手册](./operations-runbook.md) - 工作区、导入、分析、索引、API 排查
4. [关键时序图](./sequence-diagrams.md) - Mermaid 时序图与 Figma 友好的 SVG 资产
5. [架构文档](./architecture.md) - 系统分层与技术栈
6. [API 合约](./api-contracts.md) - Localhost HTTP API 端点与响应格式
7. [数据模型](./data-models.md) - SQLite 表、状态机和关系

## 3. 本次交付文档

| 文档 | 用途 |
| --- | --- |
| [项目交接文档](./handoff-document.md) | 面向接手开发和协作人员 |
| [部署手册](./deployment-guide.md) | 面向发布负责人和测试人员 |
| [运维与故障处理手册](./operations-runbook.md) | 面向现场支持和问题定位 |
| [关键时序图](./sequence-diagrams.md) | 面向流程理解、评审和设计沟通 |
| [Figma 友好 SVG 总览](./assets/slicer-core-flows.svg) | 可导入 Figma 继续美化 |

## 4. 基础项目文档

| 文档 | 用途 |
| --- | --- |
| [项目概览](./project-overview.md) | 项目目标、范围、技术栈摘要 |
| [架构文档](./architecture.md) | Tauri + React + Rust 分层架构 |
| [源码树分析](./source-tree-analysis.md) | 目录结构与关键模块职责 |
| [组件清单](./component-inventory.md) | 前端页面与通用组件 inventory |
| [开发指南](./development-guide.md) | 环境搭建、常用命令、开发任务 |
| [API 合约](./api-contracts.md) | Localhost API 与错误响应 |
| [数据模型](./data-models.md) | SQLite schema、状态机、实体关系 |

## 5. 快速开始

```bash
npm install
npm run build
cd src-tauri
cargo test
cd ..
npm run tauri dev
```

常用验证：

```bash
npm run test:media-boundaries
cd src-tauri && cargo check
```

## 6. 关键源码入口

| 关注点 | 文件/目录 |
| --- | --- |
| 前端导航 | `src/app/navigation.ts`、`src/app/AppShell.tsx` |
| Tauri 客户端 | `src/lib/tauriClient.ts` |
| Tauri command 注册 | `src-tauri/src/lib.rs` |
| 工作区 | `src-tauri/src/services/workspace_service.rs`、`src-tauri/src/artifacts/workspace_layout.rs` |
| 导入 | `src-tauri/src/services/import_service.rs` |
| 分析 | `src-tauri/src/services/analysis_service.rs` |
| 搜索/索引 | `src-tauri/src/services/search_service.rs` |
| Localhost API | `src-tauri/src/api/`、`src-tauri/src/services/api_server_service.rs` |
| 数据库迁移 | `src-tauri/migrations/` |
| 发布流水线 | `.github/workflows/release.yml` |

## 7. BMad 规划与实施资料

规划资料：

- [PRD](../_bmad-output/planning-artifacts/prd.md)
- [BMad 架构文档](../_bmad-output/planning-artifacts/architecture.md)
- [Epic 总览](../_bmad-output/planning-artifacts/epics/overview.md)
- [Epic 列表](../_bmad-output/planning-artifacts/epics/epic-list.md)
- [需求清单](../_bmad-output/planning-artifacts/epics/requirements-inventory.md)

实施资料：

- [Sprint 状态](../_bmad-output/implementation-artifacts/sprint-status.yaml)
- [MVP 验收报告](../_bmad-output/implementation-artifacts/mvp-acceptance-report.md)
- [MVP 验收计划](../_bmad-output/implementation-artifacts/mvp-acceptance-plan.md)
- [延迟工作](../_bmad-output/implementation-artifacts/deferred-work.md)
- [MVP 收尾规格](../_bmad-output/implementation-artifacts/spec-mvp-finalize.md)

## 8. AI 辅助开发提示

- 新增功能前，先读 [项目交接文档](./handoff-document.md) 和 [架构文档](./architecture.md)。
- 修改 API，读 [API 合约](./api-contracts.md)，并同步 `src-tauri/src/api/` 与前端类型。
- 修改数据模型，读 [数据模型](./data-models.md)，新增迁移后运行 `cargo test`。
- 修改主流程，读 [关键时序图](./sequence-diagrams.md)，确认导入、分析、索引、API 的状态流。
- 发布前，按 [部署手册](./deployment-guide.md) 和 [运维手册](./operations-runbook.md) 的检查清单执行。
