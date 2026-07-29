# SLICER 关键时序图

> 本文档包含可维护的 Mermaid 时序图，以及一份适合导入 Figma 继续美化的 SVG 视觉资产：[`assets/slicer-core-flows.svg`](./assets/slicer-core-flows.svg)。最后更新：2026-07-02。

## 1. 图形资产说明

- Mermaid 图用于文档维护、代码评审和快速理解。
- SVG 图是视觉化总览，适合直接拖入 Figma，或在浏览器中打开查看。
- 当前 Codex 会话未暴露可写的 Figma MCP 工具，因此未能直接在 Figma 文件中创建画板；已按 Figma 友好的分层、色彩和版式生成 SVG。

## 2. 工作区初始化

```mermaid
sequenceDiagram
    autonumber
    actor User as 用户
    participant UI as React 前端
    participant IPC as Tauri IPC
    participant WS as WorkspaceService
    participant Layout as WorkspaceLayout
    participant DB as SQLite 迁移
    participant API as ApiServerService

    User->>UI: 选择本地工作区
    UI->>IPC: select_workspace(path)
    IPC->>WS: select_workspace(path, api_server)
    WS->>WS: 校验路径存在/目录/可写
    WS->>Layout: ensure_base_layout()
    Layout-->>WS: 创建 originals/pages/metadata/indexes/tmp/app.db
    WS->>DB: run_initial_migrations(app.db)
    DB-->>WS: schema ready
    WS->>WS: 保存最近工作区 bootstrap
    WS->>API: reconcile_for_new_workspace(settings)
    API-->>WS: API disabled/running/failed
    WS-->>IPC: WorkspaceStatusDto(status=ready)
    IPC-->>UI: 更新工作区状态
```

关键实现位置：

- `src-tauri/src/services/workspace_service.rs`
- `src-tauri/src/artifacts/workspace_layout.rs`
- `src-tauri/src/repositories/db.rs`
- `src-tauri/src/services/api_server_service.rs`

## 3. 文档导入与页面切片

```mermaid
sequenceDiagram
    autonumber
    actor User as 用户
    participant UI as React 前端
    participant Client as tauriClient
    participant Cmd as import_commands
    participant Import as ImportService
    participant Converter as LibreOffice/Pdfium/Image
    participant Repo as DocumentRepository
    participant Files as 工作区文件系统
    participant Jobs as JobOrchestrator
    participant JSONL as ArtifactExporter

    User->>UI: 选择 PDF/Office/图片
    UI->>Client: importMultipleFiles(paths)
    Client->>Cmd: import_pdf/import_image(filePath)
    Cmd->>Import: import_document/import_pdf/import_image
    Import->>Jobs: create_job(import type)
    Import->>Repo: create_document()
    Import->>Files: 复制到 originals/
    alt Office 文档
        Import->>Converter: LibreOffice convert_to_pdf()
        Converter-->>Import: 临时 PDF
    end
    alt PDF 或转换后 PDF
        Import->>Converter: Pdfium render_pdf(dpi=144)
        Converter-->>Import: 页面 PNG bytes
    else 图片
        Import->>Converter: decode image and encode PNG
        Converter-->>Import: 单页 PNG bytes
    end
    loop 每一页
        Import->>Files: 写入 tmp/ 后 rename 到 pages/{document_id}/
        Import->>Repo: create_image_asset / create_page_record
        Import->>Jobs: update_progress()
    end
    Import->>Repo: update_document_status(ready)
    Import->>Jobs: update_progress(100)
    Import->>JSONL: export_all()
    Import-->>Cmd: DocumentDto
    Cmd-->>Client: 导入结果
    Client-->>UI: 刷新文档与页面列表
```

关键实现位置：

- `src/lib/tauriClient.ts`
- `src-tauri/src/commands/import_commands.rs`
- `src-tauri/src/services/import_service.rs`
- `src-tauri/src/providers/pdf_renderer.rs`
- `src-tauri/src/providers/libreoffice_converter.rs`
- `src-tauri/src/artifacts/jsonl_exporter.rs`

## 4. 页面分析

