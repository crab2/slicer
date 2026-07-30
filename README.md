**语言 / Language:** [中文](README.md) | [English](README.en.md)

# SLICER

SLICER 是一个本地优先的桌面文档切片与检索工具。它会保留 PDF 或 Office 转换后的规范 PDF，通过 OpenDataLoader PDF 提取段落、标题、表格、图片等结构化模块，仅对需要视觉理解的图片模块调用多模态模型，最后用本地 BM25 索引提供可定位到页内区域的搜索结果。

项目采用 Tauri + React + TypeScript + Rust 构建，第一优先平台是 Windows。所有文档、提取图片、数据库、JSONL 元数据和索引默认保存在用户选择的本地工作区，不做默认云同步。

## 适用场景

- 本地知识库整理：把课件、报告、方案、制度、论文等资料转成页面级知识资产。
- 文档模块检索：按关键词搜索段落、标题、表格、图片描述和来源文件名，并定位到 PDF 页内区域。
- 按需视觉理解：通过用户配置的视觉/多模态模型分析 PDF 中的有效图片模块；直接导入的图片继续使用整图分析。
- 企业资料归档：整理培训材料、销售 PPT、方案文档、流程说明等视觉资料。
- 本地自动化集成：通过 localhost HTTP API 查询搜索结果、页面记录、文档记录或触发索引重建。

## 主要功能

- 选择本地工作区，并自动初始化工作区目录结构。
- 导入 PDF、DOC、DOCX、PPT、PPTX 文件。
- PDF 及 Office 转换结果会保留为规范 PDF，并通过 OpenDataLoader PDF 提取阅读顺序、模块类型、文本、页码和 bbox。
- PDF 不生成整页 PNG；PDFium 仅读取页数、CropBox 和旋转信息，规范 PDF 直接交给 OpenDataLoader。
- Office 文档通过本机 LibreOffice headless 转为 PDF 后进入同一结构化流程。
- OpenDataLoader 提取的 PDF 图片会登记内容哈希；直接导入的图片仍使用内容哈希命名。
- 使用 SQLite 保存文档、页面、任务、分析、索引和设置状态。
- 导出页面级元数据到 `metadata/pages.jsonl`。
- 配置模型 provider、Base URL、自定义 endpoint、model name 和 API key。
- 仅分析非装饰性视觉模块并生成 `visual_module_analysis_v1` enrichment；直接图片和历史数据继续兼容 `page_analysis_v1`。
- 支持视觉模块独立失败、重试和统计，不因单个图片分析失败丢弃已提取文本。
- 构建和重建本地 BM25 索引。
- 在桌面 GUI 中搜索模块，查看命中模块 JSON 和 bbox；历史页图或直接图片存在时可显示预览。
- 可选启用 localhost HTTP API。

## 技术栈

- 前端：React 19、TypeScript、Vite
- 桌面壳：Tauri 2
- 后端：Rust
- 数据库：SQLite
- 搜索：Tantivy BM25
- HTTP API：Axum
- PDF 渲染：Pdfium
- PDF 结构化：OpenDataLoader PDF 2.5.0（本地 Java 进程）
- Office 转换：LibreOffice headless
- 密钥保存：系统密钥存储能力，API key 不写入普通配置文件

## 安装前准备

### 必需环境

1. Node.js：最低要求 `22.12+`。
2. Rust stable 与 Cargo。
3. Tauri 2 所需系统依赖。
4. Java 11+：PDF/Office 结构化处理需要，应用内置固定版本的 OpenDataLoader PDF JAR，但不内置 JRE。
5. Windows 上建议安装 Microsoft WebView2 Runtime 和 C++ Build Tools。

### 可选环境

1. LibreOffice：导入 DOC、DOCX、PPT、PPTX 时需要。只导入 PDF 时可以不配置。
2. 多模态模型 API：需要页面分析时配置。支持硅基流动 SiliconFlow、MiMo、OpenAI 和 Anthropic。

## 从源码安装

在项目根目录执行：

```bash
npm install
```

如果 Rust 依赖尚未下载，第一次运行或构建 Tauri 时 Cargo 会自动拉取依赖。

## 开发运行

启动桌面应用开发模式：

```bash
npm run tauri dev
```

仅启动前端 Vite 开发服务器：

```bash
npm run dev
```

通常使用 `npm run tauri dev`，因为它会同时启动前端和 Tauri 桌面窗口。

## 构建

构建前端：

```bash
npm run build
```

