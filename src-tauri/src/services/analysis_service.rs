use crate::artifacts::jsonl_exporter::ArtifactExporter;
use crate::domain::analysis::{
    AnalysisBatchResultDto, AnalysisResultDto, PageAnalysisContent, PageAnalysisModelInfo,
    PageAnalysisSource, PageAnalysisV1, PageRetrievalFields, ProviderResponseRecord,
    PAGE_ANALYSIS_SCHEMA_VERSION,
};
use crate::domain::page::PageRecordDto;
use crate::domain::settings::AppSettingsDto;
use crate::errors::{AppError, AppResult};
use crate::jobs::job_orchestrator::JobOrchestrator;
use crate::providers::model::anthropic_provider::AnthropicProvider;
use crate::providers::model::custom_http_provider::CustomHttpModelProvider;
use crate::providers::model::mock_provider::MockModelProvider;
use crate::providers::model::openai_provider::OpenAIProvider;
use crate::providers::model::prompt_template::page_analysis_prompt;
use crate::providers::model::provider::{
    ModelAnalysisRequest, ModelAnalysisResponse, ModelProvider,
};
use crate::providers::model::schema_validator::{validate_page_analysis_v1, ExpectedPageContext};
use crate::providers::model::siliconflow_provider::SiliconFlowProvider;
use crate::repositories::analysis_repository::AnalysisRepository;
use crate::repositories::db::{block_on_db, database_error};
use crate::repositories::document_repository::DocumentRepository;
use crate::services::settings_service::SettingsService;
use crate::services::workspace_service::WorkspaceService;
use chrono::Utc;
use serde_json::Value;
use sqlx::SqliteConnection;
use std::collections::VecDeque;
use std::fs;
use std::sync::{Arc, Mutex};

pub struct AnalysisService;

impl AnalysisService {
    pub fn analyze_page(
        workspace: &WorkspaceService,
        page_id: &str,
    ) -> AppResult<AnalysisResultDto> {
        Self::analyze_page_with_provider(workspace, page_id, None)
    }

    pub fn analyze_page_with_provider(
        workspace: &WorkspaceService,
        page_id: &str,
        provider_override: Option<&dyn ModelProvider>,
    ) -> AppResult<AnalysisResultDto> {
        let layout = workspace.workspace_layout()?;
        let orchestrator = JobOrchestrator::new(layout.clone());
        let job = orchestrator.create_job("page_analysis")?;
        let job_id = job.job_id;

        let (_settings, context) = Self::build_analysis_context(workspace)
            .map_err(|err| Self::fail_job_and_page(workspace, &orchestrator, &job_id, None, err))?;

        orchestrator.update_progress(&job_id, 10, Some("page analysis queued"))?;

        let result = Self::analyze_page_core(
            workspace,
            &layout,
            &context,
            page_id,
            true,
            true,
            provider_override,
        )
        .map_err(|err| {
            let page_for_failure = if Self::should_persist_page_failure(&err) {
                Some(page_id)
            } else {
                None
            };
            if page_for_failure.is_some() {
                Self::fail_job_and_page_with_model(
                    workspace,
                    &orchestrator,
                    &job_id,
                    page_for_failure,
                    Some(&context.provider_name),
                    Some(&context.model_name),
                    err,
                )
            } else {
                Self::fail_job_and_page(workspace, &orchestrator, &job_id, None, err)
            }
        })?;

        orchestrator.update_progress(&job_id, 100, Some("页面分析完成"))?;
        Ok(result)
    }

    pub fn analyze_new_pages(workspace: &WorkspaceService) -> AppResult<AnalysisBatchResultDto> {
        Self::analyze_new_pages_with_provider(workspace, None)
    }

    pub fn reanalyze_document(
        workspace: &WorkspaceService,
        document_id: &str,
    ) -> AppResult<AnalysisBatchResultDto> {
        Self::reanalyze_document_with_provider(workspace, document_id, None)
    }

    pub fn reanalyze_failed_pages(
        workspace: &WorkspaceService,
        document_id: &str,
    ) -> AppResult<AnalysisBatchResultDto> {
        Self::reanalyze_failed_pages_with_provider(workspace, document_id, None)
    }

    fn analyze_new_pages_with_provider(
        workspace: &WorkspaceService,
        provider_override: Option<&dyn ModelProvider>,
    ) -> AppResult<AnalysisBatchResultDto> {
        let layout = workspace.workspace_layout()?;
        let orchestrator = JobOrchestrator::new(layout.clone());
        let job = orchestrator.create_job("page_analysis_batch")?;
        let job_id = job.job_id;

        let (settings, context) = Self::build_analysis_context(workspace)
            .map_err(|err| Self::fail_batch_job(&orchestrator, &job_id, err))?;
        let mut conn = workspace
            .get_db_connection()
            .map_err(|err| Self::fail_batch_job(&orchestrator, &job_id, err))?;
        let pages = DocumentRepository::list_pages_needing_analysis(&mut conn)
            .map_err(|err| Self::fail_batch_job(&orchestrator, &job_id, err))?;
        drop(conn);

        Self::run_batch_pages(
            workspace,
            &layout,
            &orchestrator,
            &job_id,
            pages,
            false,
            settings.analysis_concurrency,
            context,
            provider_override,
        )
    }

    fn reanalyze_document_with_provider(
        workspace: &WorkspaceService,
        document_id: &str,
        provider_override: Option<&dyn ModelProvider>,
    ) -> AppResult<AnalysisBatchResultDto> {
        let layout = workspace.workspace_layout()?;
        let mut conn = workspace.get_db_connection()?;
        DocumentRepository::find_document_by_id(&mut conn, document_id)?.ok_or_else(|| {
            AppError::new(
                "document_not_found",
                "document not found",
                "analysis",
                false,
            )
        })?;
        let pages = DocumentRepository::list_pages_by_document(&mut conn, document_id)?;
        drop(conn);

        let orchestrator = JobOrchestrator::new(layout.clone());
        let job = orchestrator.create_job("document_reanalysis")?;
        let job_id = job.job_id;
        let (settings, context) = Self::build_analysis_context(workspace)
            .map_err(|err| Self::fail_batch_job(&orchestrator, &job_id, err))?;

        Self::run_batch_pages(
            workspace,
            &layout,
            &orchestrator,
            &job_id,
            pages,
            true,
            settings.analysis_concurrency,
            context,
            provider_override,
        )
    }

    fn reanalyze_failed_pages_with_provider(
        workspace: &WorkspaceService,
        document_id: &str,
        provider_override: Option<&dyn ModelProvider>,
    ) -> AppResult<AnalysisBatchResultDto> {
        let layout = workspace.workspace_layout()?;
        let mut conn = workspace.get_db_connection()?;
        DocumentRepository::find_document_by_id(&mut conn, document_id)?.ok_or_else(|| {
            AppError::new(
                "document_not_found",
                "document not found",
                "analysis",
                false,
            )
        })?;
        let pages = DocumentRepository::list_failed_pages_by_document(&mut conn, document_id)?;
        drop(conn);

        let orchestrator = JobOrchestrator::new(layout.clone());
        let job = orchestrator.create_job("document_failed_reanalysis")?;
        let job_id = job.job_id;
        let (settings, context) = Self::build_analysis_context(workspace)
            .map_err(|err| Self::fail_batch_job(&orchestrator, &job_id, err))?;

        Self::run_batch_pages(
            workspace,
            &layout,
            &orchestrator,
            &job_id,
            pages,
            true,
            settings.analysis_concurrency,
            context,
            provider_override,
        )
    }

