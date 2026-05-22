**语言 / Language:** [中文](README.md) | [English](README.en.md)

# SLICER

SLICER 是一个本地优先的桌面文档切片与检索工具。它可以把 PDF、PPT、PPTX、DOC、DOCX 等文档按页面转换成图片，再通过多模态模型生成页面级 JSON 元数据，最后用本地 BM25 索引提供可追溯的搜索结果。

项目采用 Tauri + React + TypeScript + Rust 构建，第一优先平台是 Windows。所有文档、页面图片、数据库、JSONL 元数据和索引默认保存在用户选择的本地工作区，不做默认云同步。

## 适用场景

- 本地知识库整理：把课件、报告、方案、制度、论文等资料转成页面级知识资产。
- 文档页面检索：按关键词搜索页面标题、摘要、可见文字、主题、关键词和来源文件名。
- 多模态页面理解：通过用户配置的视觉/多模态模型，把页面图片分析为结构化 JSON。
- 企业资料归档：整理培训材料、销售 PPT、方案文档、流程说明等视觉资料。
- 本地自动化集成：通过 localhost HTTP API 查询搜索结果、页面记录、文档记录或触发索引重建。

## 主要功能

- 选择本地工作区，并自动初始化工作区目录结构。
- 导入 PDF、DOC、DOCX、PPT、PPTX 文件。
- PDF 逐页渲染为 PNG 图片。
- Office 文档通过本机 LibreOffice headless 转为 PDF 后再渲染为 PNG。
- 页面图片使用内容哈希命名，减少重复图片冲突。
- 使用 SQLite 保存文档、页面、任务、分析、索引和设置状态。
- 导出页面级元数据到 `metadata/pages.jsonl`。
- 配置模型 provider、Base URL、自定义 endpoint、model name 和 API key。
- 分析页面图片并生成 `page_analysis_v1` JSON。
- 支持单页分析、批量分析新页面、单文档重新分析和失败页重试。
- 构建和重建本地 BM25 索引。
- 在桌面 GUI 中搜索页面，查看图片预览和页面 JSON。
- 可选启用 localhost HTTP API。

## 技术栈

- 前端：React 19、TypeScript、Vite
- 桌面壳：Tauri 2
- 后端：Rust
- 数据库：SQLite
- 搜索：Tantivy BM25
- HTTP API：Axum
- PDF 渲染：Pdfium
- Office 转换：LibreOffice headless
- 密钥保存：系统密钥存储能力，API key 不写入普通配置文件

## 安装前准备

### 必需环境

1. Node.js：建议 `20.19+` 或 `22.12+`。
2. Rust stable 与 Cargo。
3. Tauri 2 所需系统依赖。
4. Windows 上建议安装 Microsoft WebView2 Runtime 和 C++ Build Tools。

### 可选环境

1. LibreOffice：导入 DOC、DOCX、PPT、PPTX 时需要。只导入 PDF 时可以不配置。
2. 多模态模型 API：需要页面分析时配置。支持 OpenAI、Anthropic、SiliconFlow、OpenAI-compatible、自定义 HTTP，以及本地测试用 `local_mock`。

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
- `pages/` 保存逐页渲染出的 PNG 图片。
- `metadata/pages.jsonl` 保存页面级 JSONL 导出。
- `indexes/bm25/` 保存本地搜索索引。
- `app.db` 是 SQLite 本地账本。
- `logs/` 保存应用诊断日志。

### 2. 导入文档

进入“工作台”，点击“选择文件”，选择一个或多个文档：

- 支持：`.pdf`、`.doc`、`.docx`、`.ppt`、`.pptx`
- PDF 会直接渲染为页面图片。
- Office 文档会先调用 LibreOffice 转为 PDF，再渲染为页面图片。

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

API Key 通过系统密钥存储保存，不会写入普通配置文件。启用云端模型分析前，应用会提示页面图片会发送到用户配置的模型服务。

如果只是想本地试用流程，可以选择 `local_mock` provider。

### 5. 分析页面

导入完成后，在“工作台”的“模型分析”区域点击“分析新页面”。

分析完成后，每页会生成符合 `page_analysis_v1` 的 JSON。分析结果会写入 SQLite，并导出到工作区的：

```text
metadata/pages.jsonl
```

### 6. 构建或重建索引

进入“搜索”页或工作台中的索引状态区域，点击“构建索引”或“重建索引”。

索引基于已分析页面构建，搜索文本包含：

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
- 页面图片预览
- 页面 JSON

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
- 只有启用云端或自定义模型分析时，页面图片才会发送到用户配置的模型服务。
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

### 模型分析不可用

检查设置页中的 Provider、Base URL、自定义 Endpoint、Model Name 和 API Key。使用云端模型前需要确认隐私提示。

### 搜索不可用

搜索依赖已分析页面和 BM25 索引。请先完成页面分析，再在搜索页构建或重建索引。

### Localhost API 不可用

确认设置页已启用 API，端口未被占用，并且监听地址是 `127.0.0.1`。默认端口是 `17321`。

## 当前状态说明

本项目仍处于 MVP 开发阶段。README 以当前仓库已有能力和规划中的 MVP 流程为准，后续如果 UI、API 或工作区结构变化，应同步更新本文档。
