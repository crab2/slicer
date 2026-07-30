use crate::domain::pdf_structure::{
    DocumentArtifactInput, NormalizedBbox, PdfContentBlockDto, PdfParseRun, VisualModuleAnalysisV1,
    VisualModuleCounts, VISUAL_MODULE_ANALYSIS_SCHEMA_VERSION,
};
use crate::errors::{AppError, AppResult};
use crate::repositories::db::block_on_db;
use crate::repositories::ledger_repository::insert_error;
use sqlx::SqliteConnection;

pub struct PdfStructureRepository;

impl PdfStructureRepository {
    pub fn upsert_canonical_pdf(
        conn: &mut SqliteConnection,
        artifact: &DocumentArtifactInput,
    ) -> AppResult<()> {
        if artifact.kind != "canonical_pdf" {
            return Err(AppError::new(
                "canonical_pdf_artifact_kind_invalid",
                "规范 PDF 制品类型无效。",
                "pdf_structure_persist",
                false,
            ));
        }
        block_on_db(async {
            sqlx::query("BEGIN IMMEDIATE")
                .execute(&mut *conn)
                .await
                .map_err(|err| db_error("canonical_pdf_begin_failed", err))?;
            let result = async {
                sqlx::query(
                    "DELETE FROM document_artifacts
                     WHERE document_id = ?1 AND kind = 'canonical_pdf'",
                )
                .bind(&artifact.document_id)
                .execute(&mut *conn)
                .await
                .map_err(|err| db_error("canonical_pdf_replace_failed", err))?;
                insert_artifact(&mut *conn, artifact).await
            }
            .await;
            finish_transaction(conn, result, "canonical_pdf_commit_failed").await
        })
    }

    pub fn replace_document_structure(
        conn: &mut SqliteConnection,
        run: &PdfParseRun,
        artifacts: &[DocumentArtifactInput],
        blocks: &[PdfContentBlockDto],
    ) -> AppResult<()> {
        validate_replace_input(run, artifacts, blocks)?;
        block_on_db(async {
            sqlx::query("BEGIN IMMEDIATE")
                .execute(&mut *conn)
                .await
                .map_err(|err| db_error("pdf_structure_replace_begin_failed", err))?;
            let result = async {
                sqlx::query(
                    "DELETE FROM document_artifacts
                     WHERE document_id = ?1
                       AND kind IN (
                         'pdf_structure_json',
                         'pdf_structure_html',
                         'pdf_structure_markdown',
                         'pdf_structure_annotated_pdf',
                         'pdf_structure_image'
                       )",
                )
                .bind(&run.document_id)
                .execute(&mut *conn)
                .await
                .map_err(|err| db_error("pdf_structure_artifacts_replace_failed", err))?;
                sqlx::query("DELETE FROM pdf_parse_runs WHERE document_id = ?1")
                    .bind(&run.document_id)
                    .execute(&mut *conn)
                    .await
                    .map_err(|err| db_error("pdf_structure_previous_run_delete_failed", err))?;

                let now = chrono::Utc::now().to_rfc3339();
                sqlx::query(
                    "INSERT INTO pdf_parse_runs
                     (parse_id, document_id, parser_name, parser_version, schema_version,
                      parser_options_json, status, raw_json_path, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'succeeded', ?7, ?8, ?8)",
                )
                .bind(&run.parse_id)
                .bind(&run.document_id)
                .bind(&run.parser_name)
                .bind(&run.parser_version)
                .bind(&run.schema_version)
                .bind(&run.parser_options_json)
                .bind(&run.raw_json_path)
                .bind(&now)
                .execute(&mut *conn)
                .await
                .map_err(|err| db_error("pdf_parse_run_insert_failed", err))?;

                for artifact in artifacts {
                    insert_artifact(&mut *conn, artifact).await?;
                }
                for block in blocks {
                    insert_block(&mut *conn, block, &now).await?;
                }
                Ok(())
            }
            .await;
            finish_transaction(conn, result, "pdf_structure_replace_commit_failed").await
        })
    }

    pub fn record_parse_failure(
        conn: &mut SqliteConnection,
        run: &PdfParseRun,
        error: &AppError,
    ) -> AppResult<()> {
        block_on_db(async {
            sqlx::query("BEGIN IMMEDIATE")
                .execute(&mut *conn)
                .await
                .map_err(|err| db_error("pdf_parse_failure_begin_failed", err))?;
            let result = async {
                let error_id = insert_error(&mut *conn, error).await?;
                let now = chrono::Utc::now().to_rfc3339();
                sqlx::query(
                    "INSERT INTO pdf_parse_runs
                     (parse_id, document_id, parser_name, parser_version, schema_version,
                      parser_options_json, status, raw_json_path, error_id, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'failed', NULLIF(?7, ''), ?8, ?9, ?9)",
                )
                .bind(&run.parse_id)
                .bind(&run.document_id)
                .bind(&run.parser_name)
                .bind(&run.parser_version)
                .bind(&run.schema_version)
                .bind(&run.parser_options_json)
                .bind(&run.raw_json_path)
                .bind(&error_id)
                .bind(now)
                .execute(&mut *conn)
                .await
                .map_err(|err| db_error("pdf_parse_failure_insert_failed", err))?;
                Ok(())
            }
            .await;
            finish_transaction(conn, result, "pdf_parse_failure_commit_failed").await
        })
    }

