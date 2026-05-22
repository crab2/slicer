---
title: 'Story 3.2: 建立 page_analysis_v1 Schema、Prompt 契约与校验器'
type: 'feature'
created: '2026-05-19'
status: 'done'
baseline_commit: 'NO_VCS'
context:
  - '_bmad-output/planning-artifacts/epics.md#story-32'
  - '_bmad-output/planning-artifacts/architecture.md'
  - '_bmad-output/implementation-artifacts/3-1-配置模型-provider-endpoint-与密钥安全状态.md'
---

<!-- Validation: optional — run `bmad-create-story` validate action before dev-story. -->

# Story 3.2: 建立 `page_analysis_v1` Schema、Prompt 契约与校验器

**Status:** done

## Story

As a 本地文档处理用户,  
I want slicer 对模型输出执行版本化 schema 校验和规范化,  
So that 只有可信、结构一致的页面 JSON 会进入本地账本和后续搜索索引。

## Acceptance Criteria

1. **Given** 开发者查看模型分析模块  
   **When** 检查 schema 定义  
   **Then** 项目应包含 `page_analysis_v1` 的结构化 schema 或等价强类型校验定义  
   **And** schema 至少覆盖 `page_id`、`image_hash`、`image_path`、`source`、`analysis`、`retrieval`、`model`、`schema_version` 等核心字段

2. **Given** 模型返回 JSON 内容  
   **When** 系统准备写入 `analysis_results`  
   **Then** 输出必须先通过 `page_analysis_v1` schema 校验  
   **And** 校验失败时不得写入成功分析结果，也不得进入后续索引构建链路

3. **Given** schema 中同时包含 `page_id` 与 `image_hash`  
   **When** 校验器验证页面身份字段  
   **Then** `page_id` 必须匹配 SQLite `page_records` 中的页面 occurrence identity  
   **And** `image_hash` 必须匹配页面图片内容 identity，二者不得被校验器等同处理

4. **Given** 模型输出缺少检索文本  
   **When** 输出仍包含可规范化的 title、summary、topics、visible_text 或 keywords  
   **Then** 规范化逻辑可生成或补齐 `retrieval.bm25_text`  
   **And** 生成规则应可测试、稳定，并保留原始结构化字段用于审查

5. **Given** 模型返回非法 JSON、错误字段类型、未知 schema version 或超长内容  
   **When** 校验器处理输出  
   **Then** 应返回结构化校验错误，包含失败路径、stage、retryable 标记和安全摘要  
   **And** 日志只允许保存经过 redaction 和安全截断的诊断摘要

6. **Given** 后续版本可能升级分析 schema  
   **When** 开发者查看 schema 模块  
   **Then** schema version 应显式记录并可扩展  
   **And** Story 3.2 不应实现 BM25 索引构建、搜索体验或 localhost API

## Tasks / Subtasks

- [x] **建立分析领域类型**（AC: 1, 3, 6）
  - [x] 新建 `src-tauri/src/domain/analysis.rs` 并在 `src-tauri/src/domain/mod.rs` 注册
  - [x] 定义 `PAGE_ANALYSIS_SCHEMA_VERSION: &str = "page_analysis_v1"`
  - [x] 定义强类型结构：`PageAnalysisV1`, `PageAnalysisSource`, `PageAnalysisContent`, `PageRetrievalFields`, `PageAnalysisModelInfo`
  - [x] 字段使用 `snake_case` serde 序列化；禁止在结构中出现 API key、Authorization header、token 或完整请求体字段

- [x] **建立 prompt 契约**（AC: 1, 4, 6）
  - [x] 新建 `src-tauri/src/providers/model/mod.rs`
  - [x] 新建 `src-tauri/src/providers/model/prompt_template.rs`
  - [x] 在 prompt 中明确要求模型只返回 JSON，`schema_version` 必须为 `page_analysis_v1`
  - [x] Prompt contract 至少列出必需字段、语言偏好、禁止输出 Markdown 包裹、禁止包含 secret/credential
  - [x] 不在本 story 中实现 HTTP provider 或真实模型请求

