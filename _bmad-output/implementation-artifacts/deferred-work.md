# Deferred Work

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
