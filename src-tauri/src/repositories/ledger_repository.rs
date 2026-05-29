use crate::artifacts::workspace_layout::WorkspaceLayout;
use crate::domain::job::{JobDto, JobStatus};
use crate::errors::{AppError, AppResult};
use crate::repositories::db::{block_on_db, connect_workspace_db, database_error, run_migrations};
use chrono::Utc;
use sqlx::Row;
use uuid::Uuid;

const INTERRUPTED_JOB_SUMMARY: &str = "应用上次关闭时任务仍在运行，已标记为可恢复失败状态。";

pub struct LedgerRepository {
    layout: WorkspaceLayout,
}

impl LedgerRepository {
    pub fn new(layout: WorkspaceLayout) -> Self {
        Self { layout }
    }

    pub fn run_initial_migrations(&self) -> AppResult<()> {
        block_on_db(run_migrations(self.layout.app_db_path()))
    }

    pub fn append_job(&self, job_type: &str) -> AppResult<JobDto> {
        let now = Utc::now().to_rfc3339();
        let job = JobDto {
            job_id: Uuid::new_v4().to_string(),
            job_type: job_type.to_string(),
            status: JobStatus::Queued.as_str().to_string(),
            progress: 0,
            created_at: now.clone(),
            updated_at: now.clone(),
            error_id: None,
            error_summary: None,
            last_event_message: Some("任务已创建，等待后续执行。".to_string()),
        };

        block_on_db(async {
            run_migrations(self.layout.app_db_path()).await?;
            let mut connection = connect_workspace_db(self.layout.app_db_path()).await?;
            sqlx::query(
                "INSERT INTO jobs (
                   job_id, job_type, status, progress, created_at, updated_at, error_id, error_summary
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )
            .bind(&job.job_id)
            .bind(&job.job_type)
            .bind(&job.status)
            .bind(i64::from(job.progress))
            .bind(&job.created_at)
            .bind(&job.updated_at)
            .bind(&job.error_id)
            .bind(&job.error_summary)
            .execute(&mut connection)
            .await
            .map_err(|err| database_error("ledger", "job_insert_failed", err))?;

            insert_job_event(
                &mut connection,
                &job.job_id,
                "created",
                job.last_event_message.as_deref(),
                Some(0),
            )
            .await?;
            Ok(job)
        })
    }

    pub fn list_jobs(&self) -> AppResult<Vec<JobDto>> {
        block_on_db(async {
            run_migrations(self.layout.app_db_path()).await?;
            let mut connection = connect_workspace_db(self.layout.app_db_path()).await?;
            let rows = sqlx::query(
                "SELECT
                   jobs.job_id,
                   jobs.job_type,
                   jobs.status,
                   jobs.progress,
                   jobs.created_at,
                   jobs.updated_at,
                   jobs.error_id,
                   jobs.error_summary,
                   (
                     SELECT job_events.message
                     FROM job_events
                     WHERE job_events.job_id = jobs.job_id
                     ORDER BY job_events.created_at DESC
                     LIMIT 1
                   ) AS last_event_message
                 FROM jobs
                 ORDER BY jobs.updated_at DESC, jobs.created_at DESC",
            )
            .fetch_all(&mut connection)
            .await
            .map_err(|err| database_error("ledger", "jobs_read_failed", err))?;

            let jobs: Vec<JobDto> = rows
                .into_iter()
                .filter_map(|row| match job_from_row(row) {
                    Ok(job) => Some(job),
                    Err(err) => {
                        eprintln!("WARN: skipping corrupted job row: {}", err);
                        None
                    }
                })
                .collect();
            Ok(jobs)
        })
    }

    pub fn update_job_progress(
        &self,
        job_id: &str,
        progress: u8,
        message: Option<&str>,
    ) -> AppResult<JobDto> {
        block_on_db(async {
            run_migrations(self.layout.app_db_path()).await?;
            let mut connection = connect_workspace_db(self.layout.app_db_path()).await?;
            let now = Utc::now().to_rfc3339();
            let progress = progress.min(100);
            let status = if progress >= 100 {
                JobStatus::Succeeded.as_str()
            } else {
                JobStatus::Running.as_str()
            };

            let affected = sqlx::query(
                "UPDATE jobs
                 SET status = ?1, progress = ?2, updated_at = ?3
                 WHERE job_id = ?4",
            )
            .bind(status)
            .bind(i64::from(progress))
            .bind(now)
            .bind(job_id)
            .execute(&mut connection)
            .await
            .map_err(|err| database_error("ledger", "job_progress_update_failed", err))?
            .rows_affected();

            if affected == 0 {
                return Err(job_not_found(job_id));
            }

            insert_job_event(
                &mut connection,
                job_id,
                "progress_updated",
                message,
                Some(progress),
            )
            .await?;
            fetch_job(&mut connection, job_id).await
        })
    }