    pub fn list_indexable_blocks(
        conn: &mut SqliteConnection,
    ) -> AppResult<Vec<PdfContentBlockDto>> {
        block_on_db(async {
            let rows = sqlx::query_as::<_, PdfContentBlockRow>(
                "SELECT content_blocks.block_id, content_blocks.parse_id,
                        content_blocks.document_id, content_blocks.page_id,
                        content_blocks.page_number, content_blocks.parent_block_id,
                        content_blocks.source_element_id, content_blocks.ordinal,
                        content_blocks.block_type, content_blocks.source_text,
                        content_blocks.enrichment_json, content_blocks.raw_json,
                        content_blocks.source_image_path, content_blocks.is_indexable,
                        content_blocks.is_visual, content_blocks.is_decorative,
                        content_blocks.bbox_x, content_blocks.bbox_y,
                        content_blocks.bbox_width, content_blocks.bbox_height
                 FROM content_blocks
                 INNER JOIN pdf_parse_runs ON pdf_parse_runs.parse_id = content_blocks.parse_id
                 INNER JOIN documents
                   ON documents.document_id = content_blocks.document_id
                  AND documents.status = 'ready'
                 WHERE pdf_parse_runs.status = 'succeeded'
                   AND content_blocks.is_indexable = 1
                   AND (TRIM(content_blocks.source_text) != ''
                        OR content_blocks.enrichment_json IS NOT NULL)
                 ORDER BY content_blocks.document_id, content_blocks.page_number,
                          content_blocks.ordinal",
            )
            .fetch_all(conn)
            .await
            .map_err(|err| db_error("pdf_indexable_blocks_list_failed", err))?;
            rows.into_iter().map(PdfContentBlockRow::into_dto).collect()
        })
    }

    pub fn find_block_by_id(
        conn: &mut SqliteConnection,
        block_id: &str,
    ) -> AppResult<Option<PdfContentBlockDto>> {
        block_on_db(async {
            let row = sqlx::query_as::<_, PdfContentBlockRow>(
                "SELECT block_id, parse_id, document_id, page_id, page_number,
                        parent_block_id, source_element_id, ordinal, block_type,
                        source_text, enrichment_json, raw_json, source_image_path,
                        is_indexable, is_visual, is_decorative, bbox_x, bbox_y,
                        bbox_width, bbox_height
                 FROM content_blocks WHERE block_id = ?1",
            )
            .bind(block_id)
            .fetch_optional(conn)
            .await
            .map_err(|err| db_error("pdf_block_lookup_failed", err))?;
            row.map(PdfContentBlockRow::into_dto).transpose()
        })
    }

    pub fn list_visual_blocks_needing_analysis(
        conn: &mut SqliteConnection,
        document_id: Option<&str>,
    ) -> AppResult<Vec<PdfContentBlockDto>> {
        Self::list_visual_blocks(conn, document_id, "needed")
    }

    pub fn list_all_visual_blocks_for_document(
        conn: &mut SqliteConnection,
        document_id: &str,
    ) -> AppResult<Vec<PdfContentBlockDto>> {
        Self::list_visual_blocks(conn, Some(document_id), "all")
    }

    pub fn list_failed_visual_blocks_for_document(
        conn: &mut SqliteConnection,
        document_id: &str,
    ) -> AppResult<Vec<PdfContentBlockDto>> {
        Self::list_visual_blocks(conn, Some(document_id), "failed")
    }

    fn list_visual_blocks(
        conn: &mut SqliteConnection,
        document_id: Option<&str>,
        selection: &str,
    ) -> AppResult<Vec<PdfContentBlockDto>> {
        block_on_db(async {
            let rows = sqlx::query_as::<_, PdfContentBlockRow>(
                "SELECT content_blocks.block_id, content_blocks.parse_id,
                        content_blocks.document_id, content_blocks.page_id,
                        content_blocks.page_number, content_blocks.parent_block_id,
                        content_blocks.source_element_id, content_blocks.ordinal,
                        content_blocks.block_type, content_blocks.source_text,
                        content_blocks.enrichment_json, content_blocks.raw_json,
                        content_blocks.source_image_path, content_blocks.is_indexable,
                        content_blocks.is_visual, content_blocks.is_decorative,
                        content_blocks.bbox_x, content_blocks.bbox_y,
                        content_blocks.bbox_width, content_blocks.bbox_height
                 FROM content_blocks
                 INNER JOIN pdf_parse_runs ON pdf_parse_runs.parse_id = content_blocks.parse_id
                 INNER JOIN documents
                   ON documents.document_id = content_blocks.document_id
                  AND documents.status = 'ready'
                 LEFT JOIN visual_module_analysis
                   ON visual_module_analysis.block_id = content_blocks.block_id
                 WHERE pdf_parse_runs.status = 'succeeded'
                   AND content_blocks.is_visual = 1
                   AND content_blocks.is_decorative = 0
                   AND (?1 IS NULL OR content_blocks.document_id = ?1)
                   AND (
                     ?2 = 'all'
                     OR (?2 = 'failed' AND visual_module_analysis.status = 'failed')
                     OR (?2 = 'needed' AND visual_module_analysis.status IS NULL)
                   )
                 ORDER BY content_blocks.document_id, content_blocks.page_number,
                          content_blocks.ordinal",
            )
            .bind(document_id)
            .bind(selection)
            .fetch_all(conn)
            .await
            .map_err(|err| db_error("visual_blocks_list_failed", err))?;
            rows.into_iter().map(PdfContentBlockRow::into_dto).collect()
        })
    }