    pub fn recover_interrupted_analysis_pages(workspace: &WorkspaceService) -> AppResult<u64> {
        let layout = workspace.workspace_layout()?;
        let orchestrator = JobOrchestrator::new(layout);
        let mut conn = workspace.get_db_connection()?;
        let pending_pages = DocumentRepository::list_analysis_pending_pages(&mut conn)?;
        let affected = DocumentRepository::recover_analysis_pending_pages(
            &mut conn,
            "interrupted page analysis has been marked failed for retry",
        )?;
        drop(conn);

        if affected == 0 {
            return Ok(0);
        }

        let error = AppError::new(
            "page_analysis_interrupted",
            "interrupted page analysis has been marked failed for retry",
            "analysis_recovery",
            true,
        );
        for page in pending_pages {
            let _ = Self::record_page_failure(
                workspace,
                &orchestrator,
                &page.page_id,
                None,
                None,
                &error,
            );
        }

        Ok(affected)
    }

    fn build_analysis_context(
        workspace: &WorkspaceService,
    ) -> AppResult<(AppSettingsDto, AnalysisExecutionContext)> {
        let settings = SettingsService::get_settings(workspace)?;
        let config_status = SettingsService::get_model_configuration_status(workspace)?;
        if !config_status.configured {
            return Err(AppError::new(
                "model_configuration_incomplete",
                "complete model configuration before analyzing pages",
                "analysis",
                true,
            )
            .with_details(format!("missing={}", config_status.missing.join(","))));
        }
        if config_status.requires_privacy_notice && !config_status.privacy_notice_accepted {
            return Err(AppError::new(
                "privacy_notice_required",
                "accept the privacy notice before calling a remote model",
                "analysis",
                true,
            ));
        }

        let provider_name = settings.model_provider.trim().to_string();
        let endpoint = match provider_name.as_str() {
            "local_mock" => "local://mock".to_string(),
            "openai" => OpenAIProvider::request_endpoint(&settings)?,
            "anthropic" => AnthropicProvider::request_endpoint(&settings)?,
            "siliconflow" => SiliconFlowProvider::request_endpoint(&settings)?,
            _ => CustomHttpModelProvider::request_endpoint(&settings)?,
        };

        Ok((
            settings.clone(),
            AnalysisExecutionContext {
                provider_name,
                model_name: settings.model_name.clone(),
                endpoint,
            },
        ))
    }

    fn analyze_page_core(
        workspace: &WorkspaceService,
        layout: &crate::artifacts::workspace_layout::WorkspaceLayout,
        context: &AnalysisExecutionContext,
        page_id: &str,
        force_reanalysis: bool,
        refresh_jsonl_after_success: bool,
        provider_override: Option<&dyn ModelProvider>,
    ) -> AppResult<AnalysisResultDto> {
        let (expected_page, image_bytes) =
            Self::prepare_page_for_analysis(workspace, layout, page_id, force_reanalysis)?;
        let prompt = if context.provider_name == "siliconflow" {
            Self::siliconflow_image_interpretation_prompt(&expected_page)
        } else {
            page_analysis_prompt(
                "中文",
                &expected_page,
                &context.provider_name,
                &context.model_name,
            )
        };

        let request = ModelAnalysisRequest {
            image_bytes,
            image_mime_type: "image/png".to_string(),
            prompt,
            model_name: context.model_name.clone(),
            provider: context.provider_name.clone(),
            endpoint: context.endpoint.clone(),
            expected_page: expected_page.clone(),
        };

        let default_mock = MockModelProvider;
        let default_openai = OpenAIProvider;
        let default_anthropic = AnthropicProvider;
        let default_siliconflow = SiliconFlowProvider;
        let default_custom = CustomHttpModelProvider;
        let provider: &dyn ModelProvider = if let Some(provider) = provider_override {
            provider
        } else {
            match context.provider_name.as_str() {
                "local_mock" => &default_mock,
                "openai" => &default_openai,
                "anthropic" => &default_anthropic,
                "siliconflow" => &default_siliconflow,
                _ => &default_custom,
            }
        };

        let provider_response = provider.analyze_page(&request)?;
        let analysis = Self::normalize_provider_response(&provider_response, &expected_page)?;
        let result_json = serde_json::to_string(&analysis).map_err(|err| {
            AppError::new(
                "analysis_result_serialize_failed",
                "analysis result serialization failed",
                "analysis",
                false,
            )
            .with_details(err.to_string())
        })?;

        Self::persist_success_result(
            workspace,
            &expected_page.page_id,
            &provider_response.provider,
            &provider_response.model_name,
            &result_json,
            refresh_jsonl_after_success,
        )
    }

    fn normalize_provider_response(
        response: &ModelAnalysisResponse,
        expected_page: &ExpectedPageContext,
    ) -> AppResult<PageAnalysisV1> {
        match validate_page_analysis_v1(&response.raw_json, expected_page) {
            Ok(mut analysis) => {
                if analysis.provider_response.is_none() {
                    analysis.provider_response =
                        Self::provider_response_record(response, &response.provider);
                }
                Ok(analysis)
            }
            Err(err) if Self::can_wrap_model_content(&err, response) => {
                Self::wrap_model_content_as_page_analysis(response, expected_page)
            }
            Err(err) => Err(err),
        }
    }

    fn siliconflow_image_interpretation_prompt(expected_page: &ExpectedPageContext) -> String {
        format!(
            "Describe this page image in Chinese. Include visible text, title-like text, key topics, and useful layout cues. Return plain text only. page_id={}; image_hash={}; image_path={}",
            expected_page.page_id, expected_page.image_hash, expected_page.image_path
        )
    }

    fn can_wrap_model_content(error: &AppError, response: &ModelAnalysisResponse) -> bool {
        response.provider == "siliconflow"
            && matches!(
                error.code.as_str(),
                "analysis_json_invalid"
                    | "analysis_field_missing"
                    | "analysis_schema_version_unsupported"
                    | "analysis_field_invalid"
                    | "analysis_retrieval_text_missing"
            )
    }

    fn wrap_model_content_as_page_analysis(
        response: &ModelAnalysisResponse,
        expected_page: &ExpectedPageContext,
    ) -> AppResult<PageAnalysisV1> {
        let content = response.raw_json.trim();
        if content.is_empty() {
            return Err(AppError::new(
                "model_response_content_empty",
                "Model returned an empty image description.",
                "analysis_provider",
                true,
            ));
        }

        let sanitized_content = Self::truncate_chars(content, 50_000);
        let raw_response = response
            .provider_response_json
            .as_deref()
            .unwrap_or(&response.raw_json);

        Ok(PageAnalysisV1 {
            schema_version: PAGE_ANALYSIS_SCHEMA_VERSION.to_string(),
            page_id: expected_page.page_id.clone(),
            image_hash: expected_page.image_hash.clone(),
            image_path: expected_page.image_path.clone(),
            source: PageAnalysisSource {
                document_id: expected_page.document_id.clone(),
                page_number: expected_page.page_number,
                original_filename: None,
            },
            analysis: PageAnalysisContent {
                title: Some(format!(
                    "Page {} image description",
                    expected_page.page_number
                )),
                summary: Some(Self::summarize_model_content(&sanitized_content)),
                visible_text: Some(sanitized_content.clone()),
                topics: vec!["image description".to_string()],
                keywords: vec![],
            },
            retrieval: PageRetrievalFields {
                bm25_text: sanitized_content,
            },
            model: PageAnalysisModelInfo {
                provider: response.provider.clone(),
                model_name: response.model_name.clone(),
            },
            provider_response: Some(ProviderResponseRecord {
                endpoint_kind: response.provider.clone(),
                raw_json: Self::sanitize_provider_response_json(raw_response),
            }),
        })
    }