    pub fn mark_job_failed(
        &self,
        job_id: &str,
        error: &AppError,
        summary: &str,
    ) -> AppResult<JobDto> {
        block_on_db(async {
            run_migrations(self.layout.app_db_path()).await?;
            let mut connection = connect_workspace_db(self.layout.app_db_path()).await?;
            let error_id = insert_error(&mut connection, error).await?;
            let now = Utc::now().to_rfc3339();
            let affected = sqlx::query(
                "UPDATE jobs
                 SET status = ?1, updated_at = ?2, error_id = ?3, error_summary = ?4
                 WHERE job_id = ?5",
            )
            .bind(JobStatus::Failed.as_str())
            .bind(now)
            .bind(&error_id)
            .bind(summary)
            .bind(job_id)
            .execute(&mut connection)
            .await
            .map_err(|err| database_error("ledger", "job_failure_update_failed", err))?
            .rows_affected();

            if affected == 0 {
                return Err(job_not_found(job_id));
            }

            insert_job_event(&mut connection, job_id, "failed", Some(summary), None).await?;
            fetch_job(&mut connection, job_id).await
        })
    }

    pub fn recover_interrupted_jobs(&self) -> AppResult<Vec<JobDto>> {
        block_on_db(async {
            run_migrations(self.layout.app_db_path()).await?;
            let mut connection = connect_workspace_db(self.layout.app_db_path()).await?;
            let running_ids = sqlx::query_scalar::<_, String>(
                "SELECT job_id FROM jobs WHERE status = ?1 ORDER BY updated_at ASC",
            )
            .bind(JobStatus::Running.as_str())
            .fetch_all(&mut connection)
            .await
            .map_err(|err| database_error("ledger", "running_jobs_read_failed", err))?;

            let mut recovered = Vec::new();
            for job_id in running_ids {
                let result = async {
                    let error = AppError::new(
                        "job_interrupted",
                        INTERRUPTED_JOB_SUMMARY,
                        "job_recovery",
                        true,
                    );
                    let error_id = insert_error(&mut connection, &error).await?;
                    let now = Utc::now().to_rfc3339();
                    sqlx::query(
                        "UPDATE jobs
                         SET status = ?1, updated_at = ?2, error_id = ?3, error_summary = ?4
                         WHERE job_id = ?5",
                    )
                    .bind(JobStatus::Failed.as_str())
                    .bind(now)
                    .bind(error_id)
                    .bind(INTERRUPTED_JOB_SUMMARY)
                    .bind(&job_id)
                    .execute(&mut connection)
                    .await
                    .map_err(|err| database_error("ledger", "job_recovery_update_failed", err))?;

                    insert_job_event(
                        &mut connection,
                        &job_id,
                        "recovered_as_failed",
                        Some(INTERRUPTED_JOB_SUMMARY),
                        None,
                    )
                    .await?;
                    fetch_job(&mut connection, &job_id).await
                }
                .await;

                match result {
                    Ok(job) => recovered.push(job),
                    Err(err) => {
                        eprintln!(
                            "WARN: failed to recover job {}: {}",
                            job_id, err
                        );
                    }
                }
            }

            Ok(recovered)
        })
    }

    pub fn record_error(&self, error: &AppError) -> AppResult<()> {
        self.record_error_with_id(error).map(|_| ())
    }

    pub fn record_error_with_id(&self, error: &AppError) -> AppResult<String> {
        block_on_db(async {
            run_migrations(self.layout.app_db_path()).await?;
            let mut connection = connect_workspace_db(self.layout.app_db_path()).await?;
            insert_error(&mut connection, error).await
        })
    }
}