- [x] **实现 schema 校验与规范化器**（AC: 2, 3, 4, 5）
  - [x] 新建 `src-tauri/src/providers/model/schema_validator.rs`
  - [x] 暴露函数，例如 `validate_page_analysis_v1(raw_json, expected_page) -> AppResult<PageAnalysisV1>`
  - [x] `expected_page` 应包含 `page_id`, `document_id`, `page_number`, `image_hash`, `image_path`
  - [x] 校验 `schema_version == "page_analysis_v1"`；未知版本返回 `AppError`，code 建议 `analysis_schema_version_unsupported`
  - [x] 校验 `page_id` 与 `image_hash` 分别匹配 expected page；不得把 `page_id` 当作 `image_hash`
  - [x] 对缺失 `retrieval.bm25_text` 的有效输出，按稳定顺序由 title、summary、visible_text、topics、keywords 生成
  - [x] 对非法 JSON、字段类型错误、字段缺失、过长文本返回统一 `AppError`，stage 建议 `analysis_validation`

- [x] **准备后续持久化边界但不写入数据库**（AC: 2, 6）
  - [x] 不创建 `analysis_results` migration，除非仅添加被本 story 测试直接需要的最小类型；首个真实写入应留给 Story 3.3/3.6
  - [x] 不新增 Tauri command、HTTP API、SearchProvider、BM25 索引构建或搜索 UI
  - [x] 如需要 repository 类型，只能创建接口/DTO 草稿，不得让校验器自行写 SQLite

- [x] **测试与 fixtures**（AC: 1–6）
  - [x] 新建 `src-tauri/fixtures/sample_analysis/valid_page_analysis_v1.json`
  - [x] 新建无效 fixtures：非法 JSON、缺 `schema_version`、未知 schema version、`page_id` 不匹配、`image_hash` 不匹配、错误字段类型
  - [x] Rust 单元测试覆盖：有效 JSON round-trip、身份字段校验、`bm25_text` 补齐、非法输入错误 code/stage、redaction 不泄露 secret
  - [x] 运行 `cargo test --lib`，确保现有导入、settings、diagnostics 测试不回归

### Review Findings

- [x] [Review][Patch] 不要在非法 JSON 错误 details 中回显 raw 模型输出 [src-tauri/src/providers/model/schema_validator.rs:43]
- [x] [Review][Patch] 字段类型错误只报告 `path=$`，不满足具体失败路径要求 [src-tauri/src/providers/model/schema_validator.rs:50]
- [x] [Review][Patch] 合法 JSON 中未知或敏感字段会被 serde 忽略并通过校验 [src-tauri/src/providers/model/schema_validator.rs:21]
- [x] [Review][Patch] Prompt 将 `retrieval.bm25_text` 标为必需，但 validator 允许补齐，契约不一致 [src-tauri/src/providers/model/prompt_template.rs:5]
- [x] [Review][Patch] `language_preference` 未限制字符，可能注入额外 prompt 指令 [src-tauri/src/providers/model/prompt_template.rs:28]
- [x] [Review][Patch] 仅限制单字段长度，缺少 raw JSON、数组和总文本规模上限 [src-tauri/src/providers/model/schema_validator.rs:210]

## Dev Notes

### 当前代码基线（必读，避免重复造轮子）

| 能力 | 现状 | 本 story 动作 |
|------|------|----------------|
| 页面 occurrence identity | `DocumentRepository::create_page_record` 当前生成 `page_id = "{document_id}_{page_number}"` | 校验器必须按这个 occurrence ID 匹配，不得用 `image_hash` 替代 |
| 图片内容 identity | `PageRecordDto.image_hash` 与 `image_assets.image_hash` 已由导入流水线生成 | 校验器必须单独匹配 `image_hash` |
| 页面列表 | `DocumentRepository::list_pages_by_document` / `list_all_pages` 已返回 `PageRecordDto` | 可复用这些 DTO 设计 expected page context |
| JSONL artifact | `ArtifactExporter` 当前导出 pages/documents/jobs | 本 story 不改 JSONL 输出；分析结果 JSONL 留给 Story 3.6 |
| 错误模型 | `AppError` + `redact_secrets` 已存在 | 校验失败必须用 `AppError`，details 只放安全摘要 |
| 密钥边界 | Story 3.1 已完成 `security::read_api_key()` / keyring / settings 分离 | 本 story 不读取 API key、不实现真实模型请求 |
| 模型 provider 目录 | 当前 `src-tauri/src/providers/` 只有 converter/libreoffice/pdf_renderer | 本 story 新建 `providers/model/` 放 prompt 和 schema validator |

