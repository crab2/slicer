use crate::domain::index::{IndexVersionDto, DEFAULT_SEARCH_PROVIDER_ID};
use crate::errors::AppResult;
use crate::repositories::db::{block_on_db, database_error};
use chrono::Utc;
use sqlx::SqliteConnection;

pub struct IndexRepository;

#[derive(sqlx::FromRow)]
struct IndexVersionRow {
    version_id: String,
    provider: String,
    analyzer_version: String,
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
    pub fn create_build_version(
        conn: &mut SqliteConnection,
        version_id: &str,
        provider: &str,
        analyzer_version: &str,
        index_directory: &str,
    ) -> AppResult<IndexVersionDto> {
        let now = Utc::now().to_rfc3339();
        block_on_db(async {
            let row = sqlx::query_as::<_, IndexVersionRow>(
                "INSERT INTO index_versions
                 (version_id, provider, analyzer_version, status, index_directory, document_count,
                  build_started_at, build_finished_at, activated_at, error_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'building', ?4, 0, ?5, NULL, NULL, NULL, ?5, ?5)
                 RETURNING version_id, provider, analyzer_version, status, index_directory,
                           document_count, build_started_at, build_finished_at, activated_at,
                           error_id, created_at, updated_at",
            )
            .bind(version_id)
            .bind(provider)
            .bind(analyzer_version)
            .bind(index_directory)
            .bind(&now)
            .fetch_one(conn)
            .await
            .map_err(|err| database_error("index", "index_version_create_failed", err))?;
            Ok(row.to_dto())
        })
    }

    pub fn mark_version_ready(
        conn: &mut SqliteConnection,
        version_id: &str,
        document_count: i64,
    ) -> AppResult<IndexVersionDto> {
        let now = Utc::now().to_rfc3339();
        block_on_db(async {
            let row = sqlx::query_as::<_, IndexVersionRow>(
                "UPDATE index_versions
                 SET status = 'ready',
                     document_count = ?2,
                     build_finished_at = ?3,
                     activated_at = ?3,
                     updated_at = ?3
                 WHERE version_id = ?1
                 RETURNING version_id, provider, analyzer_version, status, index_directory,
                           document_count, build_started_at, build_finished_at, activated_at,
                           error_id, created_at, updated_at",
            )
            .bind(version_id)
            .bind(document_count)
            .bind(&now)
            .fetch_one(conn)
            .await
            .map_err(|err| database_error("index", "index_version_ready_failed", err))?;
            Ok(row.to_dto())
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
                 WHERE version_id = ?1
                 RETURNING version_id, provider, analyzer_version, status, index_directory,
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

    pub fn set_active_version(
        conn: &mut SqliteConnection,
        provider: &str,
        version_id: &str,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        block_on_db(async {
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
            .execute(conn)
            .await
            .map_err(|err| database_error("index", "index_active_set_failed", err))?;
            Ok(())
        })
    }

    pub fn find_active_version(
        conn: &mut SqliteConnection,
        provider: &str,
    ) -> AppResult<Option<IndexVersionDto>> {
        block_on_db(async {
            let row = sqlx::query_as::<_, IndexVersionRow>(
                "SELECT iv.version_id, iv.provider, iv.analyzer_version, iv.status, iv.index_directory,
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
                "SELECT version_id, provider, analyzer_version, status, index_directory,
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
                "SELECT version_id, provider, analyzer_version, status, index_directory,
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
                "SELECT version_id, provider, analyzer_version, status, index_directory,
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
