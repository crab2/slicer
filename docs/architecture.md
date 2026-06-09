# 架构文档 — SLICER

## 执行摘要

SLICER 是一个基于 **Tauri v2** 的本地文档处理桌面应用。采用 **Rust 核心 + React 前端** 的跨平台架构，支持 PDF/Office 导入、AI 页面分析、全文搜索索引和 localhost HTTP API。

---

## 技术栈

| 层次 | 技术 | 版本 |
|------|------|------|
| 桌面框架 | Tauri | v2 |
| 前端 UI | React + TypeScript | 19.1 / 5.8 |
| 前端构建 | Vite | 7.0 |
| 后端语言 | Rust | edition 2021 |
| 数据库 | SQLite (sqlx) | 0.8.6 |
| 搜索引擎 | Tantivy (BM25) | 0.22 |
| HTTP API | Axum (Tokio) | 0.8 |
| PDF 渲染 | pdfium-render | 0.8 |
| Office 转换 | LibreOffice (CLI) | 外部 |
| 凭据存储 | OS keyring | v3 |

---

## 架构模式

### 整体架构：Tauri 桌面应用

```
┌─────────────────────────────────────────┐
│              React 前端                  │
│  ┌─────────┐ ┌──────────┐ ┌──────────┐ │
│  │ 工作台   │ │ 模型分析  │ │  搜索    │ │
│  └─────────┘ └──────────┘ └──────────┘ │
│         │              │                  │
│    Tauri IPC (invoke)                    │
├─────────────────────────────────────────┤
│              Rust 后端                    │
│  ┌──────────────────────────────────┐   │
│  │  Commands (IPC 处理器)           │   │
│  ├──────────────────────────────────┤   │
│  │  Services (业务逻辑)             │   │
│  ├──────────────────────────────────┤   │
│  │  Repositories (数据访问)         │   │
│  ├──────────────────────────────────┤   │
│  │  Providers (外部集成)            │   │
│  │  ├── AI 模型 (Anthropic/OpenAI) │   │
│  │  ├── 搜索 (Tantivy BM25)        │   │
│  │  ├── PDF (pdfium)               │   │
│  │  └── 转换 (LibreOffice)         │   │
│  ├──────────────────────────────────┤   │
│  │  Axum API Server (localhost)     │   │
│  └──────────────────────────────────┘   │
│         │              │                  │
│    SQLite           文件系统             │
└─────────────────────────────────────────┘
```

### 后端分层架构（DDD + Repository Pattern）

```
Commands ──→ Services ──→ Repositories ──→ SQLite
                │
                ├──→ Providers (AI/Search/PDF/Converter)
                ├──→ Domain (Models + DTOs)
                └──→ Artifacts (File Management)
```

**设计原则：**
- **Commands** 层是 Tauri IPC 的入口，负责参数解析和错误映射
- **Services** 层包含所有业务逻辑，不直接接触数据库或文件系统
- **Repositories** 层封装 SQL 操作，每个 repository 对应一组相关的数据库操作
- **Providers** 层封装外部集成，通过 trait 抽象支持 mock 替换
- **Domain** 层定义核心数据结构，后端和前端共享 DTO 契约

### 前端架构：Feature-based SPA

```
AppShell (状态持有者)
  ├── Sidebar (导航)
  └── ContentArea
      ├── WorkbenchPage ←→ tauriClient ←→ Rust Commands
      ├── AnalysisPage ←→ tauriClient ←→ Rust Commands
      ├── SearchPage   ←→ tauriClient ←→ Rust Commands
      ├── SettingsPage ←→ tauriClient ←→ Rust Commands
      ├── ExportPage   ←→ tauriClient ←→ Rust Commands
      └── IndexPage    ←→ tauriClient ←→ Rust Commands
```

---

## 数据架构

10 张 SQLite 表，分为四个子域：

| 子域 | 表 | 说明 |
|------|---|------|
| **核心状态** | `settings`, `errors`, `jobs`, `job_events` | 应用配置、错误诊断、后台任务 |
| **文档导入** | `documents`, `image_assets`, `page_records` | 文档→页面→图片的层级关系 |
| **AI 分析** | `analysis_results` | 每页一份分析结果（唯一约束） |
| **搜索索引** | `index_versions`, `index_active` | 索引版本管理和原子切换 |

详见 [data-models.md](./data-models.md)

---

## API 设计

内嵌 **Axum 0.8** HTTP 服务器，绑定 localhost 可配置端口：

| 方法 | 路径 | 认证 | 说明 |
|------|------|------|------|
| GET | `/health` | ❌ | 工作区+索引健康状态 |
| GET | `/search?q=&limit=` | ❌ | 全文搜索 |
| GET | `/pages/{page_id}` | ❌ | 页面详情 |
| GET | `/documents/{document_id}` | ❌ | 文档详情 |
| POST | `/indexes/rebuild` | ✅ Bearer | 触发索引重建 |

详见 [api-contracts.md](./api-contracts.md)

---

## 组件总览

前端共 20+ 组件，按功能分布在 6 个视图中。不使用外部状态管理库 — 状态通过 React `useState` + Tauri IPC 管理。

详见 [component-inventory.md](./component-inventory.md)

---

## 测试策略

| 类型 | 位置 | 框架 |
|------|------|------|
| Rust 单元测试 | `src-tauri/src/` 各模块 | `#[cfg(test)]` |
| API DTO 测试 | `src-tauri/src/api/dto.rs` | 内置测试 |
| 前端类型检查 | — | `tsc --noEmit` |

---

## 部署架构

通过 GitHub Actions 自动构建，支持四平台五目标：

- **Windows** (x86_64) → `.msi` / `.exe`
- **macOS** (Apple Silicon + Intel) → `.dmg`
- **Linux** (x86_64) → `.deb` / `.AppImage`

详见 [deployment-guide.md](./deployment-guide.md)

---

## 开发工作流

```bash
npm run tauri dev   # 开发模式（前端 HMR + Rust 重编译）
npm run tauri build # 生产构建
cargo test          # Rust 测试
npx tsc --noEmit    # 前端类型检查
```

详见 [development-guide.md](./development-guide.md)
