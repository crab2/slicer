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
    Migration {
        version: 3,
        name: "documents_pages_images",
        sql: include_str!("../../migrations/0003_documents_pages_images.sql"),
    },
    Migration {
        version: 4,
        name: "analysis_results",
        sql: include_str!("../../migrations/0004_analysis_results.sql"),
    },
    Migration {
        version: 5,
        name: "index_versions",
        sql: include_str!("../../migrations/0005_index_versions.sql"),
    },
    Migration {
        version: 6,
        name: "pdf_structure",
        sql: include_str!("../../migrations/0006_pdf_structure.sql"),
    },
    Migration {
        version: 7,
        name: "previewless_pdf_pages",
        sql: include_str!("../../migrations/0007_previewless_pdf_pages.sql"),
    },
    Migration {
        version: 8,
        name: "document_view_artifacts",
        sql: include_str!("../../migrations/0008_document_view_artifacts.sql"),
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

        apply_migration(&mut connection, migration).await?;
    }

    Ok(())
}

async fn apply_migration(
    connection: &mut SqliteConnection,
    migration: &Migration,
) -> AppResult<()> {
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *connection)
        .await
        .map_err(|err| database_error("migration", "migration_begin_failed", err))?;
    let result = async {
        execute_sql_script(&mut *connection, migration.sql).await?;
        sqlx::query(
            "INSERT INTO schema_migrations (version, name, applied_at)
             VALUES (?1, ?2, ?3)",
        )
        .bind(migration.version)
        .bind(migration.name)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&mut *connection)
        .await
        .map_err(|err| database_error("migration", "migration_record_failed", err))?;
        Ok(())
    }
    .await;
    match result {
        Ok(()) => {
            sqlx::query("COMMIT")
                .execute(connection)
                .await
                .map_err(|err| database_error("migration", "migration_commit_failed", err))?;
            Ok(())
        }
        Err(error) => {
            let _ = sqlx::query("ROLLBACK").execute(connection).await;
            Err(error)
        }
    }
}