    pub fn document_has_canonical_pdf(
        conn: &mut SqliteConnection,
        document_id: &str,
    ) -> AppResult<bool> {
        block_on_db(async {
            let count = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM document_artifacts
                 WHERE document_id = ?1 AND kind = 'canonical_pdf'",
            )
            .bind(document_id)
            .fetch_one(conn)
            .await
            .map_err(|err| db_error("canonical_pdf_lookup_failed", err))?;
            Ok(count > 0)
        })
    }

    pub fn find_document_artifact_content_hash(
        conn: &mut SqliteConnection,
        document_id: &str,
        kind: &str,
        relative_path: &str,
    ) -> AppResult<Option<String>> {
        block_on_db(async {
            sqlx::query_scalar::<_, String>(
                "SELECT content_hash FROM document_artifacts
                 WHERE document_id = ?1 AND kind = ?2 AND relative_path = ?3",
            )
            .bind(document_id)
            .bind(kind)
            .bind(relative_path)
            .fetch_optional(conn)
            .await
            .map_err(|err| db_error("document_artifact_hash_lookup_failed", err))
        })
    }

    pub fn visual_module_counts_for_page(
        conn: &mut SqliteConnection,
        page_id: &str,
    ) -> AppResult<Option<VisualModuleCounts>> {
        block_on_db(async {
            let has_canonical_pdf = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*)
                 FROM page_records
                 INNER JOIN documents
                   ON documents.document_id = page_records.document_id
                  AND documents.status = 'ready'
                 INNER JOIN document_artifacts
                   ON document_artifacts.document_id = page_records.document_id
                  AND document_artifacts.kind = 'canonical_pdf'
                 WHERE page_records.page_id = ?1",
            )
            .bind(page_id)
            .fetch_one(&mut *conn)
            .await
            .map_err(|err| db_error("visual_module_counts_document_lookup_failed", err))?
                > 0;
            if !has_canonical_pdf {
                return Ok(None);
            }

            let row = sqlx::query_as::<_, VisualModuleCountRow>(
                "SELECT
                   COUNT(content_blocks.block_id) AS total,
                   COALESCE(SUM(CASE
                     WHEN visual_module_analysis.status IS NULL
                       OR visual_module_analysis.status = 'pending'
                     THEN 1 ELSE 0 END), 0) AS pending,
                   COALESCE(SUM(CASE WHEN visual_module_analysis.status = 'succeeded'
                     THEN 1 ELSE 0 END), 0) AS succeeded,
                   COALESCE(SUM(CASE WHEN visual_module_analysis.status = 'failed'
                     THEN 1 ELSE 0 END), 0) AS failed
                 FROM content_blocks
                 INNER JOIN pdf_parse_runs
                   ON pdf_parse_runs.parse_id = content_blocks.parse_id
                  AND pdf_parse_runs.status = 'succeeded'
                 LEFT JOIN visual_module_analysis
                   ON visual_module_analysis.block_id = content_blocks.block_id
                 WHERE content_blocks.page_id = ?1
                   AND content_blocks.is_visual = 1
                   AND content_blocks.is_decorative = 0",
            )
            .bind(page_id)
            .fetch_one(conn)
            .await
            .map_err(|err| db_error("visual_module_counts_lookup_failed", err))?;
            Ok(Some(VisualModuleCounts {
                total: row.total,
                pending: row.pending,
                succeeded: row.succeeded,
                failed: row.failed,
            }))
        })
    }

    pub fn try_mark_visual_pending(
        conn: &mut SqliteConnection,
        block_id: &str,
        provider: &str,
        model_name: &str,
    ) -> AppResult<Option<i64>> {
        block_on_db(async {
            sqlx::query("BEGIN IMMEDIATE")
                .execute(&mut *conn)
                .await
                .map_err(|err| db_error("visual_analysis_lease_begin_failed", err))?;
            let result = async {
                let now = chrono::Utc::now().to_rfc3339();
                let attempt = sqlx::query_scalar::<_, i64>(
                    "INSERT INTO visual_module_analysis
                     (analysis_id, block_id, schema_version, provider, model_name, status,
                      attempt_count, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, 'pending', 1, ?6, ?6)
                     ON CONFLICT(block_id) DO UPDATE SET
                       schema_version = excluded.schema_version,
                       provider = excluded.provider,
                       model_name = excluded.model_name,
                       status = 'pending',
                       error_id = NULL,
                       attempt_count = visual_module_analysis.attempt_count + 1,
                       updated_at = excluded.updated_at
                     WHERE visual_module_analysis.status != 'pending'
                     RETURNING attempt_count",
                )
                .bind(uuid::Uuid::new_v4().to_string())
                .bind(block_id)
                .bind(VISUAL_MODULE_ANALYSIS_SCHEMA_VERSION)
                .bind(provider)
                .bind(model_name)
                .bind(&now)
                .fetch_optional(&mut *conn)
                .await
                .map_err(|err| db_error("visual_analysis_lease_failed", err))?;
                Ok(attempt)
            }
            .await;
            finish_transaction(conn, result, "visual_analysis_lease_commit_failed").await
        })
    }

    pub fn save_visual_success(
        conn: &mut SqliteConnection,
        block_id: &str,
        attempt_count: i64,
        provider: &str,
        model_name: &str,
        enrichment_json: &str,
    ) -> AppResult<()> {
        let parsed: VisualModuleAnalysisV1 =
            serde_json::from_str(enrichment_json).map_err(|err| {
                AppError::new(
                    "visual_module_json_invalid",
                    "视觉模块分析结果不是有效 JSON。",
                    "visual_module_analysis",
                    false,
                )
                .with_details(err.to_string())
            })?;
        if parsed.schema_version != VISUAL_MODULE_ANALYSIS_SCHEMA_VERSION
            || parsed.block_id != block_id
            || parsed.model.provider != provider
            || parsed.model.model_name != model_name
            || (parsed.description.trim().is_empty() && parsed.visible_text.trim().is_empty())
        {
            return Err(AppError::new(
                "visual_module_identity_invalid",
                "视觉模块分析结果必须是 JSON 对象。",
                "visual_module_analysis",
                false,
            ));
        }
        let enrichment_json = serde_json::to_string(&parsed).map_err(|err| {
            AppError::new(
                "visual_module_json_serialize_failed",
                "Visual-module enrichment could not be normalized.",
                "visual_module_analysis",
                false,
            )
            .with_details(err.to_string())
        })?;
        block_on_db(async {
            sqlx::query("BEGIN IMMEDIATE")
                .execute(&mut *conn)
                .await
                .map_err(|err| db_error("visual_analysis_save_begin_failed", err))?;
            let result = async {
                let now = chrono::Utc::now().to_rfc3339();
                let analysis = sqlx::query(
                    "UPDATE visual_module_analysis
                     SET provider = ?1, model_name = ?2, status = 'succeeded',
                         result_json = ?3, error_id = NULL, updated_at = ?4
                     WHERE block_id = ?5 AND status = 'pending' AND attempt_count = ?6",
                )
                .bind(provider)
                .bind(model_name)
                .bind(&enrichment_json)
                .bind(&now)
                .bind(block_id)
                .bind(attempt_count)
                .execute(&mut *conn)
                .await
                .map_err(|err| db_error("visual_analysis_save_failed", err))?;
                if analysis.rows_affected() != 1 {
                    return Err(AppError::new(
                        "visual_analysis_lease_missing",
                        "视觉模块分析任务未处于可提交状态。",
                        "visual_module_analysis",
                        true,
                    ));
                }
                sqlx::query(
                    "UPDATE content_blocks
                     SET enrichment_json = ?1, updated_at = ?2
                     WHERE block_id = ?3",
                )
                .bind(&enrichment_json)
                .bind(now)
                .bind(block_id)
                .execute(&mut *conn)
                .await
                .map_err(|err| db_error("visual_enrichment_save_failed", err))?;
                Ok(())
            }
            .await;
            finish_transaction(conn, result, "visual_analysis_save_commit_failed").await
        })
    }

    pub fn save_visual_failure(
        conn: &mut SqliteConnection,
        block_id: &str,
        attempt_count: i64,
        error: &AppError,
    ) -> AppResult<()> {
        block_on_db(async {
            sqlx::query("BEGIN IMMEDIATE")
                .execute(&mut *conn)
                .await
                .map_err(|err| db_error("visual_analysis_failure_begin_failed", err))?;
            let result = async {
                let error_id = insert_error(&mut *conn, error).await?;
                let result = sqlx::query(
                    "UPDATE visual_module_analysis
                     SET status = 'failed', error_id = ?1, updated_at = ?2
                     WHERE block_id = ?3 AND status = 'pending' AND attempt_count = ?4",
                )
                .bind(error_id)
                .bind(chrono::Utc::now().to_rfc3339())
                .bind(block_id)
                .bind(attempt_count)
                .execute(&mut *conn)
                .await
                .map_err(|err| db_error("visual_analysis_failure_save_failed", err))?;
                if result.rows_affected() != 1 {
                    return Err(AppError::new(
                        "visual_analysis_lease_missing",
                        "The visual-module analysis lease no longer matches this attempt.",
                        "visual_module_analysis",
                        true,
                    ));
                }
                Ok(())
            }
            .await;
            finish_transaction(conn, result, "visual_analysis_failure_commit_failed").await
        })
    }

    pub fn recover_pending_visual_analyses(
        conn: &mut SqliteConnection,
        error_id: &str,
    ) -> AppResult<u64> {
        block_on_db(async {
            let result = sqlx::query(
                "UPDATE visual_module_analysis
                 SET status = 'failed', error_id = ?1, updated_at = ?2
                 WHERE status = 'pending'",
            )
            .bind(error_id)
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(conn)
            .await
            .map_err(|err| db_error("visual_analysis_pending_recovery_failed", err))?;
            Ok(result.rows_affected())
        })
    }

    pub fn count_pending_visual_analyses(conn: &mut SqliteConnection) -> AppResult<i64> {
        block_on_db(async {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM visual_module_analysis WHERE status = 'pending'",
            )
            .fetch_one(conn)
            .await
            .map_err(|err| db_error("visual_analysis_pending_count_failed", err))
        })
    }
}

