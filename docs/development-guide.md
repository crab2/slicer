# 开发指南 — SLICER

## 前提条件

| 工具 | 最低版本 | 说明 |
|------|----------|------|
| **Node.js** | LTS (≥20) | 前端依赖和构建 |
| **Rust** | stable (edition 2021) | 后端编译 |
| **Tauri CLI** | v2 | 桌面应用构建 |
| **Git** | ≥2.x | 版本控制 |
| **LibreOffice** | ≥7.x | Office 文档→PDF 转换（可选，仅转换功能需要） |

### Windows 额外要求
- **Microsoft Visual Studio C++ Build Tools** — Rust 编译依赖
- **WebView2** — Tauri 运行时（Windows 10+ 自带）

### Linux 额外要求
```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential \
  libssl-dev libayatana-appindicator3-dev librsvg2-dev patchelf
```

### macOS 额外要求
- Xcode Command Line Tools

---

## 环境搭建

```bash
# 1. 克隆仓库
git clone <repo-url>
cd slicer

# 2. 安装前端依赖
npm install

# 3. 进入 Rust 目录并构建
cd src-tauri
cargo build
cd ..
```

---

## 常用命令

| 操作 | 命令 | 说明 |
|------|------|------|
| 启动开发模式 | `npm run tauri dev` | 启动 Tauri 开发环境（热重载） |
| 仅前端开发 | `npm run dev` | Vite 开发服务器（端口 1420） |
| 生产构建 | `npm run tauri build` | 打包桌面应用 |
| 前端类型检查 | `npx tsc --noEmit` | TypeScript 编译检查 |
| Rust 编译检查 | `cd src-tauri && cargo check` | Rust 编译验证 |
| Rust 测试 | `cd src-tauri && cargo test` | 运行 Rust 单元/集成测试 |
| 前端构建 | `npm run build` | TypeScript → Vite 生产构建 |

---

## 项目结构

```
slicer/
├── src/                    # React 前端源代码
│   ├── app/                # 应用壳和导航
│   ├── components/common/  # 通用 UI 组件
│   ├── features/           # 功能模块（按页面组织）
│   │   ├── workbench/      # 工作台
│   │   ├── analysis/       # 模型分析
│   │   ├── search/         # 搜索
│   │   ├── settings/       # 设置
│   │   ├── export/         # 导出
│   │   └── index/          # 索引管理
│   ├── lib/                # 工具库（IPC 客户端、文件校验）
│   ├── types/              # TypeScript 类型定义
│   └── styles/             # 全局样式
├── src-tauri/              # Rust 后端源代码
│   ├── src/
│   │   ├── main.rs         # 入口点
│   │   ├── lib.rs          # Tauri 命令注册
│   │   ├── api/            # 内嵌 Axum HTTP API
│   │   ├── artifacts/      # 文件/资源管理
│   │   ├── commands/       # Tauri IPC 命令处理器
│   │   ├── diagnostics/    # 诊断
│   │   ├── domain/         # 领域模型
│   │   ├── errors.rs       # 统一错误处理
│   │   ├── jobs/           # 后台任务编排
│   │   ├── providers/      # 外部集成（AI/搜索/PDF）
│   │   ├── repositories/   # 数据库访问层
│   │   ├── security/       # 密钥管理
│   │   └── services/       # 业务逻辑层
│   ├── migrations/         # SQLite 迁移文件
│   └── Cargo.toml          # Rust 依赖
├── public/                 # 静态资源
├── docs/                   # 项目文档
└── package.json            # 前端依赖和脚本
```

## 架构分层（Rust 后端）

```
Commands (Tauri IPC 入口)
    ↓
Services (业务逻辑层)
    ↓
Repositories (数据访问层) ← → Domain (领域模型)
    ↓
SQLite (sqlx)

Providers (外部集成)
    ├── model/      → AI 模型调用 (Anthropic/OpenAI/SiliconFlow/Mimo)
    ├── search/     → Tantivy BM25 搜索引擎
    ├── converter/  → LibreOffice 文档转换
    └── pdf_renderer → PDF 页面渲染
```

## 测试

| 测试类型 | 命令 | 位置 |
|----------|------|------|
| Rust 单元测试 | `cargo test` | `src-tauri/src/` 各模块内 |
| Rust 集成测试 | `cargo test` | `src-tauri/tests/` |
| 前端类型检查 | `npx tsc --noEmit` | — |

## 常见开发任务

### 添加新的 Tauri 命令
1. 在 `src-tauri/src/commands/` 中定义命令函数（添加 `#[tauri::command]`）
2. 在 `src-tauri/src/lib.rs` 中注册命令
3. 在 `src/lib/tauriClient.ts` 中添加前端调用封装
4. 在 `src/types/app.ts` 中添加类型定义

### 添加数据库迁移
1. 在 `src-tauri/migrations/` 中创建新的 SQL 文件（如 `0006_xxx.sql`）
2. 在应用启动时通过 workspace 初始化自动执行

### 添加 AI 模型提供商
1. 在 `src-tauri/src/providers/model/` 中实现新 provider（参照 `anthropic_provider.rs`）
2. 在 model provider 注册逻辑中注册新 provider
3. 在设置 UI 中添加新选项