    fn provider_response_record(
        response: &ModelAnalysisResponse,
        endpoint_kind: &str,
    ) -> Option<ProviderResponseRecord> {
        response
            .provider_response_json
            .as_deref()
            .map(|raw| ProviderResponseRecord {
                endpoint_kind: endpoint_kind.to_string(),
                raw_json: Self::sanitize_provider_response_json(raw),
            })
    }

    fn prepare_page_for_analysis(
        workspace: &WorkspaceService,
        layout: &crate::artifacts::workspace_layout::WorkspaceLayout,
        page_id: &str,
        force_reanalysis: bool,
    ) -> AppResult<(ExpectedPageContext, Vec<u8>)> {
        let (expected_page, image_path, relative_image_path) = {
            let mut conn = workspace.get_db_connection()?;
            let page =
                DocumentRepository::find_page_by_id(&mut conn, page_id)?.ok_or_else(|| {
                    AppError::new("page_not_found", "page not found", "analysis", false)
                })?;
            if page.status == "analysis_pending" {
                return Err(AppError::new(
                    "page_analysis_already_running",
                    "page analysis is already running",
                    "analysis",
                    true,
                ));
            }
            let document = DocumentRepository::find_document_by_id(&mut conn, &page.document_id)?
                .ok_or_else(|| {
                AppError::new(
                    "document_not_found",
                    "page document not found",
                    "analysis",
                    false,
                )
            })?;
            let image_asset =
                DocumentRepository::find_image_asset_by_hash(&mut conn, &page.image_hash)?
                    .ok_or_else(|| {
                        AppError::new(
                            "image_asset_not_found",
                            "page image asset not found",
                            "analysis",
                            true,
                        )
                    })?;

            let lease_acquired = DocumentRepository::try_mark_page_analysis_pending(
                &mut conn,
                page_id,
                force_reanalysis,
            )?;
            if !lease_acquired {
                return Err(AppError::new(
                    "page_not_eligible_for_analysis",
                    "page is not eligible for analysis",
                    "analysis",
                    true,
                ));
            }

            (
                ExpectedPageContext {
                    page_id: page.page_id,
                    document_id: document.document_id,
                    page_number: page.page_number,
                    image_hash: page.image_hash,
                    image_path: image_asset.file_path.clone(),
                },
                layout.root().join(&image_asset.file_path),
                image_asset.file_path,
            )
        };

        let image_bytes = fs::read(&image_path).map_err(|err| {
            AppError::io("analysis", "page_image_read_failed", err).with_details(format!(
                "page_id={page_id}; image_path={relative_image_path}"
            ))
        })?;

        Ok((expected_page, image_bytes))
    }

    fn run_batch_pages(
        workspace: &WorkspaceService,
        layout: &crate::artifacts::workspace_layout::WorkspaceLayout,
        orchestrator: &JobOrchestrator,
        job_id: &str,
        pages: Vec<PageRecordDto>,
        force_reanalysis: bool,
        analysis_concurrency: u8,
        context: AnalysisExecutionContext,
        provider_override: Option<&dyn ModelProvider>,
    ) -> AppResult<AnalysisBatchResultDto> {
        let total_pages = pages.len() as i64;
        if total_pages == 0 {
            orchestrator.update_progress(job_id, 100, Some("no pages need analysis"))?;
            return Ok(AnalysisBatchResultDto {
                job_id: job_id.to_string(),
                total_pages,
                succeeded_pages: 0,
                failed_pages: 0,
                skipped_pages: 0,
                status: "succeeded".to_string(),
                updated_at: Utc::now().to_rfc3339(),
            });
        }

        orchestrator.update_progress(
            job_id,
            1,
            Some(&Self::batch_progress_message(
                "batch analysis started",
                total_pages,
                0,
                0,
                0,
                None,
            )),
        )?;

        let worker_count = usize::from(analysis_concurrency.clamp(1, 8)).min(pages.len());
        let queue = Arc::new(Mutex::new(VecDeque::from(pages)));
        let counters = Arc::new(Mutex::new(BatchCounters::default()));
        let progress_error = Arc::new(Mutex::new(None));

        std::thread::scope(|scope| {
            for _ in 0..worker_count {
                let queue = Arc::clone(&queue);
                let counters = Arc::clone(&counters);
                let progress_error = Arc::clone(&progress_error);
                let workspace = workspace.clone();
                let layout = layout.clone();
                let job_id = job_id.to_string();
                let context = context.clone();
                let provider_override = provider_override;

                scope.spawn(move || {
                    let orchestrator = JobOrchestrator::new(layout.clone());
                    loop {
                        let page = {
                            let mut queue = queue.lock().expect("analysis queue lock");
                            queue.pop_front()
                        };
                        let Some(page) = page else { break };
                        let outcome = Self::run_batch_page(
                            &workspace,
                            &layout,
                            &orchestrator,
                            &context,
                            &page.page_id,
                            force_reanalysis,
                            provider_override,
                        );

                        let progress = {
                            let mut counters = counters.lock().expect("analysis counters lock");
                            counters.completed_pages += 1;
                            counters.last_page_id = Some(page.page_id.clone());
                            match outcome {
                                BatchPageOutcome::Succeeded => counters.succeeded_pages += 1,
                                BatchPageOutcome::Failed => counters.failed_pages += 1,
                                BatchPageOutcome::Skipped => counters.skipped_pages += 1,
                            }
                            ((counters.completed_pages * 98 / total_pages) + 1)
                                .min(99)
                                .max(1) as u8
                        };

                        let counters_snapshot =
                            counters.lock().expect("analysis counters lock").clone();
                        let message = Self::batch_progress_message(
                            "batch analysis running",
                            total_pages,
                            counters_snapshot.succeeded_pages,
                            counters_snapshot.failed_pages,
                            counters_snapshot.skipped_pages,
                            counters_snapshot.last_page_id.as_deref(),
                        );
                        if let Err(err) =
                            orchestrator.update_progress(&job_id, progress, Some(&message))
                        {
                            let mut slot =
                                progress_error.lock().expect("analysis progress error lock");
                            if slot.is_none() {
                                *slot = Some(err);
                            }
                        }
                    }
                });
            }
        });

        if let Some(err) = progress_error
            .lock()
            .expect("analysis progress error lock")
            .clone()
        {
            return Err(Self::fail_batch_job(orchestrator, job_id, err));
        }

        let counters = counters.lock().expect("analysis counters lock").clone();
        let final_status = if counters.failed_pages == 0 {
            "succeeded"
        } else if counters.succeeded_pages == 0 {
            "failed"
        } else {
            "succeeded_with_failures"
        };
        let message = Self::batch_progress_message(
            "批量分析完成",
            total_pages,
            counters.succeeded_pages,
            counters.failed_pages,
            counters.skipped_pages,
            counters.last_page_id.as_deref(),
        );

        if final_status == "failed" || final_status == "succeeded_with_failures" {
            let err = AppError::new(
                if final_status == "failed" {
                    "analysis_batch_failed"
                } else {
                    "analysis_batch_succeeded_with_failures"
                },
                if final_status == "failed" {
                    "batch analysis failed; no processed pages succeeded"
                } else {
                    "batch analysis completed with failed pages"
                },
                "analysis",
                true,
            );
            let _ = orchestrator.mark_failed(job_id, &err, &message)?;
        } else {
            orchestrator.update_progress(job_id, 100, Some(&message))?;
        }

        if counters.succeeded_pages > 0 {
            Self::refresh_page_jsonl_artifact(workspace);
        }

        Ok(AnalysisBatchResultDto {
            job_id: job_id.to_string(),
            total_pages,
            succeeded_pages: counters.succeeded_pages,
            failed_pages: counters.failed_pages,
            skipped_pages: counters.skipped_pages,
            status: final_status.to_string(),
            updated_at: Utc::now().to_rfc3339(),
        })
    }

