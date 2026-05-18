use crate::errors::{AppError, AppResult};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Connection, Executor, SqliteConnection};
use std::future::Future;
use std::path::PathBuf;

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial_ledger",
        sql: include_str!("../../migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "jobs_and_events",
        sql: include_str!("../../migrations/0002_jobs_and_events.sql"),
    },
];

pub fn block_on_db<T>(future: impl Future<Output = AppResult<T>>) -> AppResult<T> {
    tauri::async_runtime::block_on(future)
}

pub async fn connect_workspace_db(path: PathBuf) -> AppResult<SqliteConnection> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    let mut connection = SqliteConnection::connect_with(&options)
        .await
        .map_err(|err| database_error("ledger", "database_open_failed", err))?;

    connection
        .execute("PRAGMA foreign_keys = ON")
        .await
        .map_err(|err| database_error("ledger", "database_pragma_failed", err))?;

    Ok(connection)
}

pub async fn run_migrations(path: PathBuf) -> AppResult<()> {
    let mut connection = connect_workspace_db(path).await?;
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
              version INTEGER PRIMARY KEY,
              name TEXT NOT NULL,
              applied_at TEXT NOT NULL
            )",
        )
        .await
        .map_err(|err| database_error("migration", "migration_metadata_failed", err))?;

    for migration in MIGRATIONS {
        let existing = sqlx::query_scalar::<_, i64>(
            "SELECT version FROM schema_migrations WHERE version = ?1",
        )
        .bind(migration.version)
        .fetch_optional(&mut connection)
        .await
        .map_err(|err| database_error("migration", "migration_lookup_failed", err))?;

        if existing.is_some() {
            continue;
        }

        execute_sql_script(&mut connection, migration.sql).await?;
        sqlx::query(
            "INSERT INTO schema_migrations (version, name, applied_at)
             VALUES (?1, ?2, ?3)",
        )
        .bind(migration.version)
        .bind(migration.name)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&mut connection)
        .await
        .map_err(|err| database_error("migration", "migration_record_failed", err))?;
    }

    Ok(())
}

async fn execute_sql_script(connection: &mut SqliteConnection, script: &str) -> AppResult<()> {
    for statement in script.split(';').map(str::trim).filter(|part| !part.is_empty()) {
        connection
            .execute(statement)
            .await
            .map_err(|err| database_error("migration", "migration_statement_failed", err))?;
    }
    Ok(())
}

pub fn database_error(
    stage: impl Into<String>,
    code: impl Into<String>,
    err: sqlx::Error,
) -> AppError {
    AppError::new(
        code,
        "SQLite 账本操作失败，请检查工作区数据库后重试。",
        stage,
        true,
    )
    .with_details(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::{block_on_db, connect_workspace_db, run_migrations};
    use std::fs;

    #[test]
    fn migrations_are_idempotent_and_create_minimal_ledger_tables() {
        let root =
            std::env::temp_dir().join(format!("slicer-db-migration-{}", std::process::id()));
        let db_path = root.join("app.db");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("temp root");

        block_on_db(run_migrations(db_path.clone())).expect("first migration");
        block_on_db(run_migrations(db_path.clone())).expect("second migration");

        block_on_db(async {
            let mut connection = connect_workspace_db(db_path).await?;
            for table in ["schema_migrations", "settings", "jobs", "errors", "job_events"] {
                let exists = sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                )
                .bind(table)
                .fetch_one(&mut connection)
                .await
                .map_err(|err| super::database_error("test", "table_lookup_failed", err))?;
                assert_eq!(exists, 1, "{table} should exist");
            }

            for future_table in [
                "documents",
                "page_records",
                "image_assets",
                "analysis_results",
                "index_versions",
            ] {
                let exists = sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                )
                .bind(future_table)
                .fetch_one(&mut connection)
                .await
                .map_err(|err| super::database_error("test", "table_lookup_failed", err))?;
                assert_eq!(exists, 0, "{future_table} should stay deferred");
            }
            Ok(())
        })
        .expect("table assertions");

        let _ = fs::remove_dir_all(root);
    }
}