打包桌面应用：

```bash
npm run tauri build
```

构建产物会由 Tauri 输出到 `src-tauri/target/` 下的对应目录。

## 基本使用流程

### 1. 选择工作区

首次启动后，在工作台或设置页选择一个本地目录作为工作区。SLICER 会在该目录下创建运行所需文件：

```text
workspace/
  originals/
  pages/
  analysis/
  metadata/
    pages.jsonl
  indexes/
    bm25/
  jobs/
  logs/
  tmp/
  app.db
```

说明：

- `originals/` 保存导入的原始文档副本。
- `pages/` 仅保存直接导入图片和历史兼容页图；新导入的 PDF/Office 不在此生成整页 PNG。
- `pdfs/` 保存导入或转换后的规范 PDF。
- `structure/` 保存 OpenDataLoader 原始 JSON 和提取的图片资源。
- `metadata/pages.jsonl` 保存页面级 JSONL 导出。
- `indexes/bm25/` 保存本地搜索索引。
- `app.db` 是 SQLite 本地账本。
- `logs/` 保存应用诊断日志。

### 2. 导入文档

进入“工作台”，点击“选择文件”，选择一个或多个文档：

- 支持：`.pdf`、`.doc`、`.docx`、`.ppt`、`.pptx`
- PDF 会保留规范副本，并直接提取结构化模块和内嵌图片，不生成整页预览图。
- Office 文档会先调用 LibreOffice 转为规范 PDF，再进入相同的直接结构化流程。

如果导入 Office 文档前没有配置 LibreOffice，任务会失败并显示可恢复的错误。配置路径后可以重新导入或重试。

### 3. 配置 LibreOffice

进入“设置”，在 LibreOffice 区域填写安装目录或 `soffice` 可执行文件路径。

Windows 常见路径示例：

```text
C:/Program Files/LibreOffice/program
```

也可以填写：

```text
C:/Program Files/LibreOffice/program/soffice.exe
```

### 4. 配置模型

进入“设置”，填写模型相关配置：

- Provider
- Base URL
- 自定义 Endpoint
- Model Name
- API Key

API Key 通过系统密钥存储保存，不会写入普通配置文件。启用云端模型分析前，应用会提示 OpenDataLoader 提取的 PDF 图片或直接导入图片会发送到用户配置的模型服务。

模型分析必须配置受支持的远程视觉模型 provider，并确认隐私提示。

### 5. 分析视觉模块

导入完成后，在“工作台”的“模型分析”区域点击“分析新内容”。

PDF/Office 中的结构化文本无需模型即可检索；只有非装饰性图片、图表等视觉模块会调用模型并写入 `visual_module_analysis_v1` enrichment。直接导入的图片和历史页面分析仍使用 `page_analysis_v1`，并可导出到：

```text
metadata/pages.jsonl
```

### 6. 构建或重建索引

进入“搜索”页或工作台中的索引状态区域，点击“构建索引”或“重建索引”。

索引基于 OpenDataLoader 结构化模块、视觉 enrichment 和兼容的历史页面分析构建，搜索文本包含：

- 页面标题
- 摘要
- 可见文字
- 主题
- 关键词
- 来源文件名

索引重建不会删除原图片或页面 JSON。重建失败时，已有可用索引会尽量保持可用。

### 7. 搜索页面

进入“搜索”页，输入关键词后执行搜索。结果包含：

- 页面标题或页码
- 摘要
- 来源文档
- 页码
- 相关度分数
- 页面预览（仅历史页图或直接图片存在时）
- 命中模块 JSON
- 模块类型和规范化 bbox；预览存在且坐标有效时高亮命中区域

## Localhost HTTP API

SLICER 可以在设置页启用本地 HTTP API。默认监听：

```text
127.0.0.1:17321
```

可用端点：

```text
GET  /health
GET  /search?q={query}&limit={n}
GET  /pages/{page_id}
GET  /documents/{document_id}
POST /indexes/rebuild
```

示例：

```bash
curl "http://127.0.0.1:17321/health"
```

```bash
curl "http://127.0.0.1:17321/search?q=多模态检索&limit=10"
```

`POST /indexes/rebuild` 是写操作/重任务接口，需要本地 token。可以在设置页的 Localhost API 区域重置 token。

```bash
curl -X POST "http://127.0.0.1:17321/indexes/rebuild" \
  -H "Authorization: Bearer <your-local-token>"
```

响应采用统一结构：