async fn insert_error(
    connection: &mut sqlx::SqliteConnection,
    error: &AppError,
) -> AppResult<String> {
    let error_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO errors (
           error_id, code, message, stage, retryable, details, correlation_id, created_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )
    .bind(&error_id)
    .bind(&error.code)
    .bind(&error.message)
    .bind(&error.stage)
    .bind(if error.retryable { 1_i64 } else { 0_i64 })
    .bind(&error.details)
    .bind(&error.correlation_id)
    .bind(Utc::now().to_rfc3339())
    .execute(connection)
    .await
    .map_err(|err| database_error("ledger", "error_insert_failed", err))?;
    Ok(error_id)
}

async fn insert_job_event(
    connection: &mut sqlx::SqliteConnection,
    job_id: &str,
    event_type: &str,
    message: Option<&str>,
    progress: Option<u8>,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO job_events (event_id, job_id, event_type, message, progress, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(job_id)
    .bind(event_type)
    .bind(message)
    .bind(progress.map(i64::from))
    .bind(Utc::now().to_rfc3339())
    .execute(connection)
    .await
    .map_err(|err| database_error("ledger", "job_event_insert_failed", err))?;
    Ok(())
}

async fn fetch_job(connection: &mut sqlx::SqliteConnection, job_id: &str) -> AppResult<JobDto> {
    let row = sqlx::query(
        "SELECT
           jobs.job_id,
           jobs.job_type,
           jobs.status,
           jobs.progress,
           jobs.created_at,
           jobs.updated_at,
           jobs.error_id,
           jobs.error_summary,
           (
             SELECT job_events.message
             FROM job_events
             WHERE job_events.job_id = jobs.job_id
             ORDER BY job_events.created_at DESC
             LIMIT 1
           ) AS last_event_message
         FROM jobs
         WHERE jobs.job_id = ?1",
    )
    .bind(job_id)
    .fetch_optional(connection)
    .await
    .map_err(|err| database_error("ledger", "job_read_failed", err))?;

    row.map(job_from_row)
        .unwrap_or_else(|| Err(job_not_found(job_id)))
}

fn job_from_row(row: sqlx::sqlite::SqliteRow) -> AppResult<JobDto> {
    let progress_i64: i64 = row
        .try_get("progress")
        .map_err(|err| database_error("ledger", "job_progress_read_failed", err))?;
    let progress = u8::try_from(progress_i64).map_err(|_| {
        AppError::new(
            "job_progress_invalid",
            "任务进度记录无效，请检查工作区账本。",
            "ledger",
            true,
        )
        .with_details(progress_i64.to_string())
    })?;

    Ok(JobDto {
        job_id: row
            .try_get("job_id")
            .map_err(|err| database_error("ledger", "job_id_read_failed", err))?,
        job_type: row
            .try_get("job_type")
            .map_err(|err| database_error("ledger", "job_type_read_failed", err))?,
        status: row
            .try_get("status")
            .map_err(|err| database_error("ledger", "job_status_read_failed", err))?,
        progress,
        created_at: row
            .try_get("created_at")
            .map_err(|err| database_error("ledger", "job_created_at_read_failed", err))?,
        updated_at: row
            .try_get("updated_at")
            .map_err(|err| database_error("ledger", "job_updated_at_read_failed", err))?,
        error_id: row
            .try_get("error_id")
            .map_err(|err| database_error("ledger", "job_error_id_read_failed", err))?,
        error_summary: row
            .try_get::<Option<String>, _>("error_summary")
            .map_err(|err| database_error("ledger", "job_error_summary_read_failed", err))?
            .map(normalize_legacy_job_text),
        last_event_message: row
            .try_get::<Option<String>, _>("last_event_message")
            .map_err(|err| database_error("ledger", "job_last_event_read_failed", err))?
            .map(normalize_legacy_job_text),
    })
}

fn normalize_legacy_job_text(value: String) -> String {
    value
        .replace("椤甸潰鍒嗘瀽瀹屾垚", "页面分析完成")
        .replace("鎵归噺鍒嗘瀽瀹屾垚", "批量分析完成")
        .replace("璇婃柇缂栧彿锛歿}", "诊断编号: ")
        .replace("鐠囧﹥鏌囩紓鏍у娇閿涙}", "诊断编号: ")
}

fn job_not_found(job_id: &str) -> AppError {
    AppError::new(
        "job_not_found",
        "未找到指定任务，请刷新任务列表后重试。",
        "job",
        true,
    )
    .with_details(job_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::LedgerRepository;
    use crate::artifacts::workspace_layout::WorkspaceLayout;
    use crate::domain::job::JobStatus;
    use crate::errors::AppError;
    use crate::repositories::db::{block_on_db, connect_workspace_db, database_error};
    use std::fs;

    #[test]
    fn ledger_writes_jobs_and_errors_to_sqlite_not_json_sidecars() {
        let (root, layout, repository) = test_repository("slicer-ledger");

        let job = repository.append_job("diagnostic").expect("job insert");
        assert_eq!(job.status, "queued");
        assert_eq!(repository.list_jobs().expect("jobs").len(), 1);

        let error = AppError::new("demo_error", "演示错误", "test", true);
        repository.record_error(&error).expect("error insert");

        assert!(!root.join("jobs.json").exists());
        assert!(!root.join("errors.json").exists());

        block_on_db(async {
            let mut connection = connect_workspace_db(layout.app_db_path()).await?;
            let error_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM errors")
                .fetch_one(&mut connection)
                .await
                .map_err(|err| database_error("test", "error_count_failed", err))?;
            assert_eq!(error_count, 1);
            Ok(())
        })
        .expect("sqlite check");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn job_orchestration_updates_progress_and_records_events() {
        let (root, layout, repository) = test_repository("slicer-job-progress");
        let job = repository.append_job("placeholder_import").expect("job");

        let running = repository
            .update_job_progress(&job.job_id, 45, Some("已处理 45%"))
            .expect("progress");
        assert_eq!(running.status, "running");
        assert_eq!(running.progress, 45);
        assert_eq!(running.last_event_message.as_deref(), Some("已处理 45%"));

        let completed = repository
            .update_job_progress(&job.job_id, 100, Some("任务完成"))
            .expect("completed");
        assert_eq!(completed.status, "succeeded");
        assert_eq!(completed.progress, 100);

        block_on_db(async {
            let mut connection = connect_workspace_db(layout.app_db_path()).await?;
            let event_count =
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM job_events WHERE job_id = ?1")
                    .bind(job.job_id)
                    .fetch_one(&mut connection)
                    .await
                    .map_err(|err| database_error("test", "event_count_failed", err))?;
            assert_eq!(event_count, 3);
            Ok(())
        })
        .expect("event check");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn list_jobs_normalizes_legacy_mojibake_messages() {
        let (root, _layout, repository) = test_repository("slicer-job-mojibake");
        let job = repository.append_job("page_analysis").expect("job");
        repository
            .update_job_progress(&job.job_id, 100, Some("椤甸潰鍒嗘瀽瀹屾垚"))
            .expect("completed");

        let jobs = repository.list_jobs().expect("jobs");
        assert_eq!(jobs[0].last_event_message.as_deref(), Some("页面分析完成"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn job_orchestration_marks_failed_with_structured_error() {
        let (root, _layout, repository) = test_repository("slicer-job-failed");
        let job = repository.append_job("placeholder_analysis").expect("job");
        let error = AppError::new("analysis_failed", "分析任务失败", "analysis", true);

        let failed = repository
            .mark_job_failed(&job.job_id, &error, "分析任务失败")
            .expect("failed");
        assert_eq!(failed.status, "failed");
        assert!(failed.error_id.is_some());
        assert_eq!(failed.error_summary.as_deref(), Some("分析任务失败"));
        assert_eq!(failed.last_event_message.as_deref(), Some("分析任务失败"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn recovers_interrupted_running_jobs_as_failed() {
        let (root, _layout, repository) = test_repository("slicer-job-recover");
        let job = repository.append_job("placeholder_index").expect("job");
        repository
            .update_job_progress(&job.job_id, 30, Some("索引构建中"))
            .expect("running");

        let recovered = repository
            .recover_interrupted_jobs()
            .expect("recover interrupted jobs");
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].status, JobStatus::Failed.as_str());
        assert_eq!(
            recovered[0].error_summary.as_deref(),
            Some(super::INTERRUPTED_JOB_SUMMARY)
        );

        let recovered_again = repository
            .recover_interrupted_jobs()
            .expect("recover again");
        assert!(recovered_again.is_empty());

        let _ = fs::remove_dir_all(root);
    }

    fn test_repository(name: &str) -> (std::path::PathBuf, WorkspaceLayout, LedgerRepository) {
        let root = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("temp root");
        let layout = WorkspaceLayout::from_root(root.clone());
        layout.ensure_base_layout().expect("layout");
        let repository = LedgerRepository::new(layout.clone());
        (root, layout, repository)
    }
}
