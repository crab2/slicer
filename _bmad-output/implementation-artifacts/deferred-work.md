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