    fn run_batch_page(
        workspace: &WorkspaceService,
        layout: &crate::artifacts::workspace_layout::WorkspaceLayout,
        orchestrator: &JobOrchestrator,
        context: &AnalysisExecutionContext,
        page_id: &str,
        force_reanalysis: bool,
        provider_override: Option<&dyn ModelProvider>,
    ) -> BatchPageOutcome {
        match Self::analyze_page_core(
            workspace,
            layout,
            context,
            page_id,
            force_reanalysis,
            false,
            provider_override,
        ) {
            Ok(_) => BatchPageOutcome::Succeeded,
            Err(err)
                if err.code == "page_analysis_already_running"
                    || err.code == "page_not_eligible_for_analysis" =>
            {
                BatchPageOutcome::Skipped
            }
            Err(err) => {
                if Self::should_persist_page_failure(&err) {
                    let _ = Self::record_page_failure(
                        workspace,
                        orchestrator,
                        page_id,
                        Some(&context.provider_name),
                        Some(&context.model_name),
                        &err,
                    );
                }
                BatchPageOutcome::Failed
            }
        }
    }

    fn batch_progress_message(
        phase: &str,
        total_pages: i64,
        succeeded_pages: i64,
        failed_pages: i64,
        skipped_pages: i64,
        last_page_id: Option<&str>,
    ) -> String {
        let last_page = last_page_id.unwrap_or("-");
        format!(
            "{phase}: total_pages={total_pages}; succeeded_pages={succeeded_pages}; failed_pages={failed_pages}; skipped_pages={skipped_pages}; last_page={last_page}; updated_at={}",
            Utc::now().to_rfc3339()
        )
    }

    fn persist_success_result(
        workspace: &WorkspaceService,
        page_id: &str,
        provider: &str,
        model_name: &str,
        result_json: &str,
        refresh_jsonl: bool,
    ) -> AppResult<AnalysisResultDto> {
        let mut conn = workspace.get_db_connection()?;
        Self::begin_transaction(&mut conn)?;
        let result = (|| {
            let page =
                DocumentRepository::find_page_by_id(&mut conn, page_id)?.ok_or_else(|| {
                    AppError::new("page_not_found", "page not found", "analysis", false)
                })?;
            if page.status != "analysis_pending" {
                return Err(AppError::new(
                    "stale_page_analysis_lease_lost",
                    "page analysis lease was lost; result was not written",
                    "analysis",
                    true,
                ));
            }
            let result = AnalysisRepository::save_success_result(
                &mut conn,
                page_id,
                PAGE_ANALYSIS_SCHEMA_VERSION,
                provider,
                model_name,
                result_json,
            )?;
            DocumentRepository::update_page_status(&mut conn, page_id, "analyzed", None)?;
            Ok(result)
        })();

        match result {
            Ok(result) => {
                Self::commit_transaction(&mut conn)?;
                if refresh_jsonl {
                    Self::refresh_page_jsonl_artifact(workspace);
                }
                Ok(result)
            }
            Err(err) => {
                let _ = Self::rollback_transaction(&mut conn);
                Err(err)
            }
        }
    }

    fn refresh_page_jsonl_artifact(workspace: &WorkspaceService) {
        if let Err(err) = ArtifactExporter::export_pages(workspace) {
            tracing::warn!(
                code = %err.code,
                correlation_id = %err.correlation_id,
                "page JSONL artifact export failed after analysis result was written"
            );
        }
    }

    fn record_page_failure(
        workspace: &WorkspaceService,
        orchestrator: &JobOrchestrator,
        page_id: &str,
        provider: Option<&str>,
        model_name: Option<&str>,
        error: &AppError,
    ) -> AppResult<()> {
        let error_id = orchestrator.record_error(error)?;
        let summary = Self::failure_summary(error);
        let mut conn = workspace.get_db_connection()?;
        DocumentRepository::update_page_status(&mut conn, page_id, "failed", Some(&summary))?;
        AnalysisRepository::save_failure_result(
            &mut conn,
            page_id,
            PAGE_ANALYSIS_SCHEMA_VERSION,
            provider.unwrap_or("unknown"),
            model_name.unwrap_or("unknown"),
            &error_id,
        )?;
        Self::refresh_page_jsonl_artifact(workspace);
        Ok(())
    }

    fn should_persist_page_failure(error: &AppError) -> bool {
        !matches!(
            error.code.as_str(),
            "page_analysis_already_running"
                | "page_not_eligible_for_analysis"
                | "stale_page_analysis_lease_lost"
        )
    }

    fn fail_batch_job(orchestrator: &JobOrchestrator, job_id: &str, error: AppError) -> AppError {
        let summary = Self::failure_summary(&error);
        let _ = orchestrator.mark_failed(job_id, &error, &summary);
        error
    }

    fn failure_summary(error: &AppError) -> String {
        let base = format!("{} 诊断编号: {}", error.message, error.correlation_id);
        if let Some(details) = &error.details {
            let preview = if details.len() > 200 {
                format!("{}...", &details[..200])
            } else {
                details.clone()
            };
            format!("{} {}", base, preview)
        } else {
            base
        }
    }

    fn begin_transaction(conn: &mut SqliteConnection) -> AppResult<()> {
        block_on_db(async {
            sqlx::query("BEGIN IMMEDIATE")
                .execute(&mut *conn)
                .await
                .map_err(|err| {
                    database_error("analysis", "analysis_transaction_begin_failed", err)
                })?;
            Ok(())
        })
    }

    fn commit_transaction(conn: &mut SqliteConnection) -> AppResult<()> {
        block_on_db(async {
            sqlx::query("COMMIT")
                .execute(&mut *conn)
                .await
                .map_err(|err| {
                    database_error("analysis", "analysis_transaction_commit_failed", err)
                })?;
            Ok(())
        })
    }

    fn rollback_transaction(conn: &mut SqliteConnection) -> AppResult<()> {
        block_on_db(async {
            sqlx::query("ROLLBACK")
                .execute(&mut *conn)
                .await
                .map_err(|err| {
                    database_error("analysis", "analysis_transaction_rollback_failed", err)
                })?;
            Ok(())
        })
    }

    fn fail_job_and_page(
        workspace: &WorkspaceService,
        orchestrator: &JobOrchestrator,
        job_id: &str,
        page_id: Option<&str>,
        error: AppError,
    ) -> AppError {
        Self::fail_job_and_page_with_model(
            workspace,
            orchestrator,
            job_id,
            page_id,
            None,
            None,
            error,
        )
    }

