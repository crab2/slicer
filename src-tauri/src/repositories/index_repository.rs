use crate::domain::index::{IndexVersionDto, DEFAULT_SEARCH_PROVIDER_ID};
use crate::errors::{AppError, AppResult};
use crate::repositories::db::{block_on_db, database_error};
use chrono::Utc;
use sqlx::SqliteConnection;

pub struct IndexRepository;

#[derive(sqlx::FromRow)]
struct IndexVersionRow {
    version_id: String,
    provider: String,
    analyzer_version: String,
    content_schema_version: String,
    content_fingerprint: String,
    status: String,
    index_directory: String,
    document_count: i64,
    build_started_at: Option<String>,
    build_finished_at: Option<String>,
    activated_at: Option<String>,
    error_id: Option<String>,
    created_at: String,
    updated_at: String,
}

impl IndexVersionRow {
    fn to_dto(self) -> IndexVersionDto {
        IndexVersionDto {
            version_id: self.version_id,
            provider: self.provider,
            analyzer_version: self.analyzer_version,
            content_schema_version: self.content_schema_version,
            content_fingerprint: self.content_fingerprint,
            status: self.status,
            index_directory: self.index_directory,
            document_count: self.document_count,
            build_started_at: self.build_started_at,
            build_finished_at: self.build_finished_at,
            activated_at: self.activated_at,
            error_id: self.error_id,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl IndexRepository {
    pub fn create_build_version_with_schema(
        conn: &mut SqliteConnection,
        version_id: &str,
        provider: &str,
        analyzer_version: &str,
        content_schema_version: &str,
        index_directory: &str,
    ) -> AppResult<IndexVersionDto> {
        let now = Utc::now().to_rfc3339();
        block_on_db(async {
            let row = sqlx::query_as::<_, IndexVersionRow>(
                "INSERT INTO index_versions
                 (version_id, provider, analyzer_version, content_schema_version, status,
                  index_directory, document_count,
                  build_started_at, build_finished_at, activated_at, error_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 'building', ?5, 0, ?6, NULL, NULL, NULL, ?6, ?6)
                 RETURNING version_id, provider, analyzer_version, content_schema_version,
                           content_fingerprint, status,
                           index_directory,
                           document_count, build_started_at, build_finished_at, activated_at,
                           error_id, created_at, updated_at",
            )
            .bind(version_id)
            .bind(provider)
            .bind(analyzer_version)
            .bind(content_schema_version)
            .bind(index_directory)
            .bind(&now)
            .fetch_one(conn)
            .await
            .map_err(|err| database_error("index", "index_version_create_failed", err))?;
            Ok(row.to_dto())
        })
    }

    pub fn activate_version_and_complete_job(
        conn: &mut SqliteConnection,
        provider: &str,
        version_id: &str,
        document_count: i64,
        content_fingerprint: &str,
        job_id: &str,
        message: &str,
    ) -> AppResult<IndexVersionDto> {
        let now = Utc::now().to_rfc3339();
        block_on_db(async {
            sqlx::query("BEGIN IMMEDIATE")
                .execute(&mut *conn)
                .await
                .map_err(|err| database_error("index", "index_activation_begin_failed", err))?;
            let result = async {
                let row = sqlx::query_as::<_, IndexVersionRow>(
                    "UPDATE index_versions
                     SET status = 'ready',
                         document_count = ?2,
                         content_fingerprint = ?3,
                         build_finished_at = ?4,
                         activated_at = ?4,
                         updated_at = ?4
                     WHERE version_id = ?1 AND provider = ?5 AND status = 'building'
                     RETURNING version_id, provider, analyzer_version, content_schema_version,
                               content_fingerprint, status, index_directory, document_count,
                               build_started_at, build_finished_at, activated_at, error_id,
                               created_at, updated_at",
                )
                .bind(version_id)
                .bind(document_count)
                .bind(content_fingerprint)
                .bind(&now)
                .bind(provider)
                .fetch_one(&mut *conn)
                .await
                .map_err(|err| database_error("index", "index_version_ready_failed", err))?;

                sqlx::query(
                    "INSERT INTO index_active (provider, version_id, updated_at)
                     VALUES (?1, ?2, ?3)
                     ON CONFLICT(provider) DO UPDATE SET
                       version_id = excluded.version_id,
                       updated_at = excluded.updated_at",
                )
                .bind(provider)
                .bind(version_id)
                .bind(&now)
                .execute(&mut *conn)
                .await
                .map_err(|err| database_error("index", "index_active_set_failed", err))?;

                let job_affected = sqlx::query(
                    "UPDATE jobs
                     SET status = 'succeeded', progress = 100, updated_at = ?2,
                         error_id = NULL, error_summary = NULL
                     WHERE job_id = ?1 AND job_type = 'index_rebuild'
                       AND status IN ('queued', 'running')",
                )
                .bind(job_id)
                .bind(&now)
                .execute(&mut *conn)
                .await
                .map_err(|err| database_error("index", "index_job_finalize_failed", err))?
                .rows_affected();
                if job_affected != 1 {
                    return Err(AppError::new(
                        "index_job_state_conflict",
                        "索引重建任务状态已变化，无法提交完成状态。",
                        "index",
                        true,
                    ));
                }

                sqlx::query(
                    "INSERT INTO job_events
                     (event_id, job_id, event_type, message, progress, created_at)
                     VALUES (?1, ?2, 'progress_updated', ?3, 100, ?4)",
                )
                .bind(uuid::Uuid::new_v4().to_string())
                .bind(job_id)
                .bind(message)
                .bind(&now)
                .execute(&mut *conn)
                .await
                .map_err(|err| database_error("index", "index_job_event_create_failed", err))?;

                Ok(row)
            }
            .await;

            match result {
                Ok(row) => match sqlx::query("COMMIT").execute(&mut *conn).await {
                    Ok(_) => Ok(row.to_dto()),
                    Err(err) => {
                        let error = database_error("index", "index_activation_commit_failed", err);
                        let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                        Err(error)
                    }
                },
                Err(error) => {
                    let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                    Err(error)
                }
            }
        })
    }

    pub fn mark_version_failed(
        conn: &mut SqliteConnection,
        version_id: &str,
        error_id: Option<&str>,
    ) -> AppResult<IndexVersionDto> {
        let now = Utc::now().to_rfc3339();
        block_on_db(async {
            let row = sqlx::query_as::<_, IndexVersionRow>(
                "UPDATE index_versions
                 SET status = 'failed',
                     build_finished_at = ?2,
                     error_id = ?3,
                     updated_at = ?2
                 WHERE version_id = ?1 AND status = 'building'
                 RETURNING version_id, provider, analyzer_version, content_schema_version,
                           content_fingerprint, status,
                           index_directory,
                           document_count, build_started_at, build_finished_at, activated_at,
                           error_id, created_at, updated_at",
            )
            .bind(version_id)
            .bind(&now)
            .bind(error_id)
            .fetch_one(conn)
            .await
            .map_err(|err| database_error("index", "index_version_failed_failed", err))?;
            Ok(row.to_dto())
        })
    }

    pub fn find_active_version(
        conn: &mut SqliteConnection,
        provider: &str,
    ) -> AppResult<Option<IndexVersionDto>> {
        block_on_db(async {
            let row = sqlx::query_as::<_, IndexVersionRow>(
                "SELECT iv.version_id, iv.provider, iv.analyzer_version, iv.content_schema_version,
                        iv.content_fingerprint, iv.status, iv.index_directory,
                        iv.document_count, iv.build_started_at, iv.build_finished_at, iv.activated_at,
                        iv.error_id, iv.created_at, iv.updated_at
                 FROM index_active ia
                 JOIN index_versions iv ON iv.version_id = ia.version_id
                 WHERE ia.provider = ?1 AND iv.status = 'ready'",
            )
            .bind(provider)
            .fetch_optional(conn)
            .await
            .map_err(|err| database_error("index", "index_active_lookup_failed", err))?;
            Ok(row.map(|r| r.to_dto()))
        })
    }

    pub fn find_version(
        conn: &mut SqliteConnection,
        version_id: &str,
    ) -> AppResult<Option<IndexVersionDto>> {
        block_on_db(async {
            let row = sqlx::query_as::<_, IndexVersionRow>(
                "SELECT version_id, provider, analyzer_version, content_schema_version,
                        content_fingerprint, status,
                        index_directory,
                        document_count, build_started_at, build_finished_at, activated_at,
                        error_id, created_at, updated_at
                 FROM index_versions
                 WHERE version_id = ?1",
            )
            .bind(version_id)
            .fetch_optional(conn)
            .await
            .map_err(|err| database_error("index", "index_version_lookup_failed", err))?;
            Ok(row.map(|r| r.to_dto()))
        })
    }

    pub fn list_building_versions(
        conn: &mut SqliteConnection,
        provider: &str,
    ) -> AppResult<Vec<IndexVersionDto>> {
        block_on_db(async {
            let rows = sqlx::query_as::<_, IndexVersionRow>(
                "SELECT version_id, provider, analyzer_version, content_schema_version,
                        content_fingerprint, status,
                        index_directory,
                        document_count, build_started_at, build_finished_at, activated_at,
                        error_id, created_at, updated_at
                 FROM index_versions
                 WHERE provider = ?1 AND status = 'building'
                 ORDER BY updated_at DESC",
            )
            .bind(provider)
            .fetch_all(conn)
            .await
            .map_err(|err| database_error("index", "index_building_list_failed", err))?;
            Ok(rows.into_iter().map(|r| r.to_dto()).collect())
        })
    }

    pub fn find_latest_failed_version(
        conn: &mut SqliteConnection,
        provider: &str,
    ) -> AppResult<Option<IndexVersionDto>> {
        block_on_db(async {
            let row = sqlx::query_as::<_, IndexVersionRow>(
                "SELECT version_id, provider, analyzer_version, content_schema_version,
                        content_fingerprint, status,
                        index_directory,
                        document_count, build_started_at, build_finished_at, activated_at,
                        error_id, created_at, updated_at
                 FROM index_versions
                 WHERE provider = ?1 AND status = 'failed'
                 ORDER BY updated_at DESC
                 LIMIT 1",
            )
            .bind(provider)
            .fetch_optional(conn)
            .await
            .map_err(|err| database_error("index", "index_failed_lookup_failed", err))?;
            Ok(row.map(|r| r.to_dto()))
        })
    }

    pub fn recover_stale_building_versions(
        conn: &mut SqliteConnection,
        provider: &str,
    ) -> AppResult<Vec<IndexVersionDto>> {
        let building = Self::list_building_versions(conn, provider)?;
        let mut recovered = Vec::new();
        for version in building {
            let updated = Self::mark_version_failed(conn, &version.version_id, None)?;
            recovered.push(updated);
        }
        Ok(recovered)
    }

    pub fn default_provider() -> &'static str {
        DEFAULT_SEARCH_PROVIDER_ID
    }
}

#[cfg(test)]
mod tests {
    use super::IndexRepository;
    use crate::repositories::db::{block_on_db, connect_workspace_db, run_migrations};
    use sqlx::SqliteConnection;
    use std::fs;

    fn insert_job(conn: &mut SqliteConnection, job_id: &str, status: &str) {
        block_on_db(async {
            sqlx::query(
                "INSERT INTO jobs
                 (job_id, job_type, status, progress, created_at, updated_at)
                 VALUES (?1, 'index_rebuild', ?2, 80,
                         '2026-07-30T00:00:00+00:00', '2026-07-30T00:00:00+00:00')",
            )
            .bind(job_id)
            .bind(status)
            .execute(conn)
            .await
            .map_err(|err| {
                crate::repositories::db::database_error("test", "job_insert_failed", err)
            })?;
            Ok(())
        })
        .expect("job");
    }

    #[test]
    fn activation_switches_ready_version_atomically_and_ready_version_cannot_fail_late() {
        let root =
            std::env::temp_dir().join(format!("slicer-index-activation-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).expect("root");
        let db_path = root.join("app.db");
        block_on_db(run_migrations(db_path.clone())).expect("migrations");
        let mut conn = block_on_db(connect_workspace_db(db_path)).expect("connection");

        IndexRepository::create_build_version_with_schema(
            &mut conn,
            "version-1",
            IndexRepository::default_provider(),
            "cjk_bigram_v2",
            "pdf_modules_v2",
            "indexes/bm25/build-version-1",
        )
        .expect("building version");
        insert_job(&mut conn, "job-1", "running");
        IndexRepository::activate_version_and_complete_job(
            &mut conn,
            IndexRepository::default_provider(),
            "version-1",
            2,
            "fingerprint-1",
            "job-1",
            "index complete",
        )
        .expect("activate");

        let active =
            IndexRepository::find_active_version(&mut conn, IndexRepository::default_provider())
                .expect("active lookup")
                .expect("active version");
        assert_eq!(active.version_id, "version-1");
        assert_eq!(active.content_fingerprint, "fingerprint-1");
        let (job_status, progress, event_count) = block_on_db(async {
            let (status, progress) = sqlx::query_as::<_, (String, i64)>(
                "SELECT status, progress FROM jobs WHERE job_id = 'job-1'",
            )
            .fetch_one(&mut conn)
            .await
            .map_err(|err| {
                crate::repositories::db::database_error("test", "job_read_failed", err)
            })?;
            let event_count = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM job_events
                 WHERE job_id = 'job-1' AND event_type = 'progress_updated' AND progress = 100",
            )
            .fetch_one(&mut conn)
            .await
            .map_err(|err| {
                crate::repositories::db::database_error("test", "job_event_read_failed", err)
            })?;
            Ok((status, progress, event_count))
        })
        .expect("completed job");
        assert_eq!(job_status, "succeeded");
        assert_eq!(progress, 100);
        assert_eq!(event_count, 1);
        assert!(IndexRepository::mark_version_failed(&mut conn, "version-1", None).is_err());
        assert_eq!(
            IndexRepository::find_active_version(&mut conn, IndexRepository::default_provider(),)
                .expect("active lookup after late failure")
                .expect("active version after late failure")
                .version_id,
            "version-1"
        );

        IndexRepository::create_build_version_with_schema(
            &mut conn,
            "version-2",
            IndexRepository::default_provider(),
            "cjk_bigram_v2",
            "pdf_modules_v2",
            "indexes/bm25/build-version-2",
        )
        .expect("second building version");
        insert_job(&mut conn, "job-2", "failed");
        let error = IndexRepository::activate_version_and_complete_job(
            &mut conn,
            IndexRepository::default_provider(),
            "version-2",
            3,
            "fingerprint-2",
            "job-2",
            "index complete",
        )
        .expect_err("job conflict must roll back activation");
        assert_eq!(error.code, "index_job_state_conflict");
        assert_eq!(
            IndexRepository::find_version(&mut conn, "version-2")
                .expect("version lookup")
                .expect("version")
                .status,
            "building"
        );
        assert_eq!(
            IndexRepository::find_active_version(&mut conn, IndexRepository::default_provider())
                .expect("active after rollback")
                .expect("active version after rollback")
                .version_id,
            "version-1"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn activation_rolls_back_when_commit_fails() {
        let root = std::env::temp_dir().join(format!(
            "slicer-index-activation-commit-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("root");
        let db_path = root.join("app.db");
        block_on_db(run_migrations(db_path.clone())).expect("migrations");
        let mut conn = block_on_db(connect_workspace_db(db_path)).expect("connection");

        IndexRepository::create_build_version_with_schema(
            &mut conn,
            "version-commit-failure",
            IndexRepository::default_provider(),
            "cjk_bigram_v2",
            "pdf_modules_v2",
            "indexes/bm25/build-version-commit-failure",
        )
        .expect("building version");
        insert_job(&mut conn, "job-commit-failure", "running");
        block_on_db(async {
            sqlx::query(
                "CREATE TABLE deferred_index_activation_violation (
                   id INTEGER PRIMARY KEY,
                   missing_job_id TEXT NOT NULL,
                   FOREIGN KEY (missing_job_id) REFERENCES jobs(job_id)
                     DEFERRABLE INITIALLY DEFERRED
                 )",
            )
            .execute(&mut conn)
            .await
            .map_err(|err| {
                crate::repositories::db::database_error(
                    "test",
                    "commit_violation_table_failed",
                    err,
                )
            })?;
            sqlx::query(
                "CREATE TRIGGER fail_index_activation_commit
                 AFTER INSERT ON job_events
                 WHEN NEW.job_id = 'job-commit-failure'
                 BEGIN
                   INSERT INTO deferred_index_activation_violation (missing_job_id)
                   VALUES ('missing-job');
                 END",
            )
            .execute(&mut conn)
            .await
            .map_err(|err| {
                crate::repositories::db::database_error(
                    "test",
                    "commit_violation_trigger_failed",
                    err,
                )
            })?;
            Ok(())
        })
        .expect("commit failure setup");

        let error = IndexRepository::activate_version_and_complete_job(
            &mut conn,
            IndexRepository::default_provider(),
            "version-commit-failure",
            4,
            "fingerprint-commit-failure",
            "job-commit-failure",
            "index complete",
        )
        .expect_err("commit failure must roll back activation");
        assert_eq!(error.code, "index_activation_commit_failed");

        let version = IndexRepository::find_version(&mut conn, "version-commit-failure")
            .expect("version lookup")
            .expect("version");
        assert_eq!(version.status, "building");
        assert!(IndexRepository::find_active_version(
            &mut conn,
            IndexRepository::default_provider()
        )
        .expect("active lookup")
        .is_none());
        let (job_status, progress, event_count, violation_count) = block_on_db(async {
            let (status, progress) = sqlx::query_as::<_, (String, i64)>(
                "SELECT status, progress FROM jobs WHERE job_id = 'job-commit-failure'",
            )
            .fetch_one(&mut conn)
            .await
            .map_err(|err| {
                crate::repositories::db::database_error("test", "job_read_failed", err)
            })?;
            let event_count = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM job_events WHERE job_id = 'job-commit-failure'",
            )
            .fetch_one(&mut conn)
            .await
            .map_err(|err| {
                crate::repositories::db::database_error("test", "job_event_read_failed", err)
            })?;
            let violation_count = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM deferred_index_activation_violation",
            )
            .fetch_one(&mut conn)
            .await
            .map_err(|err| {
                crate::repositories::db::database_error("test", "violation_read_failed", err)
            })?;
            Ok((status, progress, event_count, violation_count))
        })
        .expect("rolled back state");
        assert_eq!(job_status, "running");
        assert_eq!(progress, 80);
        assert_eq!(event_count, 0);
        assert_eq!(violation_count, 0);

        drop(conn);
        let _ = fs::remove_dir_all(root);
    }
}
