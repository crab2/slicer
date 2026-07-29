# 源码树分析 — SLICER

## 项目根目录结构

```
slicer/                              # 项目根目录
├── src/                             # 🔵 React 前端源码
│   ├── main.tsx                     # ✨ 应用入口点
│   ├── App.tsx                      # 根组件
│   ├── vite-env.d.ts                # Vite 类型声明
│   ├── app/                         # 应用壳层
│   │   ├── AppShell.tsx             # ✨ 主壳组件（导航+视图路由）
│   │   └── navigation.ts           # 导航项定义
│   ├── components/common/           # 🔧 通用 UI 组件
│   │   ├── Button.tsx              # 通用按钮
│   │   ├── EmptyState.tsx          # 空状态占位
│   │   ├── StatusBadge.tsx         # 状态徽章
│   │   └── ErrorMessage.tsx        # 错误信息
│   ├── features/                    # 📦 功能模块
│   │   ├── workbench/              # 工作台 — 导入、任务、文档浏览
│   │   │   ├── WorkbenchPage.tsx
│   │   │   └── components/         # WorkspacePicker, ImportResultList, JobList 等
│   │   ├── analysis/               # 模型分析 — AI 页面分析
│   │   │   └── AnalysisPage.tsx
│   │   ├── search/                 # 搜索 — 全文搜索界面
│   │   │   ├── SearchPage.tsx
│   │   │   ├── searchPageCopy.ts
│   │   │   └── components/IndexStatusPanel.tsx
│   │   ├── settings/               # 设置 — 工作区和应用配置
│   │   │   ├── SettingsPage.tsx
│   │   │   └── components/         # WorkspaceSettings, ApiServerSettings, PrivacyNotice
│   │   ├── export/                 # 导出 — Markdown+媒体导出
│   │   │   └── ExportPage.tsx
│   │   └── index/                  # 索引管理 — BM25 索引构建
│   │       └── IndexPage.tsx
│   ├── lib/                         # 🔧 工具库
│   │   ├── tauriClient.ts          # ✨ Tauri IPC 通信客户端（统一后端调用入口）
│   │   └── fileValidation.ts       # 文件类型校验
│   ├── types/                       # 📋 TypeScript 类型定义
│   │   └── app.ts                  # 核心 DTO 类型（50+ 结构化接口）
│   └── styles/                      # 🎨 全局样式
│       └── globals.css
│
├── src-tauri/                       # 🦀 Rust 后端源码
│   ├── Cargo.toml                   # Rust 依赖清单（23 个 crate）
│   ├── tauri.conf.json              # Tauri 应用配置（窗口/安全/打包）
│   ├── icons/                       # 应用图标
│   ├── src/
│   │   ├── main.rs                  # ✨ Rust 入口点
│   │   ├── lib.rs                   # ✨ Tauri 命令注册（30+ commands）
│   │   ├── errors.rs                # 🔴 统一错误模型（AppError + 诊断）
│   │   ├── api/                     # 🌐 内嵌 HTTP API (Axum)
│   │   │   ├── mod.rs
│   │   │   ├── server.rs            # 服务器启动/优雅关闭
│   │   │   ├── state.rs             # API 共享状态
│   │   │   ├── auth.rs              # Bearer Token 认证抽取器
│   │   │   ├── dto.rs               # 统一 API 响应/错误 DTO
│   │   │   ├── endpoints.rs         # 5 个 API 端点处理器
│   │   │   └── health.rs            # 健康检查端点
│   │   ├── artifacts/               # 📁 文件/资源管理
│   │   │   ├── mod.rs
│   │   │   ├── workspace_layout.rs  # 工作区目录结构
│   │   │   ├── jsonl_exporter.rs    # JSONL 导出
│   │   │   ├── page_json_exporter.rs # 页面 JSON 生成
│   │   │   ├── index_store.rs       # 索引文件存储
│   │   │   └── media_exporter.rs    # 媒体文件导出
│   │   ├── commands/                # 📡 Tauri IPC 命令处理器
│   │   │   ├── mod.rs
│   │   │   ├── workspace_commands.rs
│   │   │   ├── settings_commands.rs
│   │   │   ├── job_commands.rs
│   │   │   ├── import_commands.rs
│   │   │   ├── analysis_commands.rs
│   │   │   ├── search_commands.rs
│   │   │   ├── api_commands.rs
│   │   │   ├── diagnostics_commands.rs
│   │   │   └── export_commands.rs
│   │   ├── diagnostics/             # 🔍 诊断
│   │   ├── domain/                  # 🏗️ 领域模型
│   │   │   ├── mod.rs
│   │   │   ├── workspace.rs
│   │   │   ├── job.rs
│   │   │   ├── document.rs
│   │   │   ├── page.rs
│   │   │   ├── analysis.rs
│   │   │   ├── index.rs
│   │   │   └── settings.rs
│   │   ├── jobs/                    # ⚙️ 后台任务编排
│   │   │   ├── mod.rs
│   │   │   └── job_orchestrator.rs
│   │   ├── providers/               # 🔌 外部集成
│   │   │   ├── mod.rs
│   │   │   ├── model/               # AI 模型提供商
│   │   │   │   ├── mod.rs
│   │   │   │   ├── provider.rs      # Provider trait
│   │   │   │   ├── anthropic_provider.rs
│   │   │   │   ├── openai_provider.rs
│   │   │   │   ├── siliconflow_provider.rs
│   │   │   │   ├── mimo_provider.rs
│   │   │   │   ├── mock_provider.rs
│   │   │   │   ├── prompt_template.rs
│   │   │   │   └── schema_validator.rs
│   │   │   ├── search/              # 搜索引擎
│   │   │   │   ├── mod.rs
│   │   │   │   ├── search_provider.rs
│   │   │   │   ├── tantivy_bm25_provider.rs
│   │   │   │   ├── chinese_analyzer.rs
│   │   │   │   └── mock_search_provider.rs
│   │   │   ├── converter.rs         # 文档格式转换 (LibreOffice)
│   │   │   ├── libreoffice_converter.rs
│   │   │   └── pdf_renderer.rs      # PDF 页面渲染 (pdfium)
│   │   ├── repositories/            # 💾 数据访问层
│   │   │   ├── mod.rs
│   │   │   ├── db.rs                # SQLite 连接管理
│   │   │   ├── workspace_settings_repository.rs
│   │   │   ├── document_repository.rs
│   │   │   ├── analysis_repository.rs
│   │   │   ├── index_repository.rs
│   │   │   ├── settings_repository.rs
│   │   │   └── ledger_repository.rs
│   │   ├── security/                # 🔐 密钥管理
│   │   │   └── mod.rs               # OS keyring 集成
│   │   └── services/                # 🧠 业务逻辑层
│   │       ├── mod.rs
│   │       ├── workspace_service.rs
│   │       ├── settings_service.rs
│   │       ├── import_service.rs
│   │       ├── analysis_service.rs
│   │       ├── search_service.rs
│   │       └── api_server_service.rs
│   └── migrations/                  # 📊 数据库迁移
│       ├── 0001_initial.sql         # settings, errors, jobs
│       ├── 0002_jobs_and_events.sql # job_events
│       ├── 0003_documents_pages_images.sql  # documents, image_assets, page_records
│       ├── 0004_analysis_results.sql        # analysis_results
│       └── 0005_index_versions.sql          # index_versions, index_active
│
├── public/                          # 静态资源
├── docs/                            # 📚 项目文档（本工作流生成）
├── _bmad/                           # BMad 方法配置
├── _bmad-output/                    # BMad 产出物（PRD/架构/史诗/Sprint）
│   ├── planning-artifacts/          # 规划文档
│   └── implementation-artifacts/    # 实施文档
└── .github/workflows/
    └── release.yml                  # CI/CD 发布流水线
```