fn validate_replace_input(
    run: &PdfParseRun,
    artifacts: &[DocumentArtifactInput],
    blocks: &[PdfContentBlockDto],
) -> AppResult<()> {
    if run.raw_json_path.trim().is_empty() {
        return Err(AppError::new(
            "pdf_structure_raw_json_path_missing",
            "PDF 结构化原始 JSON 路径缺失。",
            "pdf_structure_persist",
            false,
        ));
    }
    if artifacts
        .iter()
        .any(|artifact| artifact.document_id != run.document_id)
        || blocks
            .iter()
            .any(|block| block.document_id != run.document_id || block.parse_id != run.parse_id)
    {
        return Err(AppError::new(
            "pdf_structure_identity_mismatch",
            "PDF 结构化数据的文档或解析标识不一致。",
            "pdf_structure_persist",
            false,
        ));
    }
    if !artifacts
        .iter()
        .any(|artifact| artifact.kind == "pdf_structure_json")
    {
        return Err(AppError::new(
            "pdf_structure_json_artifact_missing",
            "PDF 结构化 JSON 制品记录缺失。",
            "pdf_structure_persist",
            false,
        ));
    }
    Ok(())
}

async fn insert_artifact(
    conn: &mut SqliteConnection,
    artifact: &DocumentArtifactInput,
) -> AppResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO document_artifacts
         (artifact_id, document_id, kind, relative_path, content_hash,
          parser_name, parser_version, parser_options_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
    )
    .bind(&artifact.artifact_id)
    .bind(&artifact.document_id)
    .bind(&artifact.kind)
    .bind(&artifact.relative_path)
    .bind(&artifact.content_hash)
    .bind(&artifact.parser_name)
    .bind(&artifact.parser_version)
    .bind(&artifact.parser_options_json)
    .bind(now)
    .execute(conn)
    .await
    .map_err(|err| db_error("document_artifact_insert_failed", err))?;
    Ok(())
}