    fn fail_job_and_page_with_model(
        workspace: &WorkspaceService,
        orchestrator: &JobOrchestrator,
        job_id: &str,
        page_id: Option<&str>,
        provider: Option<&str>,
        model_name: Option<&str>,
        error: AppError,
    ) -> AppError {
        let summary = Self::failure_summary(&error);

        match page_id {
            Some(page_id) if Self::should_persist_page_failure(&error) => {
                match orchestrator.mark_failed(job_id, &error, &summary) {
                    Ok(job) => {
                        if let Some(error_id) = job.error_id {
                            if let Ok(mut conn) = workspace.get_db_connection() {
                                if Self::write_page_failure_result(
                                    &mut conn, page_id, &summary, provider, model_name, &error_id,
                                )
                                .is_err()
                                {
                                    let _ = Self::record_page_failure(
                                        workspace,
                                        orchestrator,
                                        page_id,
                                        provider,
                                        model_name,
                                        &error,
                                    );
                                }
                            }
                        } else {
                            let _ = Self::record_page_failure(
                                workspace,
                                orchestrator,
                                page_id,
                                provider,
                                model_name,
                                &error,
                            );
                        }
                    }
                    Err(_) => {
                        let _ = Self::record_page_failure(
                            workspace,
                            orchestrator,
                            page_id,
                            provider,
                            model_name,
                            &error,
                        );
                        let _ = orchestrator.mark_failed(job_id, &error, &summary);
                    }
                }
            }
            _ => {
                let _ = orchestrator.mark_failed(job_id, &error, &summary);
            }
        }

        error
    }

    fn write_page_failure_result(
        conn: &mut SqliteConnection,
        page_id: &str,
        summary: &str,
        provider: Option<&str>,
        model_name: Option<&str>,
        error_id: &str,
    ) -> AppResult<()> {
        DocumentRepository::update_page_status(conn, page_id, "failed", Some(summary))?;
        AnalysisRepository::save_failure_result(
            conn,
            page_id,
            PAGE_ANALYSIS_SCHEMA_VERSION,
            provider.unwrap_or("unknown"),
            model_name.unwrap_or("unknown"),
            error_id,
        )?;
        Ok(())
    }

    fn summarize_model_content(content: &str) -> String {
        let first_line = content
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or(content.trim());
        Self::truncate_chars(first_line, 240)
    }

    fn truncate_chars(value: &str, max_chars: usize) -> String {
        value.chars().take(max_chars).collect()
    }

    fn sanitize_provider_response_json(raw_json: &str) -> String {
        let parsed = match serde_json::from_str::<Value>(raw_json) {
            Ok(value) => value,
            Err(_) => return format!("provider_response_bytes={}", raw_json.len()),
        };
        let sanitized = Self::sanitize_provider_response_value(parsed);
        serde_json::to_string(&sanitized)
            .unwrap_or_else(|_| format!("provider_response_bytes={}", raw_json.len()))
    }

    fn sanitize_provider_response_value(value: Value) -> Value {
        match value {
            Value::Object(map) => {
                let mut sanitized = serde_json::Map::new();
                for (key, value) in map {
                    if Self::is_sensitive_response_key(&key) {
                        sanitized.insert(key, Value::String("[redacted]".to_string()));
                    } else {
                        sanitized.insert(key, Self::sanitize_provider_response_value(value));
                    }
                }
                Value::Object(sanitized)
            }
            Value::Array(items) => Value::Array(
                items
                    .into_iter()
                    .map(Self::sanitize_provider_response_value)
                    .collect(),
            ),
            Value::String(text) if Self::looks_like_base64_data_url(&text) => {
                Value::String("[redacted-image-data-url]".to_string())
            }
            Value::String(text) => Value::String(Self::truncate_chars(&text, 50_000)),
            other => other,
        }
    }

    fn is_sensitive_response_key(key: &str) -> bool {
        let normalized = key.to_ascii_lowercase();
        normalized.contains("api_key")
            || normalized.contains("authorization")
            || normalized.contains("token")
            || normalized.contains("secret")
            || normalized == "image_base64"
            || normalized == "data"
    }

    fn looks_like_base64_data_url(value: &str) -> bool {
        let lower = value
            .chars()
            .take(32)
            .collect::<String>()
            .to_ascii_lowercase();
        lower.starts_with("data:image/") && lower.contains(";base64,")
    }
}

#[derive(Clone)]
struct AnalysisExecutionContext {
    provider_name: String,
    model_name: String,
    endpoint: String,
}

#[derive(Default, Clone)]
struct BatchCounters {
    completed_pages: i64,
    succeeded_pages: i64,
    failed_pages: i64,
    skipped_pages: i64,
    last_page_id: Option<String>,
}

enum BatchPageOutcome {
    Succeeded,
    Failed,
    Skipped,
}

#[cfg(test)]
mod tests {
    use super::AnalysisService;
    use crate::api::state::ApiAppState;
    use crate::domain::analysis::PAGE_ANALYSIS_SCHEMA_VERSION;
    use crate::domain::settings::AppSettingsDto;
    use crate::errors::{AppError, AppResult};
    use crate::jobs::job_orchestrator::JobOrchestrator;
    use crate::providers::model::mock_provider::MockModelProvider;
    use crate::providers::model::provider::{
        ModelAnalysisRequest, ModelAnalysisResponse, ModelProvider,
    };
    use crate::repositories::analysis_repository::AnalysisRepository;
    use crate::repositories::db::block_on_db;
    use crate::repositories::document_repository::DocumentRepository;
    use crate::repositories::workspace_settings_repository::WorkspaceSettingsRepository;
    use crate::services::api_server_service::ApiServerService;
    use crate::services::workspace_service::WorkspaceService;
    use std::fs;
    use std::sync::Arc;

    fn test_state(config_dir: &std::path::Path) -> ApiAppState {
        ApiAppState::new(Arc::new(WorkspaceService::new(config_dir.to_path_buf())))
    }

    fn test_workspace() -> (WorkspaceService, std::path::PathBuf) {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "slicer-analysis-svc-{}-{nonce}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let config = root.join("config");
        let workspace_root = root.join("workspace");
        fs::create_dir_all(&config).expect("config");
        let service = WorkspaceService::new(config);
        let api = ApiServerService::new(test_state(&root.join("config")));
        let status = service.select_workspace(workspace_root.to_string_lossy().into_owned(), &api);
        assert_eq!(status.status, "ready");
        (service, root)
    }

    fn configure_mock(service: &WorkspaceService) {
        configure_mock_with_concurrency(service, 2);
    }

    fn configure_mock_with_concurrency(service: &WorkspaceService, analysis_concurrency: u8) {
        let layout = service.current_layout().expect("layout");
        let mut settings = AppSettingsDto::default();
        settings.model_provider = "local_mock".to_string();
        settings.model_name = "mock".to_string();
        settings.analysis_concurrency = analysis_concurrency;
        WorkspaceSettingsRepository::new(layout)
            .save_workspace_settings(&settings.workspace_record())
            .expect("settings");
    }

    fn seed_page(service: &WorkspaceService, write_image: bool) -> String {
        seed_document_page(
            service,
            "sample.pdf",
            "file-hash",
            1,
            "image-hash",
            write_image,
        )
        .1
    }

