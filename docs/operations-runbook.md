# SLICER 运维与故障处理手册

> 面向本地运行、发布后支持、问题定位与现场交付。最后更新：2026-07-02。

## 1. 运行形态

SLICER 是桌面应用，不依赖中心化后端。绝大多数运行状态保存在用户本机：

- 应用配置目录：Windows 下默认 `%APPDATA%/slicer`，用于 bootstrap、全局 LibreOffice 配置和日志初始化。
- 用户工作区：由用户选择，保存原始文件副本、页面图片、SQLite、JSONL、BM25 索引和工作区日志。
- 系统 keyring：保存 API key 和 provider 活跃密钥内容。
- 远程模型服务：仅在用户发起页面分析时访问，且需要用户配置 provider、model、API key 并确认隐私提示。

## 2. 启动与健康检查

### 2.1 桌面应用启动

开发模式：

```bash
npm run tauri dev
```

生产包本地验证：

```bash
npm run tauri build
```

Windows 常见产物位于：

```text
src-tauri/target/release/bundle/
```

### 2.2 Localhost API 健康检查

如果用户在设置页启用了 API：

```bash
curl "http://127.0.0.1:17321/health"
```

预期返回：

```json
{
  "data": {
    "api_version": "0.1.0",
    "workspace": {
      "status": "ready"
    }
  }
}
```

如果工作区 ready 且索引状态可读取，响应中会包含 `index` 字段。

## 3. 关键文件定位

| 目标 | 路径 |
| --- | --- |
| 工作区 SQLite | `<workspace>/app.db` |
| 页面图片 | `<workspace>/pages/{document_id}/{image_hash}.png` |
| 原始文件副本 | `<workspace>/originals/` |
| 页面 JSONL | `<workspace>/metadata/pages.jsonl` |
| BM25 索引 | `<workspace>/indexes/bm25/` |
| 活跃索引指针 | `<workspace>/indexes/bm25/active.json` |
| 临时文件 | `<workspace>/tmp/` |
| 应用配置 | `%APPDATA%/slicer` 或平台等价目录 |
| CI 发布配置 | `.github/workflows/release.yml` |

## 4. 常用排查命令

```bash
# 前端类型检查和构建
npm run build

# Rust 编译检查
cd src-tauri
cargo check

# Rust 测试
cd src-tauri
cargo test

# 仅验证媒体边界脚本
npm run test:media-boundaries
```

SQLite 快速查看：

```bash
sqlite3 "<workspace>/app.db" ".tables"
sqlite3 "<workspace>/app.db" "select status, count(*) from page_records group by status;"
sqlite3 "<workspace>/app.db" "select status, count(*) from analysis_results group by status;"
sqlite3 "<workspace>/app.db" "select status, count(*) from index_versions group by status;"
```

## 5. 故障处理

### 5.1 工作区不可用

现象：

- 应用提示未选择工作区、工作区丢失、路径不是目录、无写入权限。
- API `/health` 中 `workspace.status` 不是 `ready`。

处理：

1. 确认路径存在且是目录。
2. 确认当前用户有创建文件和目录权限。
3. 检查是否能手动创建临时文件。
4. 让用户在设置页或工作台重新选择工作区。
5. 如工作区移动过，重新选择新路径，应用会重新写入最近工作区配置。

相关错误码：

- `workspace_missing`
- `workspace_not_directory`
- `workspace_not_writable`
- `workspace_not_ready`
- `workspace_app_db_failed`

### 5.2 PDF 或图片导入失败

现象：

- 导入任务失败，文档状态为 `failed`。
- `page_records` 未生成或页面目录为空。

处理：

1. 确认文件存在且扩展名受支持。
2. PDF 先用普通阅读器打开，排除损坏或加密文件。
3. 图片确认能被系统图片查看器打开。
4. 清理 `<workspace>/tmp/` 中残留临时文件后重试。
5. 使用界面中的重试导入，或删除失败文档后重新导入。

相关错误码：

- `file_not_found`
- `unsupported_file_type`
- `pdf_page_count_failed`
- `pdf_render_failed`
- `pdf_empty_document`
- `image_decode_failed`
- `page_write_failed`
- `page_rename_failed`

### 5.3 Office 导入失败

现象：

- DOC、DOCX、PPT、PPTX 导入失败。
- 错误阶段通常为 `conversion` 或 `import`。

处理：

1. 安装 LibreOffice。
2. 在设置页填写 LibreOffice `program` 目录或 `soffice.exe` 完整路径。
3. Windows 常见路径：

```text
C:/Program Files/LibreOffice/program
C:/Program Files/LibreOffice/program/soffice.exe
```

4. 用命令行确认 `soffice --headless` 可执行。
5. 检查文件是否需要密码、是否损坏、是否被其他程序锁定。