async fn insert_block(
    conn: &mut SqliteConnection,
    block: &PdfContentBlockDto,
    now: &str,
) -> AppResult<()> {
    let (bbox_x, bbox_y, bbox_width, bbox_height) = match block.bbox {
        Some(bbox) if bbox.is_valid() => (
            Some(bbox.x),
            Some(bbox.y),
            Some(bbox.width),
            Some(bbox.height),
        ),
        _ => (None, None, None, None),
    };
    sqlx::query(
        "INSERT INTO content_blocks
         (block_id, parse_id, document_id, page_id, page_number, parent_block_id,
          source_element_id, ordinal, block_type, source_text, enrichment_json,
          raw_json, source_image_path, is_indexable, is_visual, is_decorative,
          bbox_x, bbox_y, bbox_width, bbox_height, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                 ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?21)",
    )
    .bind(&block.block_id)
    .bind(&block.parse_id)
    .bind(&block.document_id)
    .bind(&block.page_id)
    .bind(block.page_number)
    .bind(&block.parent_block_id)
    .bind(&block.source_element_id)
    .bind(block.ordinal)
    .bind(&block.block_type)
    .bind(&block.source_text)
    .bind(&block.enrichment_json)
    .bind(&block.raw_json)
    .bind(&block.source_image_path)
    .bind(block.is_indexable)
    .bind(block.is_visual)
    .bind(block.is_decorative)
    .bind(bbox_x)
    .bind(bbox_y)
    .bind(bbox_width)
    .bind(bbox_height)
    .bind(now)
    .execute(conn)
    .await
    .map_err(|err| db_error("pdf_content_block_insert_failed", err))?;
    Ok(())
}