    fn seed_document_page(
        service: &WorkspaceService,
        filename: &str,
        file_hash: &str,
        page_number: i64,
        image_hash: &str,
        write_image: bool,
    ) -> (String, String) {
        let mut conn = service.get_db_connection().expect("connection");
        let document = DocumentRepository::create_document(
            &mut conn,
            filename,
            "pdf",
            file_hash,
            &format!("originals/{filename}"),
            None,
        )
        .expect("document");
        let image_path = format!("pages/{}/{image_hash}.png", document.document_id);
        if write_image {
            let layout = service.current_layout().expect("layout");
            let absolute = layout.root().join(&image_path);
            fs::create_dir_all(absolute.parent().expect("parent")).expect("page dir");
            fs::write(&absolute, b"png-bytes").expect("image");
        }
        DocumentRepository::create_image_asset(&mut conn, image_hash, &image_path, 9)
            .expect("image asset");
        let page = DocumentRepository::create_page_record(
            &mut conn,
            &document.document_id,
            page_number,
            image_hash,
        )
        .expect("page")
        .page_id;
        DocumentRepository::update_document_status(
            &mut conn,
            &document.document_id,
            "ready",
            Some(page_number),
            None,
        )
        .expect("document ready");
        (document.document_id, page)
    }

    fn error_count(service: &WorkspaceService, error_id: &str) -> i64 {
        let mut conn = service.get_db_connection().expect("connection");
        block_on_db(async {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM errors WHERE error_id = ?1")
                .bind(error_id)
                .fetch_one(&mut conn)
                .await
                .map_err(|err| {
                    crate::repositories::db::database_error("test", "error_count_failed", err)
                })
        })
        .expect("error count")
    }

    fn error_details(service: &WorkspaceService, error_id: &str) -> Option<String> {
        let mut conn = service.get_db_connection().expect("connection");
        block_on_db(async {
            sqlx::query_scalar::<_, Option<String>>(
                "SELECT details FROM errors WHERE error_id = ?1",
            )
            .bind(error_id)
            .fetch_one(&mut conn)
            .await
            .map_err(|err| {
                crate::repositories::db::database_error("test", "error_details_failed", err)
            })
        })
        .expect("error details")
    }

