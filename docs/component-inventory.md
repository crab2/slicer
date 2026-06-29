# UI 组件清单 — SLICER

## 概述

SLICER 前端采用 **React 19 + TypeScript** 构建，使用功能模块（Feature-based）架构组织。UI 通信通过 **Tauri IPC**（`invoke`）与 Rust 后端交互。

---

## 应用架构

```
App
└── AppShell（主壳）
    ├── Sidebar（侧边导航栏）
    │   ├── Brand（品牌标识）
    │   ├── NavItems（导航按钮列表）
    │   └── StatusBadge（工作区状态指示）
    ├── Topbar（顶栏 — 当前视图标题 + 状态徽章）
    └── ContentArea（内容区 — 按 activeView 切换）
        ├── WorkbenchPage（工作台）
        ├── AnalysisPage（模型分析）
        ├── ExportPage（一键导出）
        ├── IndexPage（BM25 索引）
        ├── SearchPage（搜索）
        └── SettingsPage（设置）
```

## 导航视图

| 视图 ID | 标签 | 功能 |
|---------|------|------|
| `workbench` | 工作台 | 文档导入、任务列表、页面浏览 |
| `analysis` | 模型分析 | AI 页面分析配置和执行 |
| `export` | 一键导出 | Markdown + 媒体文件导出 |
| `index` | BM25 索引 | 全文索引构建和状态 |
| `search` | 搜索 | 全文搜索页面 |
| `settings` | 设置 | 工作区和应用配置 |

## 通用组件 (`src/components/common/`)

| 组件 | 文件 | 说明 |
|------|------|------|
| `Button` | `Button.tsx` | 通用按钮组件 |
| `EmptyState` | `EmptyState.tsx` | 空状态占位组件 |
| `StatusBadge` | `StatusBadge.tsx` | 状态徽章（`success`/`warning`/`neutral`） |
| `ErrorMessage` | `ErrorMessage.tsx` | 错误信息展示组件 |

## 功能模块组件

### 工作台 (`features/workbench/`)

| 组件 | 文件 | 说明 |
|------|------|------|
| `WorkbenchPage` | `WorkbenchPage.tsx` | 工作台主页面 |
| `WorkspacePicker` | `components/WorkspacePicker.tsx` | 工作区选择器 |
| `ImportResultList` | `components/ImportResultList.tsx` | 导入结果列表 |
| `JobList` | `components/JobList.tsx` | 任务列表 |
| `JobListControls` | `components/JobListControls.tsx` | 任务列表控制 |
| `DocumentList` | `components/DocumentList.tsx` | 文档列表 |
| `AnalysisJobList` | `components/AnalysisJobList.tsx` | 分析任务列表 |

### 分析 (`features/analysis/`)

| 组件 | 文件 | 说明 |
|------|------|------|
| `AnalysisPage` | `AnalysisPage.tsx` | 模型分析页面 |

### 搜索 (`features/search/`)

| 组件 | 文件 | 说明 |
|------|------|------|
| `SearchPage` | `SearchPage.tsx` | 搜索页面 |
| `IndexStatusPanel` | `components/IndexStatusPanel.tsx` | 索引状态面板 |

### 设置 (`features/settings/`)

| 组件 | 文件 | 说明 |
|------|------|------|
| `SettingsPage` | `SettingsPage.tsx` | 设置页面 |
| `WorkspaceSettings` | `components/WorkspaceSettings.tsx` | 工作区设置 |
| `ApiServerSettings` | `components/ApiServerSettings.tsx` | API 服务器设置 |
| `PrivacyNotice` | `components/PrivacyNotice.tsx` | 隐私声明 |

### 导出 (`features/export/`)

| 组件 | 文件 | 说明 |
|------|------|------|
| `ExportPage` | `ExportPage.tsx` | 导出页面 |

### 索引 (`features/index/`)

| 组件 | 文件 | 说明 |
|------|------|------|
| `IndexPage` | `IndexPage.tsx` | 索引管理页面 |

---

## 状态管理

SLICER **不使用** Redux、Zustand 等外部状态管理库。状态管理依赖于：

1. **React `useState`** — 组件级本地状态（每个功能页面管理自己的状态）
2. **Props 下传** — 父组件（AppShell）持有全局状态（如 `workspaceStatus`、`activeView`），通过 props 传递给子页面
3. **Tauri IPC** — 所有持久化数据通过 Rust 后端的 Tauri commands 读写，前端只维护展示状态

状态流：`Rust Backend ↔ Tauri IPC ↔ React State ↔ UI Components`

## 前端-后端通信 (`src/lib/tauriClient.ts`)

`tauriClient` 对象是前端与 Rust 后端的统一通信层：

| 类别 | Tauri Command | 用途 |
|------|--------------|------|
| 工作区 | `get_workspace_status` | 获取工作区状态 |
| | `select_workspace` | 选择工作区目录 |
| 设置 | `get_app_settings` | 获取应用设置 |
| | `save_app_settings` | 保存应用设置 |
| API 密钥 | `save_api_key` / `save_provider_api_key` | 保存 AI 提供商 API 密钥 |
| | `list_api_keys` / `add_api_key` / `activate_api_key` / `delete_api_key_record` | 密钥管理 |
| | `delete_api_key` / `delete_provider_api_key` | 删除密钥 |
| 隐私 | `accept_privacy_notice` | 接受隐私声明 |
| 任务 | `list_jobs` / `create_job` / `update_job_progress` / `fail_job` / `recover_interrupted_jobs` | 后台任务管理 |
| 导入 | `import_pdf` / `list_documents` / `retry_import` / `delete_document` | 文档导入 |
| 页面 | `list_pages` / `list_workbench_pages` / `get_page_image_preview` | 页面浏览 |
| 分析 | `analyze_page` / `analyze_new_pages` / `reanalyze_document` / `reanalyze_failed_pages` / `recover_interrupted_analysis_pages` | AI 分析 |
| 搜索 | `search_pages` / `get_index_status` / `start_index_rebuild` | 搜索 |
| API 服务 | `get_api_server_status` / `reset_api_token` | 内嵌 API 管理 |
| 导出 | `export_media` | 媒体导出 |