async fn finish_transaction<T>(
    conn: &mut SqliteConnection,
    result: AppResult<T>,
    commit_code: &str,
) -> AppResult<T> {
    match result {
        Ok(value) => match sqlx::query("COMMIT").execute(&mut *conn).await {
            Ok(_) => Ok(value),
            Err(err) => {
                let error = db_error(commit_code, err);
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                Err(error)
            }
        },
        Err(error) => {
            let _ = sqlx::query("ROLLBACK").execute(conn).await;
            Err(error)
        }
    }
}

fn db_error(code: &str, error: sqlx::Error) -> AppError {
    super::db::database_error("pdf_structure", code, error)
}

#[derive(Debug, sqlx::FromRow)]
struct PdfContentBlockRow {
    block_id: String,
    parse_id: String,
    document_id: String,
    page_id: String,
    page_number: i64,
    parent_block_id: Option<String>,
    source_element_id: Option<String>,
    ordinal: i64,
    block_type: String,
    source_text: String,
    enrichment_json: Option<String>,
    raw_json: String,
    source_image_path: Option<String>,
    is_indexable: bool,
    is_visual: bool,
    is_decorative: bool,
    bbox_x: Option<f64>,
    bbox_y: Option<f64>,
    bbox_width: Option<f64>,
    bbox_height: Option<f64>,
}

#[derive(Debug, sqlx::FromRow)]
struct VisualModuleCountRow {
    total: i64,
    pending: i64,
    succeeded: i64,
    failed: i64,
}