### Schema 建议结构

最小 `page_analysis_v1` JSON 应包含：

```json
{
  "schema_version": "page_analysis_v1",
  "page_id": "document_id_1",
  "image_hash": "sha256...",
  "image_path": "pages/<document_id>/<image_hash>.png",
  "source": {
    "document_id": "uuid",
    "page_number": 1,
    "original_filename": "example.pdf"
  },
  "analysis": {
    "title": "页面标题",
    "summary": "页面摘要",
    "visible_text": "可见文字",
    "topics": ["主题"],
    "keywords": ["关键词"]
  },
  "retrieval": {
    "bm25_text": "用于 BM25 的规范化文本"
  },
  "model": {
    "provider": "custom",
    "model_name": "configured-model"
  }
}
```

允许 `retrieval.bm25_text` 缺失或为空时由校验器补齐；不允许 `page_id`、`image_hash`、`schema_version`、`source.document_id`、`source.page_number` 缺失。

### 规范化规则

- `bm25_text` 补齐顺序固定：`analysis.title`、`analysis.summary`、`analysis.visible_text`、`analysis.topics[]`、`analysis.keywords[]`
- 每段 trim 后跳过空字符串，用 `\n` 或单个空格连接均可，但测试必须锁定输出
- 不要删除原始 `analysis` 字段；补齐仅写入 `retrieval.bm25_text`
- 对超长字符串执行安全上限校验，建议单字段上限 50_000 字符，错误 details 不回显完整内容

### 架构合规（必须遵守）

- 业务逻辑在 `services/`，SQLite 仅在 `repositories/`，外部模型能力在 `providers/model/`。[Source: `_bmad-output/planning-artifacts/architecture.md` — Architectural Boundaries]
- 模型输出必须在写入 `analysis_results` 或进入索引前通过 `page_analysis_v1` schema 校验。[Source: `_bmad-output/planning-artifacts/architecture.md` — Integration/Data Flow]
- JSON 字段、DTO、状态值统一使用 `snake_case`；时间戳使用 RFC 3339。[Source: `_bmad-output/planning-artifacts/architecture.md` — Naming & Format Patterns]
- 本 story 不实现 BM25 索引、搜索 UI、localhost API，也不调用真实模型。[Source: `_bmad-output/planning-artifacts/epics.md` — Story 3.2 AC6]

### Previous Story Intelligence

- Story 3.1 已建立模型配置、密钥安全状态和隐私确认；3.2 应复用其设置/安全边界，不能新增返回明文 key 的 command。
- Story 3.1 code review 修复过：空白 API key、隐私确认 OR 规则、全局 JSON 双写、非 localhost API 设置校验。3.2 不应回退这些安全边界。
- 当前工作台已有“分析就绪”占位；3.2 只提供 schema/prompt/validator，后续 Story 3.3 才把它接入真实分析入口。

### Testing Requirements

- 单元测试放在新增模块旁边；fixtures 放在 `src-tauri/fixtures/sample_analysis/`
- 测试不依赖外部模型服务、真实 API key、网络或 GUI
- 必须覆盖成功和失败路径；失败路径断言 `AppError.code`、`stage` 和 details 不含 secret
- 回归命令：`cargo test --lib`

### References

