# 项目概览 — SLICER

## 基本信息

| 属性 | 值 |
|------|-----|
| **项目名称** | SLICER |
| **描述** | 本地文档处理桌面应用 |
| **版本** | 0.1.0 |
| **仓库结构** | 单体仓库（Monolith） |
| **项目类型** | 桌面应用（Desktop） |
| **主要语言** | Rust + TypeScript |
| **框架** | Tauri v2 + React 19 |
| **架构模式** | DDD + Repository Pattern + Feature-based SPA |

## 功能概览

SLICER 提供五个核心功能模块：

1. **📄 文档导入（工作台）** — 支持 PDF、Office 文档导入，自动渲染页面为图片，重复检测（SHA-256），任务进度追踪
2. **🤖 AI 页面分析** — 调用 AI 模型（Anthropic/OpenAI/SiliconFlow/Mimo）分析页面内容，生成结构化 JSON
3. **🔍 全文搜索** — 基于 Tantivy BM25 的中文全文搜索，支持索引重建和原子切换
4. **🌐 Localhost API** — 内嵌 Axum HTTP 服务器，提供搜索、页面/文档查询、健康检查端点
5. **📦 一键导出** — 将分析结果和页面图片导出为 Markdown + 媒体文件

## 技术栈总览

| 类别 | 技术 | 用途 |
|------|------|------|
| 桌面框架 | Tauri v2 | 跨平台应用容器 |
| 前端 | React 19 + TypeScript 5.8 + Vite 7 | 用户界面 |
| 后端 | Rust (edition 2021) | 核心逻辑 |
| 数据库 | SQLite (sqlx 0.8.6) | 本地持久化 |
| 搜索 | Tantivy 0.22 + 中文分词 | 全文检索 |
| HTTP API | Axum 0.8 + Tokio 1.x | localhost 接口 |
| PDF | pdfium-render 0.8 | 页面渲染 |
| Office | LibreOffice (CLI) | 格式转换 |
| 安全 | OS keyring v3 | 平台密钥管理 |
| CI/CD | GitHub Actions + tauri-action | 跨平台发布 |

## 文档导航

| 文档 | 内容 |
|------|------|
| [架构文档](./architecture.md) | 系统架构、技术栈、设计模式 |
| [源码树分析](./source-tree-analysis.md) | 完整目录结构、入口点、关键目录 |
| [组件清单](./component-inventory.md) | UI 组件、状态管理、IPC 通信 |
| [数据模型](./data-models.md) | 数据库 Schema、表关系、状态机 |
| [API 合约](./api-contracts.md) | HTTP API 端点、认证、响应格式 |
| [开发指南](./development-guide.md) | 环境搭建、命令、架构分层 |
| [部署指南](./deployment-guide.md) | CI/CD 流水线、版本发布 |

## 快速开始

```bash
# 安装依赖
npm install                     # 前端依赖
cd src-tauri && cargo build     # Rust 依赖（首次较慢）

# 启动开发
npm run tauri dev

# 运行测试
cd src-tauri && cargo test
npx tsc --noEmit               # 前端类型检查
```
