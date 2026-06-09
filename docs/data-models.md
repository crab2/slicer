# 数据模型 — SLICER

## 概述

SLICER 使用 **SQLite** 作为本地持久化数据库，通过 Rust 的 `sqlx` 进行异步 SQL 操作。Schema 通过迁移文件管理，位于 `src-tauri/migrations/`。

---

## 数据库表

### 1. `settings` — 应用设置

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `key` | TEXT | PRIMARY KEY | 设置键名 |
| `value` | TEXT | NOT NULL | 设置值（JSON 序列化） |
| `updated_at` | TEXT | NOT NULL | 最后更新时间（ISO 8601） |

### 2. `errors` — 错误记录

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `error_id` | TEXT | PRIMARY KEY | 错误 ID（UUID） |
| `code` | TEXT | NOT NULL | 错误码（如 `import_pdf_timeout`） |
| `message` | TEXT | NOT NULL | 用户可读的错误描述 |
| `stage` | TEXT | NOT NULL | 错误发生的阶段（`import`/`analysis`/`search`/`api`） |
| `retryable` | INTEGER | NOT NULL, CHECK (0/1) | 是否可重试 |
| `details` | TEXT | NULLABLE | 详细诊断信息 |
| `correlation_id` | TEXT | NOT NULL, UNIQUE | 关联 ID，用于日志追踪 |
| `created_at` | TEXT | NOT NULL | 创建时间（ISO 8601） |

**索引：** `idx_errors_correlation_id` ON `errors(correlation_id)`

### 3. `jobs` — 后台任务

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `job_id` | TEXT | PRIMARY KEY | 任务 ID（UUID） |
| `job_type` | TEXT | NOT NULL | 任务类型（`import`/`analysis`/`index_rebuild`/`export`） |
| `status` | TEXT | NOT NULL, CHECK | `queued` → `running` → `succeeded`/`failed`/`cancelled` |
| `progress` | INTEGER | NOT NULL, DEFAULT 0, CHECK 0-100 | 进度百分比 |
| `created_at` | TEXT | NOT NULL | 创建时间 |
| `updated_at` | TEXT | NOT NULL | 最后更新时间 |
| `error_id` | TEXT | FK → `errors.error_id` | 关联错误 |
| `error_summary` | TEXT | NULLABLE | 错误摘要 |

**索引：** `idx_jobs_status_updated_at` ON `jobs(status, updated_at)`

### 4. `job_events` — 任务事件日志

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `event_id` | TEXT | PRIMARY KEY | 事件 ID（UUID） |
| `job_id` | TEXT | NOT NULL, FK → `jobs.job_id` | 所属任务 |
| `event_type` | TEXT | NOT NULL | 事件类型 |
| `message` | TEXT | NULLABLE | 事件描述 |
| `progress` | INTEGER | NULLABLE, CHECK 0-100 | 事件对应的进度 |
| `created_at` | TEXT | NOT NULL | 创建时间 |

**索引：** `idx_job_events_job_id_created_at` ON `job_events(job_id, created_at)`

### 5. `documents` — 文档记录

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `document_id` | TEXT | PRIMARY KEY | 文档 ID（UUID） |
| `original_filename` | TEXT | NOT NULL | 原始文件名 |
| `file_type` | TEXT | NOT NULL | 文件类型（`pdf`/`docx`/`xlsx`/`pptx`） |
| `file_hash` | TEXT | NOT NULL, UNIQUE INDEX | SHA-256 文件哈希（重复检测） |
| `original_path` | TEXT | NOT NULL | 文件原始路径 |
| `page_count` | INTEGER | NULLABLE | 页数 |
| `status` | TEXT | NOT NULL, CHECK | `pending` → `importing` → `ready`/`failed` |
| `error_summary` | TEXT | NULLABLE | 错误摘要 |
| `job_id` | TEXT | FK → `jobs.job_id` | 关联导入任务 |
| `created_at` | TEXT | NOT NULL | 创建时间 |
| `updated_at` | TEXT | NOT NULL | 最后更新时间 |

**索引：** `idx_documents_file_hash` UNIQUE ON `documents(file_hash)`

### 6. `image_assets` — 图片资产

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `image_hash` | TEXT | PRIMARY KEY | SHA-256 图片内容哈希 |
| `file_path` | TEXT | NOT NULL | 工作区内的相对存储路径 |
| `file_size` | INTEGER | NOT NULL | 文件大小（字节） |
| `created_at` | TEXT | NOT NULL | 创建时间 |

