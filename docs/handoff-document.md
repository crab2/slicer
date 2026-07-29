# SLICER 项目交接文档

> 面向接手开发、运维、测试与产品协作人员。最后更新：2026-07-02。

## 1. 项目一句话说明

SLICER 是一个本地优先的 Tauri 桌面应用，用于把 PDF、Office 文档和图片切成页面级图片，调用用户配置的多模态模型生成 `page_analysis_v1` JSON，再通过本地 Tantivy BM25 索引提供可追溯的页面检索能力。

## 2. 当前系统边界

| 边界 | 当前实现 |
| --- | --- |
| 客户端 | React 19 + TypeScript + Vite，入口为 `src/main.tsx` 和 `src/App.tsx` |
| 桌面壳 | Tauri 2，配置在 `src-tauri/tauri.conf.json` |
| 后端核心 | Rust，入口为 `src-tauri/src/main.rs` 和 `src-tauri/src/lib.rs` |
| 本地数据 | 用户选择工作区下的 SQLite `app.db`、页面图片、JSONL、BM25 索引 |
| 远程依赖 | 仅模型分析阶段会访问用户配置的 SiliconFlow、MiMo、OpenAI 或 Anthropic 服务 |
| 本地 API | 可选启用的 Axum localhost API，默认 `127.0.0.1:17321` |
| 优先平台 | Windows；CI 同时覆盖 Windows、macOS、Linux 打包 |

## 3. 必须知道的目录

| 路径 | 用途 |
| --- | --- |
| `src/app/` | 应用壳、导航与页面切换状态 |
| `src/features/` | 前端功能页：工作台、媒体导入、媒体管理、模型分析、导出、索引、搜索、设置 |
| `src/lib/tauriClient.ts` | 前端调用 Tauri command 的统一客户端 |
| `src/types/app.ts` | 前后端 DTO 对应的 TypeScript 类型 |
| `src-tauri/src/commands/` | Tauri IPC command 入口层 |
| `src-tauri/src/services/` | 业务服务层，串联仓储、文件系统、provider、任务 |
| `src-tauri/src/repositories/` | SQLite 访问层与迁移执行 |
| `src-tauri/src/providers/` | PDF、LibreOffice、模型、搜索等外部能力适配 |
| `src-tauri/src/artifacts/` | 工作区布局、JSONL 导出、媒体导出、索引指针 |
| `src-tauri/migrations/` | SQLite schema 迁移 |
| `.github/workflows/release.yml` | GitHub Release 打包流水线 |
| `docs/` | 项目文档入口与交付文档 |

## 4. 工作区数据结构

用户首次选择工作区后，`WorkspaceLayout` 会创建以下结构：

```text
workspace/
  originals/          # 导入原始文件副本
  pages/              # 页面图片，按 document_id 分目录
  analysis/           # 预留分析产物目录
  metadata/
    pages.jsonl       # 页面级 JSONL 导出
  indexes/
    bm25/
      active.json     # 活跃索引指针
      build-*/        # 每次重建生成的新索引目录
  jobs/               # 预留任务文件目录
  logs/               # 运行日志
  tmp/                # 临时转换/渲染文件
  app.db              # SQLite 主账本
```

关键点：

- `app.db` 是事实源，`metadata/pages.jsonl` 和 `indexes/bm25/` 是可重建派生产物。
- API key 不写入 SQLite 或 JSON 配置，密钥内容保存在系统 keyring；SQLite/JSON 中只保留 key id、label、provider、active 状态。
- 工作区切换后，API server 会按新工作区设置重新协调启停。

## 5. 核心业务链路

### 5.1 工作区初始化

1. 前端调用 `select_workspace`。
2. `WorkspaceService` 校验路径、创建目录、创建 `app.db` 与 `metadata/pages.jsonl`。
3. `LedgerRepository::run_initial_migrations` 执行 `src-tauri/migrations/`。
4. 最近工作区写入用户配置目录下的 bootstrap 配置。
5. 如果新工作区启用了 localhost API，`ApiServerService` 重启到新工作区上下文。

### 5.2 文档导入

1. 前端通过 `tauriClient.importFile` 按扩展名选择 `import_pdf` 或 `import_image`。
2. PDF 使用 Pdfium 渲染；Office 先用 LibreOffice headless 转 PDF，再用 Pdfium 渲染；图片会转成 PNG。
3. 原始文件复制到 `originals/`，页面图片写入 `pages/{document_id}/`。
4. 通过 SHA-256 去重原始文件和页面图片。
5. SQLite 写入 `documents`、`image_assets`、`page_records`，并更新 `jobs`。
6. 导入成功后触发 `ArtifactExporter::export_all` 刷新 `metadata/pages.jsonl`。

### 5.3 页面分析

1. 前端调用 `analyze_page`、`analyze_new_pages`、`reanalyze_document` 或 `reanalyze_failed_pages`。
2. `AnalysisService` 检查模型配置、API key 状态和隐私确认。
3. 页面图片会按最大边长与 JPEG 质量规则压缩后发送给模型 provider。
4. provider 返回结果后通过 `validate_page_analysis_v1` 校验 schema、page_id、image_hash 等上下文一致性。
5. JSON 不合规时会尝试一次修复提示；仍无法结构化但可提取文本时会写入 fallback 文本分析。
6. 成功写入 `analysis_results`，页面状态变为 `analyzed`，并刷新 `metadata/pages.jsonl`。

### 5.4 索引与搜索

