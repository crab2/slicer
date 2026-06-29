# 项目文档索引 — SLICER

## 项目概览

- **类型：** 单体仓库（Monolith） — 桌面应用
- **主要语言：** Rust + TypeScript
- **架构模式：** Tauri v2 桌面应用 + DDD 后端 + Feature-based React 前端

## 快速参考

| 属性 | 值 |
|------|-----|
| **技术栈** | Tauri v2, React 19, Rust, SQLite, Tantivy, Axum |
| **入口点** | `src/main.tsx` (前端), `src-tauri/src/main.rs` (后端) |
| **架构模式** | DDD + Repository Pattern + Feature-based SPA |
| **数据库** | SQLite (sqlx), 10 张表, 5 个迁移文件 |
| **搜索引擎** | Tantivy BM25 + 中文 jieba 分词器 |
| **API** | Axum 0.8 localhost HTTP, Bearer Token 认证 |

## 生成的文档

- [项目概览](./project-overview.md)
- [架构文档](./architecture.md)
- [源码树分析](./source-tree-analysis.md)
- [组件清单](./component-inventory.md)
- [开发指南](./development-guide.md)
- [部署指南](./deployment-guide.md)
- [API 合约](./api-contracts.md)
- [数据模型](./data-models.md)

## 已有的 BMad 文档

### 规划文档 (`_bmad-output/planning-artifacts/`)
- [PRD](../_bmad-output/planning-artifacts/prd.md)
- [架构文档 (BMad)](../_bmad-output/planning-artifacts/architecture.md)
- [史诗与故事](../_bmad-output/planning-artifacts/epics.md)

### 实施文档 (`_bmad-output/implementation-artifacts/`)
- [Sprint 状态](../_bmad-output/implementation-artifacts/sprint-status.yaml)
- [MVP 验收报告](../_bmad-output/implementation-artifacts/mvp-acceptance-report.md)
- [MVP 验收计划](../_bmad-output/implementation-artifacts/mvp-acceptance-plan.md)
- [延迟工作](../_bmad-output/implementation-artifacts/deferred-work.md)
- [MVP 收尾规格](../_bmad-output/implementation-artifacts/spec-mvp-finalize.md)

## 快速开始

```bash
git clone <repo>
npm install
cd src-tauri && cargo build && cd ..
npm run tauri dev        # 启动开发模式
npm run tauri build      # 生产构建
cd src-tauri && cargo test  # 运行测试
```

## AI 辅助开发指南

当使用此文档进行 AI 辅助开发时：

- **新增功能** → 先阅读 [architecture.md](./architecture.md) 了解系统架构
- **API 变更** → 参考 [api-contracts.md](./api-contracts.md)
- **数据模型变更** → 参考 [data-models.md](./data-models.md)，注意迁移文件
- **UI 变更** → 参考 [component-inventory.md](./component-inventory.md) 和 [source-tree-analysis.md](./source-tree-analysis.md)
- **开发环境** → 参考 [development-guide.md](./development-guide.md)
- **发布部署** → 参考 [deployment-guide.md](./deployment-guide.md)