相关错误码：

- `libreoffice_not_configured`
- `conversion_failed`
- `pdf_page_count_failed`
- `pdf_render_failed`

### 5.4 模型分析失败

现象：

- 页面状态为 `failed`。
- `analysis_results` 存在 `failed` 记录。
- 页面卡在 `analysis_pending`。

处理：

1. 检查设置页模型 provider、model name、endpoint、API key。
2. 确认 API key 已保存到系统 keyring。
3. 确认用户已接受隐私提示。
4. 检查模型服务是否支持图像输入。
5. 对单页执行重试；如果是异常退出导致 `analysis_pending`，执行恢复入口或重启应用后让恢复逻辑标记失败以便重试。
6. 如果模型返回非 JSON，系统会自动尝试一次修复；仍失败时查看 `analysis_json_invalid` 等错误。

相关错误码：

- `model_configuration_incomplete`
- `privacy_notice_required`
- `model_provider_unsupported`
- `model_request_failed`
- `analysis_json_invalid`
- `analysis_field_missing`
- `analysis_page_id_mismatch`
- `analysis_image_hash_mismatch`
- `page_analysis_already_running`
- `page_analysis_interrupted`

### 5.5 搜索或索引不可用

现象：

- 搜索提示索引未建立。
- 索引状态为 `not_built`、`building`、`failed` 或 `needs_rebuild`。
- 搜索结果缺失新分析页面。

处理：

1. 确认存在成功分析页面：

```sql
select count(*) from analysis_results where status = 'succeeded';
```

2. 在“BM25 索引”页点击构建或重建。
3. 如果旧索引可用但有新页面，状态会显示 stale，搜索仍可能使用旧索引。
4. 如果构建失败，查看 `index_versions.error_id` 关联的 `errors`。
5. 如索引目录损坏，可删除 `<workspace>/indexes/bm25/build-*` 后重新构建；不要直接手改 `app.db`，除非先备份。

相关错误码：

- `index_not_ready`
- `index_no_documents`
- `index_version_missing`
- `index_path_invalid`
- `search_result_missing_analysis`
- `search_preview_image_outside_workspace`

### 5.6 Localhost API 不可用

现象：

- `/health` 无法连接。
- 设置页 API 状态为 `failed`。
- 端口被占用。

处理：

1. 确认设置页已启用 API。
2. 确认监听地址为 `127.0.0.1`。
3. 更换端口或释放占用端口。
4. 调用设置页中的 token 重置后重试受保护端点。
5. 确认 `POST /indexes/rebuild` 带有 `Authorization: Bearer <token>`。

相关错误码：

- `api_server_disabled`
- `api_server_port_in_use`
- `api_server_bind_failed`
- `api_server_already_running`
- `missing_authorization`
- `invalid_token`
- `api_token_not_configured`

## 6. 数据恢复建议

恢复原则：

- 先备份整个工作区，再做修复。
- `originals/`、`pages/`、`app.db` 是最重要的数据。
- `metadata/pages.jsonl` 可以从 SQLite 重新导出。
- BM25 索引可以从成功分析结果重建。

备份命令示例：

```powershell
Copy-Item -Recurse -LiteralPath "<workspace>" -Destination "<workspace>-backup-20260702"
```

最小可恢复集：

```text
app.db
originals/
pages/
```

如只丢失 `indexes/bm25/`，重新构建索引即可。

如只丢失 `metadata/pages.jsonl`，触发导入、分析或导出流程可刷新；必要时可增加维护命令调用 `ArtifactExporter::export_all`。

## 7. 发布前运维检查

发布前至少确认：

1. `npm ci` 或 `npm install` 成功。
2. `npm run build` 成功。
3. `cd src-tauri && cargo test` 成功。
4. `npm run test:media-boundaries` 成功。
5. `src-tauri/tauri.conf.json` 与 `package.json` 版本一致。
6. `.github/workflows/release.yml` 矩阵仍符合目标平台。
7. Windows 产物可安装并启动。
8. 首次运行可选择工作区。
9. PDF 或图片导入、页面分析、索引构建和搜索主链路可走通。
10. 如果开启 API，`GET /health` 与 `POST /indexes/rebuild` 的认证行为符合预期。

## 8. 现场交付快速话术

- “SLICER 默认本地优先，文档和索引保存在您选择的本地工作区。”
- “只有执行模型分析时，页面图片才会发送到您配置的模型服务。”
- “API key 保存在系统密钥存储，不写入普通配置文件。”
- “Office 导入需要本机 LibreOffice；PDF 和图片导入不依赖 LibreOffice。”
- “搜索依赖先分析页面，再构建 BM25 索引。”