1. 前端或 API 调用 `start_index_rebuild`。
2. `SearchService` 收集当前成功分析结果，拼接标题、摘要、可见文字、主题、关键词、BM25 文本和原文件名。
3. `TantivyBm25SearchProvider` 在 `indexes/bm25/build-{version_id}` 构建新索引。
4. 构建成功后写入 `index_versions`，更新 `index_active` 和 `indexes/bm25/active.json`。
5. 搜索时读取活跃索引，返回页面、来源文档、分数、摘要、图片可用性和页面 JSON。

### 5.5 Localhost API

可选启用，默认绑定 `127.0.0.1:17321`，禁止配置为 `0.0.0.0`。

| 方法 | 路径 | 认证 | 说明 |
| --- | --- | --- | --- |
| `GET` | `/health` | 无 | 工作区与索引健康状态 |
| `GET` | `/search?q=&limit=` | 无 | 页面搜索 |
| `GET` | `/pages/{page_id}` | 无 | 页面记录 |
| `GET` | `/documents/{document_id}` | 无 | 文档记录 |
| `POST` | `/indexes/rebuild` | Bearer token | 触发索引重建 |

## 6. 数据库速览

| 表 | 说明 |
| --- | --- |
| `settings` | 工作区非敏感设置，值为 JSON 字符串 |
| `errors` | 统一错误与 correlation id |
| `jobs` | 导入、分析、索引、导出等任务状态 |
| `job_events` | 任务事件日志 |
| `documents` | 原始文件记录与导入状态 |
| `image_assets` | 页面图片资产，按 image hash 去重 |
| `page_records` | 文档页面记录与页面状态 |
| `analysis_results` | 每页当前分析结果，`page_id` 唯一 |
| `index_versions` | BM25 索引版本记录 |
| `index_active` | 当前活跃索引版本指针 |

重要状态：

- 文档：`pending -> importing -> ready`，失败为 `failed`。
- 页面：`pending -> rendered -> analysis_pending -> analyzed`，失败为 `failed`。
- 任务：`queued -> running -> succeeded`，失败为 `failed`，也支持 `cancelled`。
- 索引：`not_built -> building -> ready`，失败为 `failed`。

## 7. 开发交接清单

接手后建议按顺序完成：

1. 安装 Node.js LTS、Rust stable、Tauri 2 系统依赖。
2. Windows 环境确认 WebView2 Runtime 与 Visual Studio C++ Build Tools 已安装。
3. 在项目根目录执行 `npm install`。
4. 执行 `npm run build`，确认前端类型检查和 Vite 构建通过。
5. 执行 `cd src-tauri && cargo test`，确认 Rust 单元与集成测试通过。
6. 执行 `npm run tauri dev`，手工选择一个临时工作区。
7. 导入一份 PDF 或图片，确认 `documents`、`page_records` 和页面图片生成。
8. 如需 Office 导入，安装 LibreOffice 并在设置页填写 `soffice.exe` 或 `program` 目录。
9. 配置一个模型 profile，添加 API key，确认隐私提示后分析一页。
10. 构建 BM25 索引，并在搜索页验证查询结果。
11. 如需集成外部脚本，启用 Localhost API，调用 `/health` 和 `/search`。

## 8. 常见变更入口

| 变更类型 | 主要文件 |
| --- | --- |
| 新增 Tauri command | `src-tauri/src/commands/`、`src-tauri/src/lib.rs`、`src/lib/tauriClient.ts`、`src/types/app.ts` |
| 新增数据库字段/表 | `src-tauri/migrations/`、对应 repository、domain DTO、前端类型 |
| 新增模型 provider | `src-tauri/src/providers/model/`、`AnalysisService::build_analysis_context`、设置页 UI |
| 新增页面/导航 | `src/app/navigation.ts`、`src/app/AppShell.tsx`、`src/features/` |
| 修改 Localhost API | `src-tauri/src/api/server.rs`、`endpoints.rs`、`dto.rs`、`auth.rs` |
| 修改索引字段 | `domain/index.rs`、`SearchService::collect_index_documents`、Tantivy provider |
| 修改导入格式 | `fileValidation.ts`、`ImportService`、converter/pdf/image provider |

## 9. 风险与注意事项

- 不要把 API key、Bearer token、Authorization header 或原始模型响应中的敏感字段写入普通日志、JSONL 或前端错误详情。
- `POST /indexes/rebuild` 是写操作，必须保留 Bearer token 认证。
- Localhost API 当前只允许 `127.0.0.1`，不要放宽到公网监听地址。
- `analysis_results.page_id` 目前唯一，表示每页当前结果；如果未来需要历史版本，需要重新设计表结构。
- `indexes/bm25/build-*` 是可重建产物，但活跃索引切换依赖 `index_active` 与 `active.json` 保持一致。
- Office 导入依赖本机 LibreOffice，发布包不内置 LibreOffice。
- 仓库内 `dist/`、`node_modules/`、`src-tauri/target/` 不应作为交付源文件依赖。

## 10. 交接验证结果模板

交接时可记录如下信息：

```text
接手日期：
接手人：
代码分支/提交：
Node.js 版本：
Rust 版本：
操作系统：

验证项：
- npm install：
- npm run build：
- cd src-tauri && cargo test：
- npm run tauri dev：
- PDF/图片导入：
- Office 导入：
- 页面分析：
- 索引构建：
- 搜索：
- Localhost API：

遗留问题：
- 
```