```mermaid
sequenceDiagram
    autonumber
    actor User as 用户
    participant UI as React 前端
    participant Cmd as analysis_commands
    participant Analysis as AnalysisService
    participant Settings as SettingsService
    participant Provider as ModelProvider
    participant Validator as schema_validator
    participant Repo as AnalysisRepository
    participant PageRepo as DocumentRepository
    participant JSONL as ArtifactExporter

    User->>UI: 点击分析新页面/单页分析/重新分析
    UI->>Cmd: analyze_page/analyze_new_pages/reanalyze_document
    Cmd->>Analysis: 执行业务分析
    Analysis->>Settings: get_model_configuration_status()
    Settings-->>Analysis: provider/model/key/privacy 状态
    Analysis->>PageRepo: 标记 page 为 analysis_pending
    Analysis->>Analysis: 读取并压缩页面图片
    Analysis->>Provider: analyze_page(image + prompt)
    Provider-->>Analysis: raw model response
    Analysis->>Validator: validate_page_analysis_v1()
    alt JSON 合规
        Validator-->>Analysis: PageAnalysisV1
    else JSON 可修复
        Analysis->>Provider: repair prompt retry
        Provider-->>Analysis: repaired response
        Analysis->>Validator: 再次校验
    else 只能回退文本
        Analysis->>Analysis: fallback_text_analysis()
    end
    Analysis->>Repo: save_success_result 或 save_failure_result
    Analysis->>PageRepo: 更新 page status analyzed/failed
    Analysis->>JSONL: export_all()
    Analysis-->>Cmd: AnalysisResultDto 或 BatchResultDto
    Cmd-->>UI: 刷新页面分析状态
```

关键实现位置：

- `src-tauri/src/services/analysis_service.rs`
- `src-tauri/src/providers/model/*_provider.rs`
- `src-tauri/src/providers/model/schema_validator.rs`
- `src-tauri/src/domain/analysis.rs`

## 5. 索引构建与搜索

```mermaid
sequenceDiagram
    autonumber
    actor User as 用户/脚本
    participant UI as React 或 Localhost API
    participant Cmd as search_commands/api endpoints
    participant Search as SearchService
    participant Repo as AnalysisRepository/IndexRepository
    participant Tantivy as TantivyBm25SearchProvider
    participant Files as indexes/bm25/

    User->>UI: 点击构建索引或 POST /indexes/rebuild
    UI->>Cmd: start_index_rebuild()
    Cmd->>Search: start_index_rebuild()
    Search->>Repo: recover stale building versions
    Search->>Repo: collect current succeeded analyses
    Search->>Repo: create_build_version(version_id)
    Search-->>Cmd: job_id + version_id
    Search->>Search: 后台线程 run_index_rebuild()
    Search->>Tantivy: build_index(build-{version_id}, docs)
    Tantivy-->>Search: ProviderBuildStats
    Search->>Tantivy: health_check + sample search
    Search->>Repo: mark_version_ready + set_active_version
    Search->>Files: write active.json

    User->>UI: 搜索关键词
    UI->>Cmd: search_pages 或 GET /search
    Cmd->>Search: search(query, limit)
    Search->>Repo: find_active_version()
    Search->>Tantivy: search(active index)
    Tantivy-->>Search: page_id + score
    Search->>Repo: assemble page/document/analysis details
    Search-->>Cmd: SearchResponseDto
    Cmd-->>UI: 展示结果和页面 JSON
```

关键实现位置：

- `src-tauri/src/services/search_service.rs`
- `src-tauri/src/providers/search/tantivy_bm25_provider.rs`
- `src-tauri/src/repositories/index_repository.rs`
- `src-tauri/src/api/endpoints.rs`

## 6. Localhost API 启停与认证

```mermaid
sequenceDiagram
    autonumber
    actor User as 用户
    participant SettingsUI as 设置页
    participant Cmd as settings/api commands
    participant Settings as SettingsService
    participant API as ApiServerService
    participant Axum as Axum Router
    participant Auth as BearerAuth
    participant Search as SearchService

    User->>SettingsUI: 启用 API / 修改端口 / 重置 token
    SettingsUI->>Cmd: save_app_settings 或 reset_api_token
    Cmd->>Settings: save_settings()
    Settings->>API: reconcile(settings)
    alt API disabled
        API->>API: stop()
    else enabled and target changed
        API->>API: stop old server if needed
        API->>Axum: start 127.0.0.1:{port}
    end
    User->>Axum: GET /health
    Axum-->>User: workspace/index status
    User->>Axum: POST /indexes/rebuild + Bearer token
    Axum->>Auth: validate Authorization header
    Auth-->>Axum: ok
    Axum->>Search: start_index_rebuild()
    Search-->>Axum: job_id/version_id
```

关键实现位置：

- `src-tauri/src/services/api_server_service.rs`
- `src-tauri/src/api/server.rs`
- `src-tauri/src/api/auth.rs`
- `src-tauri/src/commands/api_commands.rs`
- `src-tauri/src/commands/settings_commands.rs`

