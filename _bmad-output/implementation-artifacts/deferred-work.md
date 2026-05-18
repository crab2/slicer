# Deferred Work

Collected during story 1.4 review (2026-05-18).

## Frontend Async Safety

- **refreshJobs 竞态**: 快速切换工作区时，多个并发 refreshJobs 可能以错误顺序写入 state。需要 generation counter 或 AbortController。
- **useEffect cleanup**: 组件卸载时 in-flight 的异步调用仍会执行 setState。需要 cleanup 函数忽略过期结果。
- **recoveredWorkspaceRef 时机**: ref 在 recoverInterruptedJobs 调用前设置，若失败则该工作区永远跳过恢复。应移至 try 成功后。

## Backend Robustness

- **progress 边界校验**: Rust 端 update_job_progress 接受 u8 (0-255)，SQL CHECK 仅限 0-100。orchestrator 层应 clamp 到 0-100。
- **recover_interrupted_jobs 非原子**: 循环中单个 UPDATE 失败会导致部分恢复。考虑事务或逐条容错。
- **job_from_row 脆弱**: 单行数据损坏会导致整个 list_jobs 失败。考虑跳过坏行并记录警告。
