# Deferred Work

## Deferred from: workbench UI polish review (2026-06-09)

- `src/features/workbench/components/JobList.tsx` 仍保留“创建示例任务”和“不会触发真实业务处理”的 placeholder 文案。该问题来自既有任务列表，不属于本次工作台导入接收/资产摘要/页面预览 slice，但会削弱工作台的真实产品感，后续应单独移除或改为真实诊断/恢复入口。

## Deferred from: code review of 3-5-分析失败处理-单页重试与安全诊断.md (2026-05-19)

- 工作台 `globals.css` 将 `body`/`.app-shell` 设为 `overflow: hidden`，可能影响小屏或长内容滚动；属布局侧效应，非 3.5 诊断核心。
- Story 核心实现文件 `analysis_service.rs`、`DocumentList.tsx` 仍为未跟踪状态，合并前应纳入 git。

Collected during story 1.4 review (2026-05-18).

## Frontend Async Safety

- **refreshJobs 竞态**: 快速切换工作区时，多个并发 refreshJobs 可能以错误顺序写入 state。需要 generation counter 或 AbortController。
- **useEffect cleanup**: 组件卸载时 in-flight 的异步调用仍会执行 setState。需要 cleanup 函数忽略过期结果。
- **recoveredWorkspaceRef 时机**: ref 在 recoverInterruptedJobs 调用前设置，若失败则该工作区永远跳过恢复。应移至 try 成功后。

## Backend Robustness

- **progress 边界校验**: Rust 端 update_job_progress 接受 u8 (0-255)，SQL CHECK 仅限 0-100。orchestrator 层应 clamp 到 0-100。
- **recover_interrupted_jobs 非原子**: 循环中单个 UPDATE 失败会导致部分恢复。考虑事务或逐条容错。
- **job_from_row 脆弱**: 单行数据损坏会导致整个 list_jobs 失败。考虑跳过坏行并记录警告。

## Deferred from: MVP review (2026-05-28)

- **recover_interrupted_jobs 共享 DB 连接**: per-job 容错后，单个 job 的 SQLite 级错误（连接损坏）会影响后续 job。可考虑失败后重建连接。
- **recover_interrupted_jobs 无事务包装**: 每个 job 恢复执行 4 次独立 SQL 操作，大量 job 时性能差。可考虑批量事务。
- **media_exporter 部分导出无回滚**: 导出中途失败时，已复制的文件和目录结构残留，无清理逻辑。
- **extractError/computeAnalysisStats 重复定义**: WorkbenchPage、AnalysisPage、ExportPage 三处相同函数，应提取为共享工具模块。
- **ExportPage 与 WorkbenchPage 导出 UI 重复**: 两处相同的导出面板，应抽取为共享组件。

## Deferred from: structured PDF acceptance review (2026-07-30)

- **queued 任务未被启动恢复处理**：`recover_interrupted_jobs` 只恢复 `running`，若进程在 job 创建后、首次进度更新前退出，`queued` 任务会永久保留并可能持续阻止同类任务重试。
- **共享图片清理存在并发引用竞态**：删除文档时先统计图片引用再删除文件，另一事务可在统计后新增引用，导致仍被引用的共享图片从磁盘移除。
- **PDF renderer 缺少最大页数限制**：渲染入口会按外部 PDF 声明的全部页数处理，恶意或异常超长文档可造成无界 CPU、内存和磁盘消耗。
- **retry 尚未实现旧/新记录完整原子切换**：重试会在替代导入成功前删除旧账本与制品，失败时不能保证旧记录和新记录作为一个整体回滚或切换。
- **旧 page analysis 图片缺失时预览不回退当前 page asset**：旧索引命中优先使用分析快照中的图片路径；该文件缺失时不会查询当前页面记录对应的可用图片资产。
- **后端 base64 预览缺少单文件大小上限**：预览接口在编码前直接读取完整图片文件，异常大的工作区文件可造成高峰内存占用和超大 IPC 响应。
- **结构化 PDF 媒体包导出尚未接入模块 enrichment**：`media_exporter` 仍以历史页面分析和 `image_hash` 为入口；应单独设计结构文本、ODL 图片及视觉 enrichment 的 Markdown/manifest 表达，避免在本次导入链路中仓促改变既有导出格式。
- **页面 HTTP API 尚未返回结构化块**：`/pages/{page_id}` 保持旧 `PageRecordDto` 合约；后续应增加版本化的模块、bbox、enrichment 和 ODL 制品查询接口，而不是破坏现有调用方。
- **文档删除的数据库与制品清理不是跨介质原子操作**：账本提交后若目录删除失败会遗留孤立制品；后续应采用先隔离到回收目录、提交账本后异步清理并保留恢复元数据的流程。
- **索引状态 DTO 仍沿用 page_count 字段名表示内容项数量**：前端已改为“内容项/索引项”显示；后续 API 大版本可迁移为 item_count，并保留旧字段兼容期。