async fn execute_sql_script(connection: &mut SqliteConnection, script: &str) -> AppResult<()> {
    for statement in script
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
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
    use super::{
        apply_migration, block_on_db, connect_workspace_db, execute_sql_script, run_migrations,
        Migration, MIGRATIONS,
    };
    use sqlx::Executor;
    use std::fs;

    #[test]
    fn migrations_are_idempotent_and_create_minimal_ledger_tables() {
        let root = std::env::temp_dir().join(format!("slicer-db-migration-{}", std::process::id()));
        let db_path = root.join("app.db");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("temp root");

        block_on_db(run_migrations(db_path.clone())).expect("first migration");
        block_on_db(run_migrations(db_path.clone())).expect("second migration");

        block_on_db(async {
            let mut connection = connect_workspace_db(db_path).await?;
            for table in [
                "schema_migrations",
                "settings",
                "jobs",
                "errors",
                "job_events",
                "documents",
                "page_records",
                "image_assets",
                "analysis_results",
                "index_versions",
                "index_active",
                "document_artifacts",
                "pdf_parse_runs",
                "content_blocks",
                "visual_module_analysis",
            ] {
                let exists = sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                )
                .bind(table)
                .fetch_one(&mut connection)
                .await
                .map_err(|err| super::database_error("test", "table_lookup_failed", err))?;
                assert_eq!(exists, 1, "{table} should exist");
            }
            Ok(())
        })
        .expect("table assertions");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn upgrades_v5_ledger_to_v6_without_replacing_existing_rows() {
        let root =
            std::env::temp_dir().join(format!("slicer-db-v5-upgrade-{}", std::process::id()));
        let db_path = root.join("app.db");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("temp root");

        block_on_db(async {
            let mut connection = connect_workspace_db(db_path.clone()).await?;
            connection
                .execute(
                    "CREATE TABLE schema_migrations (
                       version INTEGER PRIMARY KEY,
                       name TEXT NOT NULL,
                       applied_at TEXT NOT NULL
                     )",
                )
                .await
                .map_err(|err| super::database_error("test", "migration_seed_failed", err))?;
            for migration in MIGRATIONS.iter().filter(|migration| migration.version <= 5) {
                execute_sql_script(&mut connection, migration.sql).await?;
                sqlx::query(
                    "INSERT INTO schema_migrations (version, name, applied_at)
                     VALUES (?1, ?2, '2026-01-01T00:00:00Z')",
                )
                .bind(migration.version)
                .bind(migration.name)
                .execute(&mut connection)
                .await
                .map_err(|err| {
                    super::database_error("test", "migration_version_seed_failed", err)
                })?;
            }
            sqlx::query(
                "INSERT INTO index_versions
                 (version_id, provider, analyzer_version, status, index_directory,
                  document_count, created_at, updated_at)
                 VALUES ('legacy-v1', 'tantivy_bm25', 'cjk_bigram_v2', 'ready',
                         'indexes/bm25/build-legacy-v1', 7,
                         '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            )
            .execute(&mut connection)
            .await
            .map_err(|err| super::database_error("test", "legacy_index_seed_failed", err))?;
            Ok(())
        })
        .expect("seed v5 database");

        block_on_db(run_migrations(db_path.clone())).expect("upgrade to v6");
        block_on_db(run_migrations(db_path.clone())).expect("v6 rerun is idempotent");

        block_on_db(async {
            let mut connection = connect_workspace_db(db_path).await?;
            let row = sqlx::query_as::<_, (String, String, i64)>(
                "SELECT content_schema_version, content_fingerprint, document_count
                 FROM index_versions WHERE version_id = 'legacy-v1'",
            )
            .fetch_one(&mut connection)
            .await
            .map_err(|err| super::database_error("test", "legacy_index_lookup_failed", err))?;
            assert_eq!(row, ("page_v1".to_string(), String::new(), 7));

            let version_count = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = 6",
            )
            .fetch_one(&mut connection)
            .await
            .map_err(|err| super::database_error("test", "v6_version_lookup_failed", err))?;
            assert_eq!(version_count, 1);
            Ok(())
        })
        .expect("v6 assertions");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn upgrades_v6_ledger_to_previewless_pages_without_losing_relations() {
        let root =
            std::env::temp_dir().join(format!("slicer-db-v6-upgrade-{}", uuid::Uuid::new_v4()));
        let db_path = root.join("app.db");
        fs::create_dir_all(&root).expect("temp root");

        block_on_db(async {
            let mut connection = connect_workspace_db(db_path.clone()).await?;
            connection
                .execute(
                    "CREATE TABLE schema_migrations (
                       version INTEGER PRIMARY KEY,
                       name TEXT NOT NULL,
                       applied_at TEXT NOT NULL
                     )",
                )
                .await
                .map_err(|err| super::database_error("test", "migration_seed_failed", err))?;
            for migration in MIGRATIONS.iter().filter(|migration| migration.version <= 6) {
                execute_sql_script(&mut connection, migration.sql).await?;
                sqlx::query(
                    "INSERT INTO schema_migrations (version, name, applied_at)
                     VALUES (?1, ?2, '2026-01-01T00:00:00Z')",
                )
                .bind(migration.version)
                .bind(migration.name)
                .execute(&mut connection)
                .await
                .map_err(|err| {
                    super::database_error("test", "migration_version_seed_failed", err)
                })?;
            }
            execute_sql_script(
                &mut connection,
                "INSERT INTO documents
                     (document_id, original_filename, file_type, file_hash, original_path,
                      page_count, status, created_at, updated_at)
                     VALUES ('doc-1', 'legacy.pdf', 'pdf', 'file-hash', 'originals/legacy.pdf',
                             1, 'ready', '2026-01-01', '2026-01-01');
                     INSERT INTO image_assets
                     (image_hash, file_path, file_size, created_at)
                     VALUES ('image-hash', 'pages/doc-1/image.png', 10, '2026-01-01');
                     INSERT INTO page_records
                     (page_id, document_id, page_number, image_hash, status, created_at, updated_at,
                      pdf_width_points, pdf_height_points, crop_left_points, crop_bottom_points,
                      crop_right_points, crop_top_points, rotation_degrees,
                      preview_width_px, preview_height_px)
                     VALUES ('page-1', 'doc-1', 1, 'image-hash', 'analyzed',
                             '2026-01-01', '2026-01-01', 100, 200, 0, 0, 100, 200, 0, 200, 400);
                     INSERT INTO analysis_results
                     (analysis_id, page_id, schema_version, provider, model_name, status,
                      result_json, created_at, updated_at)
                     VALUES ('analysis-1', 'page-1', 'page_analysis_v1', 'mock', 'mock-model',
                             'succeeded', '{\"legacy\":true}', '2026-01-01', '2026-01-01');
                     INSERT INTO pdf_parse_runs
                     (parse_id, document_id, parser_name, parser_version, schema_version,
                      parser_options_json, status, raw_json_path, created_at, updated_at)
                     VALUES ('parse-1', 'doc-1', 'opendataloader-pdf', '2.5.0', 'v2', '{}',
                             'succeeded', 'structure/doc-1/out.json', '2026-01-01', '2026-01-01');
                     INSERT INTO content_blocks
                     (block_id, parse_id, document_id, page_id, page_number, ordinal, block_type,
                      source_text, enrichment_json, raw_json, is_indexable, is_visual,
                      is_decorative, created_at, updated_at)
                     VALUES ('parent', 'parse-1', 'doc-1', 'page-1', 1, 0, 'figure', 'caption',
                             '{\"description\":\"old enrichment\"}', '{}', 1, 1, 0,
                             '2026-01-01', '2026-01-01');
                     INSERT INTO content_blocks
                     (block_id, parse_id, document_id, page_id, page_number, parent_block_id,
                      ordinal, block_type, source_text, raw_json, is_indexable, is_visual,
                      is_decorative, created_at, updated_at)
                     VALUES ('child', 'parse-1', 'doc-1', 'page-1', 1, 'parent', 1, 'caption',
                             'nested', '{}', 0, 0, 0, '2026-01-01', '2026-01-01');
                     INSERT INTO visual_module_analysis
                     (analysis_id, block_id, schema_version, provider, model_name, status,
                      result_json, attempt_count, created_at, updated_at)
                     VALUES ('visual-1', 'parent', 'visual_module_analysis_v1', 'mock',
                             'mock-model', 'succeeded', '{\"description\":\"old enrichment\"}',
                             2, '2026-01-01', '2026-01-01')",
            )
            .await?;
            Ok(())
        })
        .expect("seed v6 database");

        block_on_db(run_migrations(db_path.clone())).expect("upgrade to v7");

        block_on_db(async {
            let mut connection = connect_workspace_db(db_path).await?;
            let legacy = sqlx::query_as::<_, (Option<String>, String, Option<i64>, Option<i64>)>(
                "SELECT image_hash, status, preview_width_px, preview_height_px
                 FROM page_records WHERE page_id = 'page-1'",
            )
            .fetch_one(&mut connection)
            .await
            .map_err(|err| super::database_error("test", "legacy_page_lookup_failed", err))?;
            assert_eq!(
                legacy,
                (
                    Some("image-hash".to_string()),
                    "analyzed".to_string(),
                    Some(200),
                    Some(400)
                )
            );

            connection
                .execute(
                    "INSERT INTO page_records
                     (page_id, document_id, page_number, image_hash, status, created_at, updated_at)
                     VALUES ('page-2', 'doc-1', 2, NULL, 'structured',
                             '2026-01-02', '2026-01-02')",
                )
                .await
                .map_err(|err| {
                    super::database_error("test", "structured_page_insert_failed", err)
                })?;

            let analysis_json = sqlx::query_scalar::<_, String>(
                "SELECT result_json FROM analysis_results WHERE analysis_id = 'analysis-1'",
            )
            .fetch_one(&mut connection)
            .await
            .map_err(|err| super::database_error("test", "analysis_lookup_failed", err))?;
            assert_eq!(analysis_json, "{\"legacy\":true}");
            let block = sqlx::query_as::<_, (Option<String>, Option<String>)>(
                "SELECT parent_block_id, enrichment_json FROM content_blocks
                 WHERE block_id = 'child'",
            )
            .fetch_one(&mut connection)
            .await
            .map_err(|err| super::database_error("test", "block_lookup_failed", err))?;
            assert_eq!(block.0.as_deref(), Some("parent"));
            let enrichment = sqlx::query_scalar::<_, String>(
                "SELECT enrichment_json FROM content_blocks WHERE block_id = 'parent'",
            )
            .fetch_one(&mut connection)
            .await
            .map_err(|err| super::database_error("test", "enrichment_lookup_failed", err))?;
            assert!(enrichment.contains("old enrichment"));
            let visual_attempts = sqlx::query_scalar::<_, i64>(
                "SELECT attempt_count FROM visual_module_analysis WHERE analysis_id = 'visual-1'",
            )
            .fetch_one(&mut connection)
            .await
            .map_err(|err| super::database_error("test", "visual_lookup_failed", err))?;
            assert_eq!(visual_attempts, 2);

            let violations = sqlx::query("PRAGMA foreign_key_check")
                .fetch_all(&mut connection)
                .await
                .map_err(|err| super::database_error("test", "foreign_key_check_failed", err))?;
            assert!(violations.is_empty(), "foreign key violations were found");
            Ok(())
        })
        .expect("v7 assertions");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn upgrades_v7_artifacts_to_viewer_formats_without_losing_rows_or_cascades() {
        let root =
            std::env::temp_dir().join(format!("slicer-db-v7-upgrade-{}", uuid::Uuid::new_v4()));
        let db_path = root.join("app.db");
        fs::create_dir_all(&root).expect("temp root");

        block_on_db(async {
            let mut connection = connect_workspace_db(db_path.clone()).await?;
            connection
                .execute(
                    "CREATE TABLE schema_migrations (
                       version INTEGER PRIMARY KEY,
                       name TEXT NOT NULL,
                       applied_at TEXT NOT NULL
                     )",
                )
                .await
                .map_err(|err| super::database_error("test", "migration_seed_failed", err))?;
            for migration in MIGRATIONS.iter().filter(|migration| migration.version <= 7) {
                execute_sql_script(&mut connection, migration.sql).await?;
                sqlx::query(
                    "INSERT INTO schema_migrations (version, name, applied_at)
                     VALUES (?1, ?2, '2026-01-01T00:00:00Z')",
                )
                .bind(migration.version)
                .bind(migration.name)
                .execute(&mut connection)
                .await
                .map_err(|err| {
                    super::database_error("test", "migration_version_seed_failed", err)
                })?;
            }
            execute_sql_script(
                &mut connection,
                "INSERT INTO documents
                   (document_id, original_filename, file_type, file_hash, original_path,
                    page_count, status, created_at, updated_at)
                 VALUES ('doc-v7', 'legacy.pdf', 'pdf', 'hash', 'originals/legacy.pdf',
                         1, 'ready', '2026-01-01', '2026-01-01');
                 INSERT INTO document_artifacts
                   (artifact_id, document_id, kind, relative_path, content_hash,
                    created_at, updated_at)
                 VALUES ('artifact-pdf', 'doc-v7', 'canonical_pdf', 'pdfs/legacy.pdf',
                         'pdf-hash', '2026-01-01', '2026-01-01');
                 INSERT INTO document_artifacts
                   (artifact_id, document_id, kind, relative_path, content_hash,
                    created_at, updated_at)
                 VALUES ('artifact-json', 'doc-v7', 'pdf_structure_json',
                         'structure/legacy.json', 'json-hash', '2026-01-01', '2026-01-01');
                 INSERT INTO document_artifacts
                   (artifact_id, document_id, kind, relative_path, content_hash,
                    created_at, updated_at)
                 VALUES ('artifact-image', 'doc-v7', 'pdf_structure_image',
                         'structure/images/legacy.png', 'image-hash',
                         '2026-01-01', '2026-01-01')",
            )
            .await?;
            Ok(())
        })
        .expect("seed v7 database");

        block_on_db(run_migrations(db_path.clone())).expect("upgrade to v8");
        block_on_db(run_migrations(db_path.clone())).expect("v8 rerun is idempotent");

        block_on_db(async {
            let mut connection = connect_workspace_db(db_path).await?;
            let legacy_rows = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM document_artifacts WHERE document_id = 'doc-v7'",
            )
            .fetch_one(&mut connection)
            .await
            .map_err(|err| super::database_error("test", "artifact_count_failed", err))?;
            assert_eq!(legacy_rows, 3);
            let pdf_hash = sqlx::query_scalar::<_, String>(
                "SELECT content_hash FROM document_artifacts WHERE artifact_id = 'artifact-pdf'",
            )
            .fetch_one(&mut connection)
            .await
            .map_err(|err| super::database_error("test", "artifact_hash_failed", err))?;
            assert_eq!(pdf_hash, "pdf-hash");

            sqlx::query(
                "INSERT INTO document_artifacts
                   (artifact_id, document_id, kind, relative_path, content_hash,
                    created_at, updated_at)
                 VALUES ('artifact-md', 'doc-v7', 'pdf_structure_markdown',
                         'structure/legacy.md', 'md-hash', '2026-01-02', '2026-01-02')",
            )
            .execute(&mut connection)
            .await
            .map_err(|err| super::database_error("test", "new_artifact_insert_failed", err))?;
            let duplicate = sqlx::query(
                "INSERT INTO document_artifacts
                   (artifact_id, document_id, kind, relative_path, content_hash,
                    created_at, updated_at)
                 VALUES ('artifact-pdf-2', 'doc-v7', 'canonical_pdf', 'pdfs/other.pdf',
                         'other-hash', '2026-01-02', '2026-01-02')",
            )
            .execute(&mut connection)
            .await;
            assert!(
                duplicate.is_err(),
                "singleton index must reject a second canonical PDF"
            );

            sqlx::query("DELETE FROM documents WHERE document_id = 'doc-v7'")
                .execute(&mut connection)
                .await
                .map_err(|err| super::database_error("test", "document_delete_failed", err))?;
            let remaining = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM document_artifacts WHERE document_id = 'doc-v7'",
            )
            .fetch_one(&mut connection)
            .await
            .map_err(|err| super::database_error("test", "cascade_count_failed", err))?;
            assert_eq!(remaining, 0);
            let violations = sqlx::query("PRAGMA foreign_key_check")
                .fetch_all(&mut connection)
                .await
                .map_err(|err| super::database_error("test", "foreign_key_check_failed", err))?;
            assert!(violations.is_empty(), "foreign key violations were found");
            Ok(())
        })
        .expect("v8 assertions");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failed_migration_rolls_back_schema_and_version_record() {
        let root = std::env::temp_dir().join(format!(
            "slicer-db-migration-rollback-{}",
            uuid::Uuid::new_v4()
        ));
        let db_path = root.join("app.db");
        fs::create_dir_all(&root).expect("temp root");

        block_on_db(async {
            let mut connection = connect_workspace_db(db_path).await?;
            connection
                .execute(
                    "CREATE TABLE schema_migrations (
                       version INTEGER PRIMARY KEY,
                       name TEXT NOT NULL,
                       applied_at TEXT NOT NULL
                     )",
                )
                .await
                .map_err(|err| super::database_error("test", "migration_seed_failed", err))?;
            let broken = Migration {
                version: 99,
                name: "broken",
                sql: "CREATE TABLE should_rollback (id INTEGER); INVALID SQL",
            };
            apply_migration(&mut connection, &broken)
                .await
                .expect_err("broken migration must fail");

            let table_count = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name = 'should_rollback'",
            )
            .fetch_one(&mut connection)
            .await
            .map_err(|err| super::database_error("test", "rollback_table_lookup_failed", err))?;
            let version_count = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = 99",
            )
            .fetch_one(&mut connection)
            .await
            .map_err(|err| super::database_error("test", "rollback_version_lookup_failed", err))?;
            assert_eq!(table_count, 0);
            assert_eq!(version_count, 0);
            Ok(())
        })
        .expect("rollback assertions");

        let _ = fs::remove_dir_all(root);
    }
}