    #[test]
    fn mock_provider_success_writes_result_and_marks_page_analyzed() {
        let (service, root) = test_workspace();
        configure_mock(&service);
        let page_id = seed_page(&service, true);

        let result = AnalysisService::analyze_page_with_provider(
            &service,
            &page_id,
            Some(&MockModelProvider),
        )
        .expect("analysis");

        assert_eq!(result.status, "succeeded");
        let mut conn = service.get_db_connection().expect("connection");
        let page = DocumentRepository::find_page_by_id(&mut conn, &page_id)
            .expect("lookup")
            .expect("page");
        assert_eq!(page.status, "analyzed");
        let current = AnalysisRepository::find_current_by_page_id(&mut conn, &page_id)
            .expect("analysis lookup")
            .expect("result");
        assert_eq!(current.status, "succeeded");
        let jobs = JobOrchestrator::new(service.current_layout().expect("layout"))
            .list_jobs()
            .expect("jobs");
        let job = jobs
            .iter()
            .find(|job| job.job_type == "page_analysis")
            .expect("page analysis job");
        assert_eq!(job.last_event_message.as_deref(), Some("页面分析完成"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn siliconflow_plain_caption_is_wrapped_and_records_provider_response() {
        let (service, root) = test_workspace();
        let page_id = seed_page(&service, true);
        let provider = PlainCaptionProvider;
        let layout = service.current_layout().expect("layout");
        let context = super::AnalysisExecutionContext {
            provider_name: "siliconflow".to_string(),
            model_name: "zai-org/GLM-4.6V".to_string(),
            endpoint: "test://siliconflow".to_string(),
        };

        let result = AnalysisService::analyze_page_core(
            &service,
            &layout,
            &context,
            &page_id,
            true,
            true,
            Some(&provider),
        )
        .expect("siliconflow caption analysis");

        assert_eq!(result.status, "succeeded");
        let result_json = result.result_json.expect("result json");
        let parsed: serde_json::Value =
            serde_json::from_str(&result_json).expect("wrapped analysis json");
        assert_eq!(parsed["schema_version"], PAGE_ANALYSIS_SCHEMA_VERSION);
        assert_eq!(parsed["page_id"], page_id);
        assert_eq!(
            parsed["analysis"]["visible_text"],
            "This image shows a Chinese document page with a title and paragraphs."
        );
        assert_eq!(parsed["provider_response"]["endpoint_kind"], "siliconflow");
        let raw_response = parsed["provider_response"]["raw_json"]
            .as_str()
            .expect("raw provider json");
        assert!(raw_response.contains("019bda85c39aba6a5fccce598dac8587"));
        assert!(raw_response.contains("This image shows a Chinese document page"));
        assert!(!raw_response.contains("data:image/png;base64"));

        let mut conn = service.get_db_connection().expect("connection");
        let page = DocumentRepository::find_page_by_id(&mut conn, &page_id)
            .expect("page lookup")
            .expect("page");
        assert_eq!(page.status, "analyzed");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn validator_rejects_mismatched_page_and_does_not_write_success() {
        let (service, root) = test_workspace();
        configure_mock(&service);
        let page_id = seed_page(&service, true);
        let provider = MismatchProvider;

        let err = AnalysisService::analyze_page_with_provider(&service, &page_id, Some(&provider))
            .expect_err("mismatch should fail");

        assert_eq!(err.code, "analysis_page_id_mismatch");
        let mut conn = service.get_db_connection().expect("connection");
        let page = DocumentRepository::find_page_by_id(&mut conn, &page_id)
            .expect("page lookup")
            .expect("page");
        assert_eq!(page.status, "failed");
        assert!(page
            .error_summary
            .as_deref()
            .unwrap_or_default()
            .contains(&err.correlation_id));
        let current = AnalysisRepository::find_current_by_page_id(&mut conn, &page_id)
            .expect("analysis lookup")
            .expect("failure result");
        assert_eq!(current.status, "failed");
        assert!(current.error_id.is_some());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn provider_failure_persists_failed_result_with_shared_error_id_and_safe_diagnostics() {
        let (service, root) = test_workspace();
        configure_mock(&service);
        let page_id = seed_page(&service, true);
        let provider = SecretFailureProvider;

        let err = AnalysisService::analyze_page_with_provider(&service, &page_id, Some(&provider))
            .expect_err("provider should fail");

        assert_eq!(err.code, "model_request_failed");
        let layout = service.current_layout().expect("layout");
        let jobs = JobOrchestrator::new(layout).list_jobs().expect("jobs");
        let failed_job = jobs
            .iter()
            .find(|job| job.job_type == "page_analysis")
            .expect("page analysis job");
        assert_eq!(failed_job.status, "failed");
        let mut conn = service.get_db_connection().expect("connection");
        let page = DocumentRepository::find_page_by_id(&mut conn, &page_id)
            .expect("page lookup")
            .expect("page");
        assert_eq!(page.status, "failed");
        assert!(page
            .error_summary
            .as_deref()
            .unwrap_or_default()
            .contains(&err.correlation_id));
        let current = AnalysisRepository::find_current_by_page_id(&mut conn, &page_id)
            .expect("analysis lookup")
            .expect("failure result");
        assert_eq!(current.status, "failed");
        assert_eq!(current.error_id, failed_job.error_id);
        let details = error_details(&service, current.error_id.as_deref().expect("error id"))
            .expect("stored details");
        assert!(!details.contains("Authorization"));
        assert!(!details.contains("sk-secret"));
        assert!(!details.contains("raw model body"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn missing_image_file_returns_structured_error() {
        let (service, root) = test_workspace();
        configure_mock(&service);
        let page_id = seed_page(&service, false);

        let err = AnalysisService::analyze_page_with_provider(
            &service,
            &page_id,
            Some(&MockModelProvider),
        )
        .expect_err("missing image");

        assert_eq!(err.code, "page_image_read_failed");
        assert_eq!(err.stage, "analysis");
        let mut conn = service.get_db_connection().expect("connection");
        let page = DocumentRepository::find_page_by_id(&mut conn, &page_id)
            .expect("lookup")
            .expect("page");
        assert_eq!(page.status, "failed");
        let current = AnalysisRepository::find_current_by_page_id(&mut conn, &page_id)
            .expect("analysis lookup")
            .expect("failure result");
        assert_eq!(current.status, "failed");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn incomplete_configuration_fails_before_provider_call() {
        let (service, root) = test_workspace();
        let page_id = seed_page(&service, true);

        let err = AnalysisService::analyze_page_with_provider(
            &service,
            &page_id,
            Some(&MockModelProvider),
        )
        .expect_err("missing config");

        assert_eq!(err.code, "model_configuration_incomplete");
        let mut conn = service.get_db_connection().expect("connection");
        let page = DocumentRepository::find_page_by_id(&mut conn, &page_id)
            .expect("lookup")
            .expect("page");
        assert_eq!(page.status, "rendered");
        assert!(
            AnalysisRepository::find_current_by_page_id(&mut conn, &page_id)
                .expect("analysis lookup")
                .is_none()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn duplicate_page_analysis_is_rejected_without_marking_page_failed() {
        let (service, root) = test_workspace();
        configure_mock(&service);
        let page_id = seed_page(&service, true);
        let mut conn = service.get_db_connection().expect("connection");
        DocumentRepository::update_page_status(&mut conn, &page_id, "analysis_pending", None)
            .expect("pending");
        drop(conn);

        let err = AnalysisService::analyze_page_with_provider(
            &service,
            &page_id,
            Some(&MockModelProvider),
        )
        .expect_err("already running");

        assert_eq!(err.code, "page_analysis_already_running");
        let mut conn = service.get_db_connection().expect("connection");
        let page = DocumentRepository::find_page_by_id(&mut conn, &page_id)
            .expect("lookup")
            .expect("page");
        assert_eq!(page.status, "analysis_pending");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failed_page_retry_success_clears_current_error_and_keeps_error_history() {
        let (service, root) = test_workspace();
        configure_mock(&service);
        let page_id = seed_page(&service, true);
        let provider = MismatchProvider;

        let first_err =
            AnalysisService::analyze_page_with_provider(&service, &page_id, Some(&provider))
                .expect_err("first analysis should fail");
        assert_eq!(first_err.code, "analysis_page_id_mismatch");
        let old_error_id = {
            let mut conn = service.get_db_connection().expect("connection");
            AnalysisRepository::find_current_by_page_id(&mut conn, &page_id)
                .expect("failed lookup")
                .expect("failed result")
                .error_id
                .expect("old error id")
        };
        assert_eq!(error_count(&service, &old_error_id), 1);

        let retry = AnalysisService::analyze_page_with_provider(
            &service,
            &page_id,
            Some(&MockModelProvider),
        )
        .expect("retry should succeed");

        assert_eq!(retry.status, "succeeded");
        let mut conn = service.get_db_connection().expect("connection");
        let page = DocumentRepository::find_page_by_id(&mut conn, &page_id)
            .expect("page lookup")
            .expect("page");
        assert_eq!(page.status, "analyzed");
        assert!(page.error_summary.is_none());
        let current = AnalysisRepository::find_current_by_page_id(&mut conn, &page_id)
            .expect("current lookup")
            .expect("current result");
        assert_eq!(current.status, "succeeded");
        assert!(current.error_id.is_none());
        assert!(current.result_json.is_some());
        assert_eq!(error_count(&service, &old_error_id), 1);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn batch_analyzes_only_pages_without_current_success() {
        let (service, root) = test_workspace();
        configure_mock(&service);
        let fresh_page = seed_page(&service, true);
        let (_doc_id, analyzed_page) =
            seed_document_page(&service, "done.pdf", "file-hash-2", 1, "image-hash-2", true);
        let (_pending_doc_id, pending_page) = seed_document_page(
            &service,
            "pending.pdf",
            "file-hash-3",
            1,
            "image-hash-3",
            true,
        );
        {
            let mut conn = service.get_db_connection().expect("connection");
            AnalysisRepository::save_success_result(
                &mut conn,
                &analyzed_page,
                PAGE_ANALYSIS_SCHEMA_VERSION,
                "local_mock",
                "mock",
                r#"{"ok":true}"#,
            )
            .expect("existing success");
            DocumentRepository::update_page_status(
                &mut conn,
                &pending_page,
                "analysis_pending",
                None,
            )
            .expect("mark pending");
        }

        let result =
            AnalysisService::analyze_new_pages_with_provider(&service, Some(&MockModelProvider))
                .expect("batch analysis");

        assert_eq!(result.total_pages, 1);
        assert_eq!(result.succeeded_pages, 1);
        assert_eq!(result.failed_pages, 0);
        let mut conn = service.get_db_connection().expect("connection");
        assert_eq!(
            AnalysisRepository::find_current_by_page_id(&mut conn, &fresh_page)
                .expect("fresh lookup")
                .expect("fresh result")
                .status,
            "succeeded"
        );
        assert_eq!(
            AnalysisRepository::find_current_by_page_id(&mut conn, &analyzed_page)
                .expect("existing lookup")
                .expect("existing result")
                .result_json
                .as_deref(),
            Some(r#"{"ok":true}"#)
        );
        let pending = DocumentRepository::find_page_by_id(&mut conn, &pending_page)
            .expect("pending lookup")
            .expect("pending page");
        assert_eq!(pending.status, "analysis_pending");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reanalyze_document_only_overwrites_target_document_pages() {
        let (service, root) = test_workspace();
        configure_mock(&service);
        let (target_doc, target_page) = seed_document_page(
            &service,
            "target.pdf",
            "target-hash",
            1,
            "target-image",
            true,
        );
        let (_other_doc, other_page) =
            seed_document_page(&service, "other.pdf", "other-hash", 1, "other-image", true);
        {
            let mut conn = service.get_db_connection().expect("connection");
            for page_id in [&target_page, &other_page] {
                AnalysisRepository::save_success_result(
                    &mut conn,
                    page_id,
                    PAGE_ANALYSIS_SCHEMA_VERSION,
                    "local_mock",
                    "old-model",
                    r#"{"old":true}"#,
                )
                .expect("existing success");
                DocumentRepository::update_page_status(&mut conn, page_id, "analyzed", None)
                    .expect("mark analyzed");
            }
        }

        let result = AnalysisService::reanalyze_document_with_provider(
            &service,
            &target_doc,
            Some(&MockModelProvider),
        )
        .expect("document reanalysis");

        assert_eq!(result.total_pages, 1);
        assert_eq!(result.succeeded_pages, 1);
        let mut conn = service.get_db_connection().expect("connection");
        let target_result = AnalysisRepository::find_current_by_page_id(&mut conn, &target_page)
            .expect("target lookup")
            .expect("target result");
        let other_result = AnalysisRepository::find_current_by_page_id(&mut conn, &other_page)
            .expect("other lookup")
            .expect("other result");
        assert_eq!(target_result.status, "succeeded");
        assert_ne!(
            target_result.result_json.as_deref(),
            Some(r#"{"old":true}"#)
        );
        assert_eq!(other_result.result_json.as_deref(), Some(r#"{"old":true}"#));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn recovery_marks_leftover_analysis_pending_pages_failed() {
        let (service, root) = test_workspace();
        configure_mock(&service);
        let page_id = seed_page(&service, true);
        {
            let mut conn = service.get_db_connection().expect("connection");
            DocumentRepository::update_page_status(&mut conn, &page_id, "analysis_pending", None)
                .expect("pending");
        }

        let recovered = AnalysisService::recover_interrupted_analysis_pages(&service)
            .expect("recover pending pages");

        assert_eq!(recovered, 1);
        let mut conn = service.get_db_connection().expect("connection");
        let page = DocumentRepository::find_page_by_id(&mut conn, &page_id)
            .expect("lookup")
            .expect("page");
        assert_eq!(page.status, "failed");
        assert!(page.error_summary.unwrap_or_default().contains("retry"));
        let current = AnalysisRepository::find_current_by_page_id(&mut conn, &page_id)
            .expect("analysis lookup")
            .expect("failure result");
        assert_eq!(current.status, "failed");
        assert!(current.error_id.is_some());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn batch_respects_configured_analysis_concurrency() {
        let (service, root) = test_workspace();
        configure_mock_with_concurrency(&service, 1);
        for i in 0..4 {
            let filename = format!("doc-{i}.pdf");
            let file_hash = format!("file-hash-{i}");
            let image_hash = format!("image-hash-{i}");
            seed_document_page(&service, &filename, &file_hash, 1, &image_hash, true);
        }
        let provider = CountingProvider::new();

        let result = AnalysisService::analyze_new_pages_with_provider(&service, Some(&provider))
            .expect("batch analysis");

        assert_eq!(result.total_pages, 4);
        assert_eq!(
            provider.max_seen.load(std::sync::atomic::Ordering::SeqCst),
            1
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn batch_preserves_successes_when_one_page_fails() {
        let (service, root) = test_workspace();
        configure_mock(&service);
        let ok_page = seed_page(&service, true);
        let missing_image_page = seed_document_page(
            &service,
            "missing.pdf",
            "missing-hash",
            1,
            "missing-image",
            false,
        )
        .1;

        let result =
            AnalysisService::analyze_new_pages_with_provider(&service, Some(&MockModelProvider))
                .expect("batch partial failure");

        assert_eq!(result.total_pages, 2);
        assert_eq!(result.succeeded_pages, 1);
        assert_eq!(result.failed_pages, 1);
        assert_eq!(result.status, "succeeded_with_failures");
        let layout = service.current_layout().expect("layout");
        let jobs = JobOrchestrator::new(layout).list_jobs().expect("jobs");
        let batch_job = jobs
            .iter()
            .find(|job| job.job_id == result.job_id)
            .expect("batch job");
        assert_eq!(batch_job.status, "failed");
        assert!(batch_job
            .error_summary
            .as_deref()
            .unwrap_or_default()
            .contains("failed_pages=1"));
        let mut conn = service.get_db_connection().expect("connection");
        let ok_current = AnalysisRepository::find_current_by_page_id(&mut conn, &ok_page)
            .expect("ok lookup")
            .expect("ok result");
        let failed_current =
            AnalysisRepository::find_current_by_page_id(&mut conn, &missing_image_page)
                .expect("failed lookup")
                .expect("failed result");
        assert_eq!(ok_current.status, "succeeded");
        assert_eq!(failed_current.status, "failed");
        assert!(failed_current.error_id.is_some());

        let _ = fs::remove_dir_all(root);
    }

    struct MismatchProvider;

    impl ModelProvider for MismatchProvider {
        fn analyze_page(&self, request: &ModelAnalysisRequest) -> AppResult<ModelAnalysisResponse> {
            let expected = &request.expected_page;
            let raw_json = serde_json::json!({
                "schema_version": PAGE_ANALYSIS_SCHEMA_VERSION,
                "page_id": "wrong-page",
                "image_hash": expected.image_hash,
                "image_path": expected.image_path,
                "source": {
                    "document_id": expected.document_id,
                    "page_number": expected.page_number,
                    "original_filename": null
                },
                "analysis": {
                    "title": "bad",
                    "summary": "bad",
                    "visible_text": "bad",
                    "topics": [],
                    "keywords": []
                },
                "retrieval": {
                    "bm25_text": "bad"
                },
                "model": {
                    "provider": request.provider,
                    "model_name": request.model_name
                }
            })
            .to_string();

            Ok(ModelAnalysisResponse {
                raw_json,
                provider: request.provider.clone(),
                model_name: request.model_name.clone(),
                provider_response_json: None,
            })
        }
    }

    struct PlainCaptionProvider;

    impl ModelProvider for PlainCaptionProvider {
        fn analyze_page(&self, request: &ModelAnalysisRequest) -> AppResult<ModelAnalysisResponse> {
            let provider_response_json = serde_json::json!({
                "id": "019bda85c39aba6a5fccce598dac8587",
                "object": "chat.completion",
                "created": 1768897758_i64,
                "model": request.model_name,
                "choices": [
                    {
                        "index": 0,
                        "message": {
                            "role": "assistant",
                            "content": "This image shows a Chinese document page with a title and paragraphs.",
                            "reasoning_content": "The image visibly contains a document layout."
                        },
                        "finish_reason": "stop"
                    }
                ],
                "usage": {
                    "prompt_tokens": 1383,
                    "completion_tokens": 205,
                    "total_tokens": 1588
                },
                "system_fingerprint": ""
            })
            .to_string();

            Ok(ModelAnalysisResponse {
                raw_json: "This image shows a Chinese document page with a title and paragraphs."
                    .to_string(),
                provider: request.provider.clone(),
                model_name: request.model_name.clone(),
                provider_response_json: Some(provider_response_json),
            })
        }
    }

    struct SecretFailureProvider;

    impl ModelProvider for SecretFailureProvider {
        fn analyze_page(
            &self,
            _request: &ModelAnalysisRequest,
        ) -> AppResult<ModelAnalysisResponse> {
            Err(AppError::new(
                "model_request_failed",
                "model provider call failed",
                "analysis_provider",
                true,
            )
            .with_details("Authorization: Bearer sk-secret; raw model body omitted"))
        }
    }

    struct CountingProvider {
        current: std::sync::atomic::AtomicUsize,
        max_seen: std::sync::atomic::AtomicUsize,
    }

    impl CountingProvider {
        fn new() -> Self {
            Self {
                current: std::sync::atomic::AtomicUsize::new(0),
                max_seen: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    impl ModelProvider for CountingProvider {
        fn analyze_page(&self, request: &ModelAnalysisRequest) -> AppResult<ModelAnalysisResponse> {
            let current = self
                .current
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                + 1;
            self.max_seen
                .fetch_max(current, std::sync::atomic::Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(10));
            let result = MockModelProvider.analyze_page(request);
            self.current
                .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            result
        }
    }
}