impl PdfContentBlockRow {
    fn into_dto(self) -> AppResult<PdfContentBlockDto> {
        let bbox = match (self.bbox_x, self.bbox_y, self.bbox_width, self.bbox_height) {
            (Some(x), Some(y), Some(width), Some(height)) => {
                let bbox = NormalizedBbox {
                    x,
                    y,
                    width,
                    height,
                };
                bbox.is_valid().then_some(bbox)
            }
            (None, None, None, None) => None,
            _ => None,
        };
        Ok(PdfContentBlockDto {
            block_id: self.block_id,
            parse_id: self.parse_id,
            document_id: self.document_id,
            page_id: self.page_id,
            page_number: self.page_number,
            parent_block_id: self.parent_block_id,
            source_element_id: self.source_element_id,
            ordinal: self.ordinal,
            block_type: self.block_type,
            source_text: self.source_text,
            enrichment_json: self.enrichment_json,
            raw_json: self.raw_json,
            source_image_path: self.source_image_path,
            is_indexable: self.is_indexable,
            is_visual: self.is_visual,
            is_decorative: self.is_decorative,
            bbox,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::PdfStructureRepository;
    use crate::domain::pdf_structure::{
        DocumentArtifactInput, NormalizedBbox, PdfContentBlockDto, PdfParseRun,
    };
    use crate::repositories::db::{block_on_db, connect_workspace_db, run_migrations};
    use std::fs;

    fn run(document_id: &str, parse_id: &str) -> PdfParseRun {
        PdfParseRun {
            parse_id: parse_id.to_string(),
            document_id: document_id.to_string(),
            parser_name: "opendataloader-pdf".to_string(),
            parser_version: "2.5.0".to_string(),
            schema_version: "v2".to_string(),
            parser_options_json: "{}".to_string(),
            raw_json_path: format!("structure/{parse_id}/document.json"),
        }
    }

    #[test]
    fn replacement_is_atomic_and_cascades_old_blocks() {
        let root = std::env::temp_dir().join(format!("slicer-pdf-repo-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("root");
        let db = root.join("app.db");
        block_on_db(run_migrations(db.clone())).expect("migrate");
        let mut conn = block_on_db(connect_workspace_db(db)).expect("connect");
        seed_document_and_page(&mut conn, "doc", "page");

        let artifact = |parse_id: &str| DocumentArtifactInput {
            artifact_id: format!("artifact-{parse_id}"),
            document_id: "doc".to_string(),
            kind: "pdf_structure_json".to_string(),
            relative_path: format!("structure/{parse_id}/document.json"),
            content_hash: parse_id.to_string(),
            parser_name: Some("opendataloader-pdf".to_string()),
            parser_version: Some("2.5.0".to_string()),
            parser_options_json: Some("{}".to_string()),
        };
        let block = |parse_id: &str, block_id: &str| PdfContentBlockDto {
            block_id: block_id.to_string(),
            parse_id: parse_id.to_string(),
            document_id: "doc".to_string(),
            page_id: "page".to_string(),
            page_number: 1,
            parent_block_id: None,
            source_element_id: Some("1".to_string()),
            ordinal: 0,
            block_type: "paragraph".to_string(),
            source_text: "searchable".to_string(),
            enrichment_json: None,
            raw_json: "{}".to_string(),
            source_image_path: None,
            is_indexable: true,
            is_visual: false,
            is_decorative: false,
            bbox: Some(NormalizedBbox {
                x: 0.1,
                y: 0.1,
                width: 0.5,
                height: 0.2,
            }),
        };

        PdfStructureRepository::replace_document_structure(
            &mut conn,
            &run("doc", "parse-1"),
            &[artifact("parse-1")],
            &[block("parse-1", "block-1")],
        )
        .expect("first parse");
        assert_eq!(
            PdfStructureRepository::try_mark_visual_pending(
                &mut conn,
                "block-1",
                "mock",
                "mock-model",
            )
            .expect("visual analysis ledger"),
            Some(1)
        );
        PdfStructureRepository::replace_document_structure(
            &mut conn,
            &run("doc", "parse-2"),
            &[artifact("parse-2")],
            &[block("parse-2", "block-2")],
        )
        .expect("replacement");

        let blocks = PdfStructureRepository::list_indexable_blocks(&mut conn).expect("blocks");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_id, "block-2");
        assert!(
            PdfStructureRepository::find_block_by_id(&mut conn, "block-1")
                .expect("lookup")
                .is_none()
        );

        let visual_rows = block_on_db(async {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM visual_module_analysis")
                .fetch_one(&mut conn)
                .await
                .map_err(|err| super::db_error("visual_count_failed", err))
        })
        .expect("visual rows");
        assert_eq!(visual_rows, 0, "old visual analyses must cascade");

        let duplicate_blocks = [block("parse-3", "block-3"), block("parse-3", "block-3")];
        PdfStructureRepository::replace_document_structure(
            &mut conn,
            &run("doc", "parse-3"),
            &[artifact("parse-3")],
            &duplicate_blocks,
        )
        .expect_err("duplicate block must roll back replacement");

        let blocks = PdfStructureRepository::list_indexable_blocks(&mut conn)
            .expect("previous parse survives rollback");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_id, "block-2");
        let parse_ids = block_on_db(async {
            sqlx::query_scalar::<_, String>(
                "SELECT parse_id FROM pdf_parse_runs WHERE document_id = 'doc'",
            )
            .fetch_all(&mut conn)
            .await
            .map_err(|err| super::db_error("parse_ids_failed", err))
        })
        .expect("parse ids");
        assert_eq!(parse_ids, vec!["parse-2".to_string()]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failed_parse_persists_error_and_run_in_one_transaction() {
        let root =
            std::env::temp_dir().join(format!("slicer-pdf-failed-run-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).expect("root");
        let db = root.join("app.db");
        block_on_db(run_migrations(db.clone())).expect("migrate");
        let mut conn = block_on_db(connect_workspace_db(db)).expect("connect");
        seed_document_and_page(&mut conn, "doc", "page");
        let error = crate::errors::AppError::new(
            "opendataloader_fixture_failed",
            "fixture failure",
            "pdf_structure_validate",
            true,
        );

        PdfStructureRepository::record_parse_failure(
            &mut conn,
            &run("doc", "failed-parse"),
            &error,
        )
        .expect("record failed parse");

        let stored = block_on_db(async {
            sqlx::query_as::<_, (String, String, String)>(
                "SELECT pdf_parse_runs.status, pdf_parse_runs.error_id, errors.code
                 FROM pdf_parse_runs
                 JOIN errors ON errors.error_id = pdf_parse_runs.error_id
                 WHERE pdf_parse_runs.parse_id = 'failed-parse'",
            )
            .fetch_one(&mut conn)
            .await
            .map_err(|err| super::db_error("failed_parse_read_failed", err))
        })
        .expect("failed parse row");
        assert_eq!(stored.0, "failed");
        assert!(!stored.1.is_empty());
        assert_eq!(stored.2, "opendataloader_fixture_failed");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn visual_analysis_attempt_token_rejects_stale_completion() {
        let root = std::env::temp_dir().join(format!(
            "slicer-visual-attempt-token-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("root");
        let db = root.join("app.db");
        block_on_db(run_migrations(db.clone())).expect("migrate");
        let mut conn = block_on_db(connect_workspace_db(db)).expect("connect");
        seed_document_and_page(&mut conn, "doc", "page");

        let artifact = DocumentArtifactInput {
            artifact_id: "artifact-parse".to_string(),
            document_id: "doc".to_string(),
            kind: "pdf_structure_json".to_string(),
            relative_path: "structure/parse/document.json".to_string(),
            content_hash: "hash".to_string(),
            parser_name: Some("opendataloader-pdf".to_string()),
            parser_version: Some("2.5.0".to_string()),
            parser_options_json: Some("{}".to_string()),
        };
        let block = PdfContentBlockDto {
            block_id: "block-1".to_string(),
            parse_id: "parse".to_string(),
            document_id: "doc".to_string(),
            page_id: "page".to_string(),
            page_number: 1,
            parent_block_id: None,
            source_element_id: Some("1".to_string()),
            ordinal: 0,
            block_type: "image".to_string(),
            source_text: String::new(),
            enrichment_json: None,
            raw_json: "{}".to_string(),
            source_image_path: Some("structure/parse/images/1.png".to_string()),
            is_indexable: true,
            is_visual: true,
            is_decorative: false,
            bbox: None,
        };
        PdfStructureRepository::replace_document_structure(
            &mut conn,
            &run("doc", "parse"),
            &[artifact],
            &[block],
        )
        .expect("structure");

        let first_attempt = PdfStructureRepository::try_mark_visual_pending(
            &mut conn,
            "block-1",
            "mock",
            "mock-model",
        )
        .expect("first lease")
        .expect("first attempt");
        assert_eq!(first_attempt, 1);
        block_on_db(async {
            sqlx::query(
                "UPDATE visual_module_analysis SET status = 'failed' WHERE block_id = 'block-1'",
            )
            .execute(&mut conn)
            .await
            .map_err(|err| super::db_error("visual_test_recovery_failed", err))?;
            Ok(())
        })
        .expect("simulate recovery");
        let second_attempt = PdfStructureRepository::try_mark_visual_pending(
            &mut conn,
            "block-1",
            "mock",
            "mock-model",
        )
        .expect("second lease")
        .expect("second attempt");
        assert_eq!(second_attempt, 2);

        let enrichment = r#"{"schema_version":"visual_module_analysis_v1","block_id":"block-1","description":"diagram","visible_text":"","keywords":[],"model":{"provider":"mock","model_name":"mock-model"}}"#;
        let stale_success = PdfStructureRepository::save_visual_success(
            &mut conn,
            "block-1",
            first_attempt,
            "mock",
            "mock-model",
            enrichment,
        )
        .expect_err("stale success must not claim a newer lease");
        assert_eq!(stale_success.code, "visual_analysis_lease_missing");

        let error_count_before = block_on_db(async {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM errors")
                .fetch_one(&mut conn)
                .await
                .map_err(|err| super::db_error("visual_test_error_count_failed", err))
        })
        .expect("error count before stale failure");
        let stale_failure = PdfStructureRepository::save_visual_failure(
            &mut conn,
            "block-1",
            first_attempt,
            &crate::errors::AppError::new(
                "stale_worker_failed",
                "stale worker failed",
                "visual_module_analysis",
                true,
            ),
        )
        .expect_err("stale failure must not claim a newer lease");
        assert_eq!(stale_failure.code, "visual_analysis_lease_missing");
        let error_count_after = block_on_db(async {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM errors")
                .fetch_one(&mut conn)
                .await
                .map_err(|err| super::db_error("visual_test_error_count_failed", err))
        })
        .expect("error count after stale failure");
        assert_eq!(error_count_after, error_count_before);

        PdfStructureRepository::save_visual_success(
            &mut conn,
            "block-1",
            second_attempt,
            "mock",
            "mock-model",
            enrichment,
        )
        .expect("current attempt succeeds");
        let stored = block_on_db(async {
            sqlx::query_as::<_, (String, i64, Option<String>)>(
                "SELECT visual_module_analysis.status, visual_module_analysis.attempt_count,
                        content_blocks.enrichment_json
                 FROM visual_module_analysis
                 JOIN content_blocks ON content_blocks.block_id = visual_module_analysis.block_id
                 WHERE visual_module_analysis.block_id = 'block-1'",
            )
            .fetch_one(&mut conn)
            .await
            .map_err(|err| super::db_error("visual_test_state_read_failed", err))
        })
        .expect("stored current attempt");
        assert_eq!(stored.0, "succeeded");
        assert_eq!(stored.1, second_attempt);
        assert_eq!(stored.2.as_deref(), Some(enrichment));

        let _ = fs::remove_dir_all(root);
    }

    fn seed_document_and_page(conn: &mut sqlx::SqliteConnection, document_id: &str, page_id: &str) {
        block_on_db(async {
            let now = chrono::Utc::now().to_rfc3339();
            sqlx::query(
                "INSERT INTO documents
                 (document_id, original_filename, file_type, file_hash, original_path,
                  status, created_at, updated_at)
                 VALUES (?1, 'doc.pdf', 'pdf', ?1, 'originals/doc.pdf', 'ready', ?3, ?3)",
            )
            .bind(document_id)
            .bind(document_id)
            .bind(&now)
            .execute(&mut *conn)
            .await
            .expect("document");
            sqlx::query(
                "INSERT INTO image_assets (image_hash, file_path, file_size, created_at)
                 VALUES ('image', 'pages/image.png', 1, ?1)",
            )
            .bind(&now)
            .execute(&mut *conn)
            .await
            .expect("image");
            sqlx::query(
                "INSERT INTO page_records
                 (page_id, document_id, page_number, image_hash, status, created_at, updated_at)
                 VALUES (?1, ?2, 1, 'image', 'rendered', ?3, ?3)",
            )
            .bind(page_id)
            .bind(document_id)
            .bind(now)
            .execute(conn)
            .await
            .expect("page");
            Ok(())
        })
        .expect("seed");
    }
}