## 关键入口点

| 入口 | 路径 | 说明 |
|------|------|------|
| 前端 HTML | `index.html` | Vite 构建入口 |
| 前端 JS | `src/main.tsx` | React 渲染入口 |
| 后端原生 | `src-tauri/src/main.rs` | Rust 进程入口 |
| Tauri 命令注册 | `src-tauri/src/lib.rs` | 30+ IPC 命令注册点 |
| IPC 客户端 | `src/lib/tauriClient.ts` | 前端→后端统一通信层 |

## 关键目录

| 目录 | 用途 | 关键程度 |
|------|------|---------|
| `src-tauri/src/domain/` | 核心领域模型和 DTO | ⭐⭐⭐ |
| `src-tauri/src/services/` | 核心业务逻辑 | ⭐⭐⭐ |
| `src-tauri/src/commands/` | Tauri IPC 入口点 | ⭐⭐⭐ |
| `src-tauri/src/repositories/` | 数据持久化 | ⭐⭐ |
| `src-tauri/src/providers/` | 外部集成 | ⭐⭐ |
| `src-tauri/src/api/` | HTTP API | ⭐⭐ |
| `src-tauri/migrations/` | 数据库 Schema | ⭐⭐ |
| `src/features/` | 前端功能页面 | ⭐⭐ |
| `src/lib/tauriClient.ts` | IPC 通信层 | ⭐⭐ |
| `src/types/app.ts` | 前端类型定义 | ⭐ |