### 7. `page_records` — 页面记录

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `page_id` | TEXT | PRIMARY KEY | 页面 ID（UUID） |
| `document_id` | TEXT | NOT NULL, FK → `documents.document_id` | 所属文档 |
| `page_number` | INTEGER | NOT NULL | 页码（1-based） |
| `image_hash` | TEXT | NOT NULL, FK → `image_assets.image_hash` | 关联页面渲染图片 |
| `status` | TEXT | NOT NULL, CHECK | `pending` → `rendered` → `analysis_pending` → `analyzed`/`failed` |
| `error_summary` | TEXT | NULLABLE | 错误摘要 |
| `created_at` | TEXT | NOT NULL | 创建时间 |
| `updated_at` | TEXT | NOT NULL | 最后更新时间 |

**索引：** `idx_page_records_document_id` ON `page_records(document_id)`
**索引：** `idx_page_records_image_hash` ON `page_records(image_hash)`

### 8. `analysis_results` — AI 分析结果

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `analysis_id` | TEXT | PRIMARY KEY | 分析 ID（UUID） |
| `page_id` | TEXT | NOT NULL, FK → `page_records.page_id`, UNIQUE | 关联页面（每页一份结果） |
| `schema_version` | TEXT | NOT NULL | 分析 JSON Schema 版本 |
| `provider` | TEXT | NOT NULL | AI 提供商（`anthropic`/`openai`/`siliconflow`/`mimo`） |
| `model_name` | TEXT | NOT NULL | 模型名称 |
| `status` | TEXT | NOT NULL, CHECK | `succeeded`/`failed` |
| `result_json` | TEXT | NULLABLE | 分析结果 JSON |
| `error_id` | TEXT | FK → `errors.error_id` | 关联错误 |
| `created_at` | TEXT | NOT NULL | 创建时间 |
| `updated_at` | TEXT | NOT NULL | 最后更新时间 |

**索引：** `idx_analysis_results_page_id` ON `analysis_results(page_id)`

### 9. `index_versions` — 搜索索引版本

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `version_id` | TEXT | PRIMARY KEY | 索引版本 ID（UUID） |
| `provider` | TEXT | NOT NULL | 搜索提供方（`tantivy_bm25`） |
| `analyzer_version` | TEXT | NOT NULL | 分词器版本 |
| `status` | TEXT | NOT NULL, CHECK | `not_built` → `building` → `ready`/`failed` |
| `index_directory` | TEXT | NOT NULL | 索引文件目录路径 |
| `document_count` | INTEGER | NOT NULL, DEFAULT 0 | 已索引文档数 |
| `build_started_at` | TEXT | NULLABLE | 构建开始时间 |
| `build_finished_at` | TEXT | NULLABLE | 构建完成时间 |
| `activated_at` | TEXT | NULLABLE | 激活时间 |
| `error_id` | TEXT | FK → `errors.error_id` | 关联错误 |
| `created_at` | TEXT | NOT NULL | 创建时间 |
| `updated_at` | TEXT | NOT NULL | 最后更新时间 |

**索引：** `idx_index_versions_status_updated` ON `index_versions(status, updated_at)`

### 10. `index_active` — 活跃索引指针

| 列名 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `provider` | TEXT | PRIMARY KEY | 搜索提供方 |
| `version_id` | TEXT | NOT NULL, FK → `index_versions.version_id` | 当前活跃索引版本 |
| `updated_at` | TEXT | NOT NULL | 切换时间 |

---

## 实体关系

```
settings (key-value 配置)
errors ←── jobs (任务关联错误)
errors ←── analysis_results (分析错误)
errors ←── index_versions (索引构建错误)
jobs ←── job_events (任务事件日志)
jobs ←── documents (导入任务)
documents ←── page_records (文档→页面)
image_assets ←── page_records (页面→渲染图片)
page_records ←── analysis_results (页面→AI 分析)
index_versions ←── index_active (当前活跃索引)
```

## 状态机

### 文档状态
```
pending → importing → ready
                   → failed
```

### 页面状态
```
pending → rendered → analysis_pending → analyzed
                  → failed
```

### 任务状态
```
queued → running → succeeded
                → failed
                → cancelled
```

### 索引状态
```
not_built → building → ready
                    → failed
```