- [Source: `_bmad-output/planning-artifacts/epics.md` — Epic 3, Story 3.2]
- [Source: `_bmad-output/planning-artifacts/architecture.md` — Project Structure / Architectural Boundaries / Data Flow]
- [Source: `_bmad-output/implementation-artifacts/3-1-配置模型-provider-endpoint-与密钥安全状态.md` — Previous Story Intelligence]
- [Source: `src-tauri/src/repositories/document_repository.rs` — `create_page_record`, `list_pages_by_document`, `list_all_pages`]
- [Source: `src-tauri/src/domain/page.rs` — `PageRecordDto`]
- [Source: `src-tauri/src/errors.rs` — `AppError`, `redact_secrets`]

## Dev Agent Record

### Agent Model Used

GPT-5.5

### Debug Log References

- `cargo test --lib providers::model::schema_validator -- --nocapture` 先失败，确认校验器测试覆盖缺失实现。
- `cargo test --lib model -- --nocapture` 通过，确认模型 prompt 与 schema validator 单元测试转绿。
- `cargo test --lib` 通过：36 passed。
- `ReadLints` 检查新增/修改 Rust 文件：No linter errors found。
- Code review 修复后运行 `cargo test --lib model -- --nocapture` 通过：17 passed。
- Code review 修复后运行 `cargo test --lib` 通过：40 passed。
- Code review 修复后 `ReadLints` 检查 `analysis.rs`、`prompt_template.rs`、`schema_validator.rs`、`Cargo.toml`：No linter errors found。

### Completion Notes List

- 新增 `page_analysis_v1` 领域强类型与 schema version 常量，字段保持 `snake_case` serde 合同，未引入 secret/request body 字段。
- 新增 `providers/model` prompt contract，明确只返回 JSON、固定 `page_analysis_v1`、列出必需字段、语言偏好与 secret/credential 禁止项；未实现 HTTP provider 或真实模型请求。
- 新增 `validate_page_analysis_v1` 与 `ExpectedPageContext`，校验 schema version、页面 occurrence identity、图片内容 identity、source/page path，并对缺失 `retrieval.bm25_text` 的有效输出按固定顺序补齐。
- 校验失败统一返回 `AppError`，stage 为 `analysis_validation`，包含安全 details；非法 JSON 场景复用 redaction，超长字符串只返回路径和长度摘要。
- 未创建 `analysis_results` migration、Tauri command、HTTP API、SearchProvider、BM25 索引构建或搜索 UI。
- Code review 修复：非法 JSON 不再回显 raw，字段类型错误返回具体 serde path，未知字段被拒绝，`retrieval` 对象必需但 `bm25_text` 可补齐，语言偏好做 prompt sanitization，并增加 raw payload、数组和总文本规模上限。

### File List

- `src-tauri/src/domain/analysis.rs` (new)
- `src-tauri/src/domain/mod.rs`
- `src-tauri/src/providers/mod.rs`
- `src-tauri/src/providers/model/mod.rs` (new)
- `src-tauri/src/providers/model/prompt_template.rs` (new)
- `src-tauri/src/providers/model/schema_validator.rs` (new)
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`
- `src-tauri/fixtures/sample_analysis/valid_page_analysis_v1.json` (new)
- `src-tauri/fixtures/sample_analysis/invalid_json.json` (new)
- `src-tauri/fixtures/sample_analysis/missing_schema_version.json` (new)
- `src-tauri/fixtures/sample_analysis/unknown_schema_version.json` (new)
- `src-tauri/fixtures/sample_analysis/page_id_mismatch.json` (new)
- `src-tauri/fixtures/sample_analysis/image_hash_mismatch.json` (new)
- `src-tauri/fixtures/sample_analysis/wrong_field_type.json` (new)

## Change Log

- 2026-05-19: Story 3.2 实现 — 新增 page_analysis_v1 领域类型、prompt contract、schema validator、fixtures 与单元测试；状态更新为 review。
- 2026-05-19: Story 3.2 code review 修复 — 收紧 schema validator 安全边界、错误路径、prompt 契约和规模上限；状态更新为 done。
- 2026-05-19: Story 3.2 创建 — page_analysis_v1 schema、prompt contract 与校验器开发上下文。
