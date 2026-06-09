# API 合约 — SLICER Localhost API

## 概述

SLICER 内嵌了一个 **Axum 0.8** HTTP API 服务器，监听 localhost，供外部工具和脚本访问应用数据。API 默认受 **Bearer Token** 认证保护。

---

## 基础信息

| 属性 | 值 |
|------|-----|
| 协议 | HTTP/1.1 |
| 绑定地址 | `127.0.0.1:{port}`（用户可配置） |
| 数据格式 | JSON |
| 认证方式 | `Authorization: Bearer <token>` |
| API 版本 | 0.1.0（与项目版本一致） |

## 认证

所有受保护端点需要 `Authorization: Bearer <token>` 请求头。Token 在应用设置中生成，存储在平台的加密密钥链（keyring）中。

**认证错误响应（401）：**
```json
{
  "error": {
    "code": "invalid_token",
    "message": "API token 无效。"
  }
}
```

## 统一响应格式

### 成功响应
```json
{
  "data": T
}
```
- `GET` 返回 `200 OK`
- `POST` 返回 `201 Created`

### 错误响应
```json
{
  "error": {
    "code": "string",
    "message": "string",
    "stage": "api",
    "retryable": true,
    "details": "string|null",
    "correlation_id": "uuid"
  }
}
```

HTTP 状态码映射：
- `api_server_*` → `503 Service Unavailable`
- `*_not_found` / `not_found` → `404 Not Found`
- `validation_*` → `400 Bad Request`
- 其他 → `500 Internal Server Error`

---

## 端点

### 1. `GET /health` — 健康检查

**认证：** 无需

**响应：**
```json
{
  "data": {
    "api_version": "0.1.0",
    "workspace": {
      "status": "ready",
      "workspace_path": "/path/to/workspace"
    },
    "index": {
      "provider": "tantivy_bm25",
      "status": "ready",
      "document_count": 42,
      "activated_at": "2026-05-28T10:00:00+08:00"
    }
  }
}
```

- `workspace` 始终存在
- `index` 仅在工作区 `ready` 且索引状态可获取时返回

---

### 2. `GET /search` — 全文搜索

**认证：** 无需

**查询参数：**

| 参数 | 类型 | 必需 | 默认值 | 说明 |
|------|------|------|--------|------|
| `q` | string | ✅ | — | 搜索关键词 |
| `limit` | integer | ❌ | 20 | 返回结果数上限 |

**响应：**
```json
{
  "data": {
    "results": [
      {
        "page_id": "uuid",
        "document_id": "uuid",
        "document_filename": "report.pdf",
        "page_number": 3,
        "score": 0.852,
        "snippet": "匹配内容的上下文片段..."
      }
    ],
    "total_hits": 15,
    "query": "搜索词"
  }
}
```

---

### 3. `GET /pages/{page_id}` — 获取页面详情

**认证：** 无需

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `page_id` | UUID | 页面唯一标识 |

**响应（200）：**
```json
{
  "data": {
    "page_id": "uuid",
    "document_id": "uuid",
    "page_number": 3,
    "status": "analyzed",
    "image_hash": "sha256..."
  }
}
```

**错误：**
- `page_not_found` (404) — 页面不存在

---

### 4. `GET /documents/{document_id}` — 获取文档详情

**认证：** 无需

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `document_id` | UUID | 文档唯一标识 |

**响应（200）：**
```json
{
  "data": {
    "document_id": "uuid",
    "original_filename": "report.pdf",
    "file_type": "pdf",
    "page_count": 10,
    "status": "ready",
    "created_at": "2026-05-28T09:00:00+08:00"
  }
}
```

**错误：**
- `document_not_found` (404) — 文档不存在

---

### 5. `POST /indexes/rebuild` — 触发索引重建

**认证：** ✅ 需要 Bearer Token

**响应（201）：**
```json
{
  "data": {
    "version_id": "uuid",
    "status": "building"
  }
}
```

**错误：**
- `missing_authorization` (401) — 未提供认证头
- `invalid_token` (401) — Token 无效
- `api_token_not_configured` (401) — Token 未配置
