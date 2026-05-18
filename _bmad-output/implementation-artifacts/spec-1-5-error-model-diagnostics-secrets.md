---
title: 'Story 1.5: 建立统一错误模型、诊断日志与敏感信息保护'
type: 'feature'
created: '2026-05-18'
status: 'done'
baseline_commit: 'c3e289a'
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** slicer 已有基础 `AppError` 结构体和 `errors` 表，但缺少结构化日志（tracing）、API key 安全存储（keyring）、增强的敏感信息脱敏、诊断日志文件写入，以及前端 correlation_id 展示。当前错误模型可工作但不满足 FR21 和 Story 1.5 的全部安全与诊断要求。

**Approach:** 在现有 `AppError` 基础上增量增强：添加 tracing 依赖和初始化、添加 keyring 依赖用于 API key 安全存储、增强 `redact_secrets` 覆盖 Authorization/Bearer 模式、建立诊断日志写入工作区 `logs/` 目录、在 tracing span 中注入 correlation_id、前端 ErrorMessage 展示 correlation_id。

## Boundaries & Constraints

**Always:**
- `AppError` 必须包含 code, message, stage, retryable, details, correlation_id
- API key 不得出现在 SQLite 普通字段、日志、错误详情、搜索响应或前端持久状态中
- 错误详情中的敏感信息必须经过 redaction
- 字段命名使用 snake_case，时间戳使用 RFC 3339
- 日志通过 tracing 生成，可通过 correlation_id 与错误记录关联

**Ask First:**
- keyring crate 选择（keyring vs platform-specific）
- 日志文件轮转策略

**Never:**
- 不实现真实模型调用或保存完整原始模型响应
- 不在 errors 表或日志中存储明文 API key
- 不在前端回显完整密钥

</frozen-after-approval>

## Code Map

- `src-tauri/src/errors.rs` -- AppError 结构体、redact_secrets 函数
- `src-tauri/src/repositories/ledger_repository.rs` -- record_error 方法
- `src-tauri/src/commands/diagnostics_commands.rs` -- record_diagnostic_error 命令
- `src-tauri/src/domain/settings.rs` -- AppSettingsDto（api_key_configured 字段）
- `src-tauri/src/repositories/settings_repository.rs` -- BootstrapSettings 配置读写
- `src-tauri/src/services/settings_service.rs` -- SettingsService
- `src-tauri/src/lib.rs` -- Tauri 应用构建与插件注册
- `src-tauri/Cargo.toml` -- 依赖声明
- `src/components/common/ErrorMessage.tsx` -- 前端错误展示组件
- `src/types/app.ts` -- AppErrorDto 类型定义

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/Cargo.toml` -- 添加 tracing, tracing-subscriber, tracing-appender, keyring 依赖
- [x] `src-tauri/src/errors.rs` -- 增强 redact_secrets 覆盖 Authorization header 和 Bearer token 模式
- [x] `src-tauri/src/diagnostics/mod.rs` -- 新建诊断模块：tracing 初始化、日志文件写入 logs/ 目录、correlation_id span 注入
- [x] `src-tauri/src/lib.rs` -- 初始化 tracing subscriber，注册诊断模块
- [x] `src-tauri/src/security/mod.rs` -- 新建安全模块：keyring API key 读写接口
- [x] `src-tauri/src/services/settings_service.rs` -- 扩展保存/读取 API key 到 keyring
- [x] `src/components/common/ErrorMessage.tsx` -- 展示 correlation_id 与诊断入口占位
- [x] `src/types/app.ts` -- 确认 AppErrorDto 包含 correlation_id 字段

**Acceptance Criteria:**
- Given Rust service 发生错误, when 错误被转换为 AppError, then 应包含 code, message, stage, retryable, details, correlation_id
- Given 前端收到 Tauri command 错误, when 展示给用户, then 应显示结构化错误对象（非字符串），包含中文摘要和 correlation_id
- Given 工作区已初始化且发生错误, when 错误被记录, then 应写入 errors 账本且可通过 correlation_id 关联
- Given 应用运行, when 写入诊断日志, then 应通过 tracing 生成并写入 logs/ 目录
- Given 用户保存 API key, when 持久化, then 应通过 keyring 保存，不得明文进入 SQLite 或日志
- Given 错误详情包含敏感信息, when 输出, then 应对 api_key, Authorization, token, secret 执行 redaction
- Given 错误模型序列化, when 返回前端或 API, then 字段使用 snake_case

## Verification

**Commands:**
- `cargo test --lib` -- expected: 所有现有测试通过 + 新增 redaction 测试通过
- `npx tsc --noEmit` -- expected: TypeScript 类型检查通过
- `npx vite build` -- expected: Vite 构建通过

## Suggested Review Order

**Dependencies**

- 新增 tracing、tracing-appender、keyring 依赖声明
  [`Cargo.toml`](../../src-tauri/Cargo.toml#L29)

**Secret Redaction**

- 增强 redact_secrets 覆盖 Authorization/Bearer 模式，含 5 个单元测试
  [`errors.rs`](../../src-tauri/src/errors.rs#L58)

**Diagnostics**

- tracing 初始化：daily 文件轮转 + stderr 输出
  [`diagnostics/mod.rs`](../../src-tauri/src/diagnostics/mod.rs#L7)

**Security**

- keyring API key 安全存储接口
  [`security/mod.rs`](../../src-tauri/src/security/mod.rs#L7)

**Tauri Commands**

- 注册诊断/安全模块，初始化 tracing
  [`lib.rs`](../../src-tauri/src/lib.rs#L1)

- save_api_key / delete_api_key 命令
  [`settings_commands.rs`](../../src-tauri/src/commands/settings_commands.rs#L14)

**Settings Service**

- API key 读写委托 security 模块
  [`settings_service.rs`](../../src-tauri/src/services/settings_service.rs#L7)

**Frontend**

- ErrorMessage 展示 correlation_id
  [`ErrorMessage.tsx`](../../src/components/common/ErrorMessage.tsx#L8)

- tauriClient API key 方法
  [`tauriClient.ts`](../../src/lib/tauriClient.ts#L49)