```json
{
  "data": {}
}
```

错误响应采用：

```json
{
  "error": {
    "code": "example_error",
    "message": "错误说明",
    "stage": "api",
    "retryable": true,
    "details": null,
    "correlation_id": "..."
  }
}
```

## 页面 JSON 示例

页面分析结果使用 `page_analysis_v1` schema。示例结构如下：

```json
{
  "page_id": "page_123",
  "image_hash": "7f9a2c91b44d18e2...",
  "image_path": "pages/doc_123/7f9a2c91b44d18e2.png",
  "source": {
    "document_id": "doc_123",
    "original_filename": "AI产品方案.pptx",
    "page_number": 12,
    "total_pages": 30,
    "document_type": "pptx"
  },
  "analysis": {
    "title": "多模态检索系统架构",
    "summary": "该页展示了文档转图片、视觉理解、索引构建和查询返回的整体流程。",
    "topics": ["多模态", "文档解析", "检索"],
    "visible_text": ["输入文档", "图片生成", "多模态分析", "BM25"],
    "keywords": ["PPT转图片", "页面级索引", "视觉分析"],
    "content_type": "architecture_diagram"
  },
  "retrieval": {
    "bm25_text": "多模态检索系统架构 输入文档 图片生成 多模态分析 BM25 页面级索引"
  },
  "model": {
    "provider": "custom_http",
    "model_name": "configured-by-user"
  },
  "schema_version": "page_analysis_v1"
}
```

## 常用命令

```bash
npm install
npm run tauri dev
npm run build
npm run tauri build
```

Rust 后端测试：

```bash
cd src-tauri
cargo test
```

Rust 后端编译检查：

```bash
cd src-tauri
cargo check
```

## 隐私与安全说明

- SLICER 默认本地优先，文档、图片、数据库和索引保存在用户选择的工作区。
- 应用不做默认云同步。
- API Key 使用系统密钥存储，不应出现在日志、导出 JSON、错误提示或搜索结果中。
- 只有启用云端或自定义模型分析时，OpenDataLoader 提取的 PDF 图片或直接导入图片才会发送到用户配置的模型服务；PDF 整页和结构化文本不会发送。
- Localhost API 默认绑定 `127.0.0.1`，不应默认监听公网地址。
- 索引重建等写操作接口需要本地 token。

## 目录结构

```text
.
  src/                  React + TypeScript 前端
  src-tauri/            Rust/Tauri 后端
  src-tauri/src/api/    localhost HTTP API
  src-tauri/src/commands/
                        Tauri commands
  src-tauri/src/services/
                        应用服务层
  src-tauri/src/repositories/
                        SQLite 访问层
  src-tauri/src/providers/
                        PDF、LibreOffice、模型、搜索 provider
  src-tauri/src/artifacts/
                        工作区文件、JSONL、索引目录管理
  src-tauri/migrations/ SQLite 迁移
  public/               静态资源
  docs/                 项目文档
```

## 故障排查

### 启动失败或构建失败

确认 Node.js、Rust、Cargo、Tauri 系统依赖已安装。Windows 环境还需要 WebView2 Runtime 和 C++ Build Tools。

### Office 文档导入失败

确认 LibreOffice 已安装，并在设置页填写正确路径。可以填写 LibreOffice 的 `program` 目录，也可以直接填写 `soffice.exe`。

### PDF 无法渲染

文件可能已损坏、加密，或 Pdfium 渲染库不可用。可以先确认 PDF 能否在普通 PDF 阅读器中打开。

### PDF 结构化失败

确认 `java -version` 可执行且版本为 11 或更高。应用会校验内置 OpenDataLoader PDF JAR；校验或解析失败时不会自动退回到“整页图片调用模型”。

### 模型分析不可用

检查设置页中的 Provider、Base URL、自定义 Endpoint、Model Name 和 API Key。使用云端模型前需要确认隐私提示。

### 搜索不可用

结构化文本可直接构建 BM25 索引；图片模块需先完成视觉分析。请在搜索页构建或重建索引，旧页级索引会保留到新的模块级索引验证并激活成功。

### Localhost API 不可用

确认设置页已启用 API，端口未被占用，并且监听地址是 `127.0.0.1`。默认端口是 `17321`。

## 当前状态说明

本项目仍处于 MVP 开发阶段。README 以当前仓库已有能力和规划中的 MVP 流程为准，后续如果 UI、API 或工作区结构变化，应同步更新本文档。
