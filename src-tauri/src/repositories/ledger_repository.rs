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
            sqlx::query("BEGIN IMMEDIATE")
                .execute(&mut connection)
                .await
                .map_err(|err| database_error("ledger", "job_progress_begin_failed", err))?;
            let result = async {
                let affected = sqlx::query(
                    "UPDATE jobs
                     SET status = ?1, progress = ?2, updated_at = ?3
                     WHERE job_id = ?4 AND status IN (?5, ?6) AND ?2 >= progress",
                )
                .bind(status)
                .bind(i64::from(progress))
                .bind(now)
                .bind(job_id)
                .bind(JobStatus::Queued.as_str())
                .bind(JobStatus::Running.as_str())
                .execute(&mut connection)
                .await
                .map_err(|err| database_error("ledger", "job_progress_update_failed", err))?
                .rows_affected();

                if affected == 1 {
                    insert_job_event(
                        &mut connection,
                        job_id,
                        "progress_updated",
                        message,
                        Some(progress),
                    )
                    .await?;
                }
                fetch_job(&mut connection, job_id).await
            }
            .await;
            finish_job_transaction(&mut connection, result, "job_progress_commit_failed").await
        })
    }

    pub fn complete_import_transaction(
        connection: &mut sqlx::SqliteConnection,
        document_id: &str,
        job_id: &str,
        page_count: i64,
        message: &str,
    ) -> AppResult<()> {
        block_on_db(async {
            sqlx::query("BEGIN IMMEDIATE")
                .execute(&mut *connection)
                .await
                .map_err(|err| database_error("import", "import_finalize_begin_failed", err))?;
            let result = async {
                let now = Utc::now().to_rfc3339();
                let document_affected = sqlx::query(
                    "UPDATE documents
                     SET status = 'ready', page_count = ?1, error_summary = NULL, updated_at = ?2
                     WHERE document_id = ?3 AND status = 'importing'",
                )
                .bind(page_count)
                .bind(&now)
                .bind(document_id)
                .execute(&mut *connection)
                .await
                .map_err(|err| database_error("import", "import_document_finalize_failed", err))?
                .rows_affected();
                if document_affected != 1 {
                    return Err(AppError::new(
                        "import_document_state_conflict",
                        "导入文档状态已变化，无法提交完成状态。",
                        "import",
                        true,
                    ));
                }

                let job_affected = sqlx::query(
                    "UPDATE jobs
                     SET status = ?1, progress = 100, updated_at = ?2,
                         error_id = NULL, error_summary = NULL
                     WHERE job_id = ?3 AND status IN (?4, ?5)",
                )
                .bind(JobStatus::Succeeded.as_str())
                .bind(&now)
                .bind(job_id)
                .bind(JobStatus::Queued.as_str())
                .bind(JobStatus::Running.as_str())
                .execute(&mut *connection)
                .await
                .map_err(|err| database_error("import", "import_job_finalize_failed", err))?
                .rows_affected();
                if job_affected != 1 {
                    return Err(AppError::new(
                        "import_job_state_conflict",
                        "导入任务状态已变化，无法提交完成状态。",
                        "import",
                        true,
                    ));
                }

                insert_job_event(
                    &mut *connection,
                    job_id,
                    "progress_updated",
                    Some(message),
                    Some(100),
                )
                .await
            }
            .await;

            match result {
                Ok(()) => match sqlx::query("COMMIT").execute(&mut *connection).await {
                    Ok(_) => Ok(()),
                    Err(err) => {
                        let error = database_error("import", "import_finalize_commit_failed", err);
                        let _ = sqlx::query("ROLLBACK").execute(&mut *connection).await;
                        Err(error)
                    }
                },
                Err(error) => {
                    let _ = sqlx::query("ROLLBACK").execute(&mut *connection).await;
                    Err(error)
                }
            }
        })
    }

    pub fn fail_import_if_active(
        &self,
        document_id: &str,
        job_id: &str,
        error: &AppError,
        summary: &str,
    ) -> AppResult<bool> {
        block_on_db(async {
            run_migrations(self.layout.app_db_path()).await?;
            let mut connection = connect_workspace_db(self.layout.app_db_path()).await?;
            sqlx::query("BEGIN IMMEDIATE")
                .execute(&mut connection)
                .await
                .map_err(|err| database_error("import", "import_failure_begin_failed", err))?;
            let result = async {
                let status =
                    sqlx::query_scalar::<_, String>("SELECT status FROM jobs WHERE job_id = ?1")
                        .bind(job_id)
                        .fetch_optional(&mut connection)
                        .await
                        .map_err(|err| {
                            database_error("import", "import_job_status_read_failed", err)
                        })?;
                let Some(status) = status else {
                    return Err(job_not_found(job_id));
                };
                if status != JobStatus::Queued.as_str() && status != JobStatus::Running.as_str() {
                    return Ok(false);
                }

                let error_id = insert_error(&mut connection, error).await?;
                let now = Utc::now().to_rfc3339();
                let document_affected = sqlx::query(
                    "UPDATE documents
                     SET status = 'failed', error_summary = ?1, updated_at = ?2
                     WHERE document_id = ?3 AND job_id = ?4 AND status = 'importing'",
                )
                .bind(summary)
                .bind(&now)
                .bind(document_id)
                .bind(job_id)
                .execute(&mut connection)
                .await
                .map_err(|err| database_error("import", "import_document_failure_failed", err))?
                .rows_affected();
                if document_affected != 1 {
                    return Err(import_failure_state_conflict(document_id, job_id));
                }

                let job_affected = sqlx::query(
                    "UPDATE jobs
                     SET status = ?1, updated_at = ?2, error_id = ?3, error_summary = ?4
                     WHERE job_id = ?5 AND status IN (?6, ?7)",
                )
                .bind(JobStatus::Failed.as_str())
                .bind(&now)
                .bind(&error_id)
                .bind(summary)
                .bind(job_id)
                .bind(JobStatus::Queued.as_str())
                .bind(JobStatus::Running.as_str())
                .execute(&mut connection)
                .await
                .map_err(|err| database_error("import", "import_job_failure_failed", err))?
                .rows_affected();
                if job_affected != 1 {
                    return Err(import_failure_state_conflict(document_id, job_id));
                }
                insert_job_event(&mut connection, job_id, "failed", Some(summary), None).await?;
                Ok(true)
            }
            .await;

            match result {
                Ok(changed) => match sqlx::query("COMMIT").execute(&mut connection).await {
                    Ok(_) => Ok(changed),
                    Err(err) => {
                        let error = database_error("import", "import_failure_commit_failed", err);
                        let _ = sqlx::query("ROLLBACK").execute(&mut connection).await;
                        Err(error)
                    }
                },
                Err(error) => {
                    let _ = sqlx::query("ROLLBACK").execute(&mut connection).await;
                    Err(error)
                }
            }
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
            sqlx::query("BEGIN IMMEDIATE")
                .execute(&mut connection)
                .await
                .map_err(|err| database_error("ledger", "job_failure_begin_failed", err))?;
            let result = async {
                let status =
                    sqlx::query_scalar::<_, String>("SELECT status FROM jobs WHERE job_id = ?1")
                        .bind(job_id)
                        .fetch_optional(&mut connection)
                        .await
                        .map_err(|err| database_error("ledger", "job_failure_status_failed", err))?
                        .ok_or_else(|| job_not_found(job_id))?;
                if status == JobStatus::Queued.as_str() || status == JobStatus::Running.as_str() {
                    let error_id = insert_error(&mut connection, error).await?;
                    let now = Utc::now().to_rfc3339();
                    let affected = sqlx::query(
                        "UPDATE jobs
                         SET status = ?1, updated_at = ?2, error_id = ?3, error_summary = ?4
                         WHERE job_id = ?5 AND status IN (?6, ?7)",
                    )
                    .bind(JobStatus::Failed.as_str())
                    .bind(now)
                    .bind(&error_id)
                    .bind(summary)
                    .bind(job_id)
                    .bind(JobStatus::Queued.as_str())
                    .bind(JobStatus::Running.as_str())
                    .execute(&mut connection)
                    .await
                    .map_err(|err| database_error("ledger", "job_failure_update_failed", err))?
                    .rows_affected();
                    if affected != 1 {
                        return Err(AppError::new(
                            "job_failure_state_conflict",
                            "任务状态已变化，无法记录失败状态。",
                            "ledger",
                            true,
                        ));
                    }
                    insert_job_event(&mut connection, job_id, "failed", Some(summary), None)
                        .await?;
                }
                fetch_job(&mut connection, job_id).await
            }
            .await;
            finish_job_transaction(&mut connection, result, "job_failure_commit_failed").await
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
                    sqlx::query("BEGIN IMMEDIATE")
                        .execute(&mut connection)
                        .await
                        .map_err(|err| {
                            database_error("ledger", "job_recovery_begin_failed", err)
                        })?;
                    let recovery = async {
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
                        .bind(&now)
                        .bind(&error_id)
                        .bind(INTERRUPTED_JOB_SUMMARY)
                        .bind(&job_id)
                        .execute(&mut connection)
                        .await
                        .map_err(|err| {
                            database_error("ledger", "job_recovery_update_failed", err)
                        })?;

                        sqlx::query(
                            "UPDATE documents
                         SET status = 'failed', error_summary = ?1, updated_at = ?2
                         WHERE job_id = ?3 AND status = 'importing'",
                        )
                        .bind(INTERRUPTED_JOB_SUMMARY)
                        .bind(&now)
                        .bind(&job_id)
                        .execute(&mut connection)
                        .await
                        .map_err(|err| {
                            database_error("ledger", "job_recovery_document_update_failed", err)
                        })?;

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

                    match recovery {
                        Ok(job) => match sqlx::query("COMMIT").execute(&mut connection).await {
                            Ok(_) => Ok(job),
                            Err(err) => {
                                let error =
                                    database_error("ledger", "job_recovery_commit_failed", err);
                                let _ = sqlx::query("ROLLBACK").execute(&mut connection).await;
                                Err(error)
                            }
                        },
                        Err(error) => {
                            let _ = sqlx::query("ROLLBACK").execute(&mut connection).await;
                            Err(error)
                        }
                    }
                }
                .await;

                match result {
                    Ok(job) => recovered.push(job),
                    Err(err) => {
                        eprintln!("WARN: failed to recover job {}: {}", job_id, err);
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

async fn finish_job_transaction(
    connection: &mut sqlx::SqliteConnection,
    result: AppResult<JobDto>,
    commit_code: &str,
) -> AppResult<JobDto> {
    match result {
        Ok(job) => match sqlx::query("COMMIT").execute(&mut *connection).await {
            Ok(_) => Ok(job),
            Err(err) => {
                let error = database_error("ledger", commit_code, err);
                let _ = sqlx::query("ROLLBACK").execute(&mut *connection).await;
                Err(error)
            }
        },
        Err(error) => {
            let _ = sqlx::query("ROLLBACK").execute(&mut *connection).await;
            Err(error)
        }
    }
}

pub(crate) async fn insert_error(
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

fn import_failure_state_conflict(document_id: &str, job_id: &str) -> AppError {
    AppError::new(
        "import_failure_state_conflict",
        "导入文档或任务状态已变化，未写入失败状态。",
        "import",
        true,
    )
    .with_details(format!("document_id={document_id}; job_id={job_id}"))
}

#[cfg(test)]
mod tests {
    use super::LedgerRepository;
    use crate::artifacts::workspace_layout::WorkspaceLayout;
    use crate::domain::job::JobStatus;
    use crate::errors::AppError;
    use crate::repositories::db::{block_on_db, connect_workspace_db, database_error};
    use crate::repositories::document_repository::DocumentRepository;
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
    fn succeeded_job_rejects_failure_and_progress_regression() {
        let (root, layout, repository) = test_repository("slicer-job-terminal-state");
        let job = repository.append_job("index_rebuild").expect("job");
        let completed = repository
            .update_job_progress(&job.job_id, 100, Some("completed"))
            .expect("completed");
        assert_eq!(completed.status, JobStatus::Succeeded.as_str());

        let error = AppError::new("late_failure", "late failure", "test", true);
        let after_failure = repository
            .mark_job_failed(&job.job_id, &error, "late failure")
            .expect("terminal failure ignored");
        let after_progress = repository
            .update_job_progress(&job.job_id, 20, Some("late progress"))
            .expect("terminal progress ignored");

        for result in [&after_failure, &after_progress] {
            assert_eq!(result.status, JobStatus::Succeeded.as_str());
            assert_eq!(result.progress, 100);
            assert!(result.error_id.is_none());
            assert!(result.error_summary.is_none());
            assert_eq!(result.last_event_message.as_deref(), Some("completed"));
        }

        block_on_db(async {
            let mut connection = connect_workspace_db(layout.app_db_path()).await?;
            let (event_count, error_count) = sqlx::query_as::<_, (i64, i64)>(
                "SELECT
                   (SELECT COUNT(*) FROM job_events WHERE job_id = ?1),
                   (SELECT COUNT(*) FROM errors)",
            )
            .bind(&job.job_id)
            .fetch_one(&mut connection)
            .await
            .map_err(|err| database_error("test", "terminal_state_count_failed", err))?;
            assert_eq!(event_count, 2);
            assert_eq!(error_count, 0);
            Ok(())
        })
        .expect("terminal state check");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn running_job_progress_is_monotonic() {
        let (root, layout, repository) = test_repository("slicer-job-monotonic-progress");
        let job = repository.append_job("batch_analysis").expect("job");
        repository
            .update_job_progress(&job.job_id, 80, Some("eighty"))
            .expect("progress 80");

        let after_late_update = repository
            .update_job_progress(&job.job_id, 20, Some("late twenty"))
            .expect("late progress ignored");
        assert_eq!(after_late_update.status, JobStatus::Running.as_str());
        assert_eq!(after_late_update.progress, 80);
        assert_eq!(
            after_late_update.last_event_message.as_deref(),
            Some("eighty")
        );

        block_on_db(async {
            let mut connection = connect_workspace_db(layout.app_db_path()).await?;
            let event_count =
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM job_events WHERE job_id = ?1")
                    .bind(&job.job_id)
                    .fetch_one(&mut connection)
                    .await
                    .map_err(|err| database_error("test", "monotonic_event_count_failed", err))?;
            assert_eq!(event_count, 2);
            Ok(())
        })
        .expect("monotonic progress check");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failed_job_rejects_late_success() {
        let (root, layout, repository) = test_repository("slicer-job-failed-terminal");
        let job = repository.append_job("analysis").expect("job");
        let error = AppError::new("analysis_failed", "failed", "analysis", true);
        let failed = repository
            .mark_job_failed(&job.job_id, &error, "failed")
            .expect("failed");
        assert_eq!(failed.status, JobStatus::Failed.as_str());

        let after_success = repository
            .update_job_progress(&job.job_id, 100, Some("late success"))
            .expect("late success ignored");
        assert_eq!(after_success.status, JobStatus::Failed.as_str());
        assert_eq!(after_success.progress, 0);
        assert_eq!(after_success.error_summary.as_deref(), Some("failed"));

        block_on_db(async {
            let mut connection = connect_workspace_db(layout.app_db_path()).await?;
            let event_count =
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM job_events WHERE job_id = ?1")
                    .bind(&job.job_id)
                    .fetch_one(&mut connection)
                    .await
                    .map_err(|err| database_error("test", "failed_event_count_failed", err))?;
            assert_eq!(event_count, 2);
            Ok(())
        })
        .expect("failed terminal check");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn progress_update_rolls_back_when_event_insert_fails() {
        let (root, layout, repository) = test_repository("slicer-job-progress-rollback");
        let job = repository.append_job("analysis").expect("job");
        let mut connection =
            block_on_db(connect_workspace_db(layout.app_db_path())).expect("connection");
        block_on_db(async {
            sqlx::query(
                "CREATE TRIGGER fail_progress_event
                 BEFORE INSERT ON job_events
                 WHEN NEW.event_type = 'progress_updated'
                 BEGIN
                   SELECT RAISE(ABORT, 'forced progress event failure');
                 END",
            )
            .execute(&mut connection)
            .await
            .map_err(|err| database_error("test", "progress_trigger_create_failed", err))?;
            Ok(())
        })
        .expect("failure trigger");
        drop(connection);

        let failure = repository
            .update_job_progress(&job.job_id, 50, Some("half"))
            .expect_err("event failure");
        assert_eq!(failure.code, "job_event_insert_failed");
        let after = repository.list_jobs().expect("jobs").remove(0);
        assert_eq!(after.status, JobStatus::Queued.as_str());
        assert_eq!(after.progress, 0);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn progress_update_rolls_back_when_commit_fails() {
        let (root, layout, repository) = test_repository("slicer-job-progress-commit");
        let job = repository.append_job("analysis").expect("job");
        let mut connection =
            block_on_db(connect_workspace_db(layout.app_db_path())).expect("connection");
        block_on_db(async {
            sqlx::query(
                "CREATE TABLE deferred_progress_violation (
                   id INTEGER PRIMARY KEY,
                   missing_job_id TEXT NOT NULL,
                   FOREIGN KEY (missing_job_id) REFERENCES jobs(job_id)
                     DEFERRABLE INITIALLY DEFERRED
                 )",
            )
            .execute(&mut connection)
            .await
            .map_err(|err| database_error("test", "progress_violation_table_failed", err))?;
            sqlx::query(
                "CREATE TRIGGER fail_progress_commit
                 AFTER INSERT ON job_events
                 WHEN NEW.event_type = 'progress_updated'
                 BEGIN
                   INSERT INTO deferred_progress_violation (missing_job_id)
                   VALUES ('missing-job');
                 END",
            )
            .execute(&mut connection)
            .await
            .map_err(|err| database_error("test", "progress_commit_trigger_failed", err))?;
            Ok(())
        })
        .expect("commit failure setup");
        drop(connection);

        let failure = repository
            .update_job_progress(&job.job_id, 50, Some("half"))
            .expect_err("commit failure");
        assert_eq!(failure.code, "job_progress_commit_failed");
        let after = repository.list_jobs().expect("jobs").remove(0);
        assert_eq!(after.status, JobStatus::Queued.as_str());
        assert_eq!(after.progress, 0);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failure_record_rolls_back_when_job_update_is_ignored() {
        let (root, layout, repository) = test_repository("slicer-job-failure-conflict");
        let job = repository.append_job("analysis").expect("job");
        let mut connection =
            block_on_db(connect_workspace_db(layout.app_db_path())).expect("connection");
        block_on_db(async {
            sqlx::query(
                "CREATE TRIGGER ignore_job_failure
                 BEFORE UPDATE ON jobs
                 WHEN NEW.status = 'failed'
                 BEGIN
                   SELECT RAISE(IGNORE);
                 END",
            )
            .execute(&mut connection)
            .await
            .map_err(|err| database_error("test", "failure_ignore_trigger_failed", err))?;
            Ok(())
        })
        .expect("ignore trigger");
        drop(connection);

        let error = AppError::new("analysis_failed", "failed", "analysis", true);
        let failure = repository
            .mark_job_failed(&job.job_id, &error, "failed")
            .expect_err("state conflict");
        assert_eq!(failure.code, "job_failure_state_conflict");

        let mut connection =
            block_on_db(connect_workspace_db(layout.app_db_path())).expect("connection");
        let (status, error_count, event_count) = block_on_db(async {
            sqlx::query_as::<_, (String, i64, i64)>(
                "SELECT
                   (SELECT status FROM jobs WHERE job_id = ?1),
                   (SELECT COUNT(*) FROM errors),
                   (SELECT COUNT(*) FROM job_events WHERE job_id = ?1)",
            )
            .bind(&job.job_id)
            .fetch_one(&mut connection)
            .await
            .map_err(|err| database_error("test", "failure_conflict_check_failed", err))
        })
        .expect("failure conflict check");
        assert_eq!(status, JobStatus::Queued.as_str());
        assert_eq!(error_count, 0);
        assert_eq!(event_count, 1);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn import_failure_requires_matching_document_job_and_rolls_back() {
        let (root, layout, repository) = test_repository("slicer-import-failure-owner");
        let requested_job = repository
            .append_job("document_import")
            .expect("requested job");
        let owning_job = repository
            .append_job("document_import")
            .expect("owning job");
        let mut connection =
            block_on_db(connect_workspace_db(layout.app_db_path())).expect("connection");
        let document = DocumentRepository::create_document(
            &mut connection,
            "owned.pdf",
            "pdf",
            "owned-hash",
            "originals/owned.pdf",
            Some(&owning_job.job_id),
        )
        .expect("document");
        drop(connection);

        let error = AppError::new("import_failed", "failed", "import", true);
        let failure = repository
            .fail_import_if_active(
                &document.document_id,
                &requested_job.job_id,
                &error,
                "failed",
            )
            .expect_err("mismatched owner must fail");
        assert_eq!(failure.code, "import_failure_state_conflict");

        let mut connection =
            block_on_db(connect_workspace_db(layout.app_db_path())).expect("connection");
        let state = block_on_db(async {
            sqlx::query_as::<_, (String, Option<String>, String, Option<String>, i64, i64)>(
                "SELECT
                   (SELECT status FROM documents WHERE document_id = ?1),
                   (SELECT error_summary FROM documents WHERE document_id = ?1),
                   (SELECT status FROM jobs WHERE job_id = ?2),
                   (SELECT error_id FROM jobs WHERE job_id = ?2),
                   (SELECT COUNT(*) FROM errors),
                   (SELECT COUNT(*) FROM job_events
                    WHERE job_id = ?2 AND event_type = 'failed')",
            )
            .bind(&document.document_id)
            .bind(&requested_job.job_id)
            .fetch_one(&mut connection)
            .await
            .map_err(|err| database_error("test", "import_failure_owner_check_failed", err))
        })
        .expect("state check");
        assert_eq!(state.0, "importing");
        assert!(state.1.is_none());
        assert_eq!(state.2, JobStatus::Queued.as_str());
        assert!(state.3.is_none());
        assert_eq!(state.4, 0);
        assert_eq!(state.5, 0);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn import_failure_rolls_back_document_when_job_cas_loses() {
        let (root, layout, repository) = test_repository("slicer-import-failure-rollback");
        let job = repository.append_job("document_import").expect("job");
        let mut connection =
            block_on_db(connect_workspace_db(layout.app_db_path())).expect("connection");
        let document = DocumentRepository::create_document(
            &mut connection,
            "rollback.pdf",
            "pdf",
            "rollback-import-hash",
            "originals/rollback.pdf",
            Some(&job.job_id),
        )
        .expect("document");
        block_on_db(async {
            sqlx::query(
                "CREATE TRIGGER ignore_import_job_failure
                 BEFORE UPDATE ON jobs
                 WHEN NEW.status = 'failed'
                 BEGIN
                   SELECT RAISE(IGNORE);
                 END",
            )
            .execute(&mut connection)
            .await
            .map_err(|err| database_error("test", "import_failure_trigger_create_failed", err))?;
            Ok(())
        })
        .expect("failure trigger");
        drop(connection);

        let error = AppError::new("import_failed", "failed", "import", true);
        let failure = repository
            .fail_import_if_active(&document.document_id, &job.job_id, &error, "failed")
            .expect_err("job CAS conflict");
        assert_eq!(failure.code, "import_failure_state_conflict");

        let mut connection =
            block_on_db(connect_workspace_db(layout.app_db_path())).expect("connection");
        let state = block_on_db(async {
            sqlx::query_as::<_, (String, Option<String>, String, Option<String>, i64, i64)>(
                "SELECT
                   (SELECT status FROM documents WHERE document_id = ?1),
                   (SELECT error_summary FROM documents WHERE document_id = ?1),
                   (SELECT status FROM jobs WHERE job_id = ?2),
                   (SELECT error_id FROM jobs WHERE job_id = ?2),
                   (SELECT COUNT(*) FROM errors),
                   (SELECT COUNT(*) FROM job_events
                    WHERE job_id = ?2 AND event_type = 'failed')",
            )
            .bind(&document.document_id)
            .bind(&job.job_id)
            .fetch_one(&mut connection)
            .await
            .map_err(|err| database_error("test", "import_failure_rollback_check_failed", err))
        })
        .expect("state check");
        assert_eq!(state.0, "importing");
        assert!(state.1.is_none());
        assert_eq!(state.2, JobStatus::Queued.as_str());
        assert!(state.3.is_none());
        assert_eq!(state.4, 0);
        assert_eq!(state.5, 0);

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

    #[test]
    fn recovery_fails_associated_importing_documents_with_retryable_error() {
        let (root, layout, repository) = test_repository("slicer-import-recover");
        let job = repository.append_job("document_import").expect("job");
        repository
            .update_job_progress(&job.job_id, 40, Some("import running"))
            .expect("running");

        let mut connection =
            block_on_db(connect_workspace_db(layout.app_db_path())).expect("connection");
        let importing = DocumentRepository::create_document(
            &mut connection,
            "interrupted.pdf",
            "pdf",
            "interrupted-hash",
            "originals/interrupted.pdf",
            Some(&job.job_id),
        )
        .expect("importing document");
        let ready = DocumentRepository::create_document(
            &mut connection,
            "ready.pdf",
            "pdf",
            "ready-hash",
            "originals/ready.pdf",
            Some(&job.job_id),
        )
        .expect("ready document");
        DocumentRepository::update_document_status(
            &mut connection,
            &ready.document_id,
            "ready",
            Some(1),
            None,
        )
        .expect("mark ready");
        drop(connection);

        let recovered = repository
            .recover_interrupted_jobs()
            .expect("recover interrupted import");
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].status, JobStatus::Failed.as_str());
        assert_eq!(
            recovered[0].error_summary.as_deref(),
            Some(super::INTERRUPTED_JOB_SUMMARY)
        );

        let mut connection =
            block_on_db(connect_workspace_db(layout.app_db_path())).expect("connection");
        let importing =
            DocumentRepository::find_document_by_id(&mut connection, &importing.document_id)
                .expect("importing lookup")
                .expect("importing document remains");
        assert_eq!(importing.status, "failed");
        assert_eq!(
            importing.error_summary.as_deref(),
            Some(super::INTERRUPTED_JOB_SUMMARY)
        );
        assert_eq!(importing.job_id.as_deref(), Some(job.job_id.as_str()));

        let ready = DocumentRepository::find_document_by_id(&mut connection, &ready.document_id)
            .expect("ready lookup")
            .expect("ready document remains");
        assert_eq!(ready.status, "ready");
        assert!(ready.error_summary.is_none());

        let error = block_on_db(async {
            sqlx::query_as::<_, (String, String, i64)>(
                "SELECT errors.code, errors.stage, errors.retryable
                 FROM jobs
                 INNER JOIN errors ON errors.error_id = jobs.error_id
                 WHERE jobs.job_id = ?1",
            )
            .bind(&job.job_id)
            .fetch_one(&mut connection)
            .await
            .map_err(|err| database_error("test", "recovery_error_lookup_failed", err))
        })
        .expect("structured recovery error");
        assert_eq!(
            error,
            ("job_interrupted".to_string(), "job_recovery".to_string(), 1)
        );

        drop(connection);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn recovery_rolls_back_job_document_and_error_when_event_insert_fails() {
        let (root, layout, repository) = test_repository("slicer-import-recover-rollback");
        let job = repository.append_job("document_import").expect("job");
        repository
            .update_job_progress(&job.job_id, 20, Some("import running"))
            .expect("running");

        let mut connection =
            block_on_db(connect_workspace_db(layout.app_db_path())).expect("connection");
        let document = DocumentRepository::create_document(
            &mut connection,
            "rollback.pdf",
            "pdf",
            "rollback-hash",
            "originals/rollback.pdf",
            Some(&job.job_id),
        )
        .expect("importing document");
        block_on_db(async {
            sqlx::query(
                "CREATE TRIGGER fail_interrupted_recovery_event
                 BEFORE INSERT ON job_events
                 WHEN NEW.event_type = 'recovered_as_failed'
                 BEGIN
                   SELECT RAISE(ABORT, 'forced recovery failure');
                 END",
            )
            .execute(&mut connection)
            .await
            .map_err(|err| database_error("test", "recovery_trigger_create_failed", err))?;
            Ok(())
        })
        .expect("failure trigger");
        drop(connection);

        let recovered = repository
            .recover_interrupted_jobs()
            .expect("recovery continues after per-job failure");
        assert!(recovered.is_empty());

        let jobs = repository.list_jobs().expect("jobs");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].status, JobStatus::Running.as_str());
        assert!(jobs[0].error_id.is_none());

        let mut connection =
            block_on_db(connect_workspace_db(layout.app_db_path())).expect("connection");
        let document =
            DocumentRepository::find_document_by_id(&mut connection, &document.document_id)
                .expect("document lookup")
                .expect("document remains");
        assert_eq!(document.status, "importing");
        assert!(document.error_summary.is_none());
        let error_count = block_on_db(async {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM errors")
                .fetch_one(&mut connection)
                .await
                .map_err(|err| database_error("test", "recovery_error_count_failed", err))
        })
        .expect("error count");
        assert_eq!(error_count, 0);
        let recovery_event_count = block_on_db(async {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM job_events
                 WHERE job_id = ?1 AND event_type = 'recovered_as_failed'",
            )
            .bind(&job.job_id)
            .fetch_one(&mut connection)
            .await
            .map_err(|err| database_error("test", "recovery_event_count_failed", err))
        })
        .expect("recovery event count");
        assert_eq!(recovery_event_count, 0);

        drop(connection);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn recovery_rolls_back_failed_commit_and_continues_with_later_jobs() {
        let (root, layout, repository) = test_repository("slicer-import-recover-commit");
        let first = repository.append_job("document_import").expect("first job");
        repository
            .update_job_progress(&first.job_id, 20, Some("first import running"))
            .expect("first running");
        let second = repository
            .append_job("document_import")
            .expect("second job");
        repository
            .update_job_progress(&second.job_id, 30, Some("second import running"))
            .expect("second running");

        let mut connection =
            block_on_db(connect_workspace_db(layout.app_db_path())).expect("connection");
        let first_document = DocumentRepository::create_document(
            &mut connection,
            "first.pdf",
            "pdf",
            "first-commit-hash",
            "originals/first.pdf",
            Some(&first.job_id),
        )
        .expect("first document");
        block_on_db(async {
            sqlx::query(
                "UPDATE jobs
                 SET updated_at = CASE job_id
                   WHEN ?1 THEN '2020-01-01T00:00:00Z'
                   WHEN ?2 THEN '2021-01-01T00:00:00Z'
                 END
                 WHERE job_id IN (?1, ?2)",
            )
            .bind(&first.job_id)
            .bind(&second.job_id)
            .execute(&mut connection)
            .await
            .map_err(|err| database_error("test", "recovery_order_seed_failed", err))?;
            sqlx::query(
                "CREATE TABLE forced_recovery_commit_failures (
                   job_id TEXT PRIMARY KEY
                 )",
            )
            .execute(&mut connection)
            .await
            .map_err(|err| database_error("test", "commit_failure_table_create_failed", err))?;
            sqlx::query("INSERT INTO forced_recovery_commit_failures (job_id) VALUES (?1)")
                .bind(&first.job_id)
                .execute(&mut connection)
                .await
                .map_err(|err| database_error("test", "commit_failure_seed_failed", err))?;
            sqlx::query(
                "CREATE TABLE deferred_recovery_violation (
                   id INTEGER PRIMARY KEY,
                   missing_job_id TEXT NOT NULL,
                   FOREIGN KEY (missing_job_id) REFERENCES jobs(job_id)
                     DEFERRABLE INITIALLY DEFERRED
                 )",
            )
            .execute(&mut connection)
            .await
            .map_err(|err| database_error("test", "deferred_failure_table_create_failed", err))?;
            sqlx::query(
                "CREATE TRIGGER fail_selected_recovery_commit
                 AFTER INSERT ON job_events
                 WHEN NEW.event_type = 'recovered_as_failed'
                   AND EXISTS (
                     SELECT 1 FROM forced_recovery_commit_failures
                     WHERE job_id = NEW.job_id
                   )
                 BEGIN
                   INSERT INTO deferred_recovery_violation (missing_job_id)
                   VALUES ('missing-job');
                 END",
            )
            .execute(&mut connection)
            .await
            .map_err(|err| database_error("test", "commit_failure_trigger_create_failed", err))?;
            Ok(())
        })
        .expect("commit failure setup");
        drop(connection);

        let recovered = repository
            .recover_interrupted_jobs()
            .expect("recovery continues after commit failure");
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].job_id, second.job_id);

        let jobs = repository.list_jobs().expect("jobs");
        let first_after = jobs
            .iter()
            .find(|job| job.job_id == first.job_id)
            .expect("first job remains");
        assert_eq!(first_after.status, JobStatus::Running.as_str());
        assert!(first_after.error_id.is_none());
        let second_after = jobs
            .iter()
            .find(|job| job.job_id == second.job_id)
            .expect("second job remains");
        assert_eq!(second_after.status, JobStatus::Failed.as_str());

        let mut connection =
            block_on_db(connect_workspace_db(layout.app_db_path())).expect("connection");
        let first_document =
            DocumentRepository::find_document_by_id(&mut connection, &first_document.document_id)
                .expect("first document lookup")
                .expect("first document remains");
        assert_eq!(first_document.status, "importing");
        assert!(first_document.error_summary.is_none());
        let recovery_counts = block_on_db(async {
            sqlx::query_as::<_, (i64, i64, i64)>(
                "SELECT
                   (SELECT COUNT(*) FROM errors),
                   (SELECT COUNT(*) FROM job_events
                    WHERE job_id = ?1 AND event_type = 'recovered_as_failed'),
                   (SELECT COUNT(*) FROM job_events
                    WHERE job_id = ?2 AND event_type = 'recovered_as_failed')",
            )
            .bind(&first.job_id)
            .bind(&second.job_id)
            .fetch_one(&mut connection)
            .await
            .map_err(|err| database_error("test", "commit_recovery_counts_failed", err))
        })
        .expect("recovery counts");
        assert_eq!(recovery_counts, (1, 0, 1));

        drop(connection);
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
