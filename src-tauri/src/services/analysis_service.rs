use crate::artifacts::jsonl_exporter::ArtifactExporter;
use crate::domain::analysis::{
    AnalysisBatchResultDto, AnalysisResultDto, PageAnalysisContent, PageAnalysisModelInfo,
    PageAnalysisSource, PageAnalysisV1, PageRetrievalFields, ProviderResponseRecord,
    PAGE_ANALYSIS_SCHEMA_VERSION,
};
use crate::domain::page::PageRecordDto;
use crate::domain::pdf_structure::PdfContentBlockDto;
use crate::domain::settings::AppSettingsDto;
use crate::errors::{AppError, AppResult};
use crate::jobs::job_orchestrator::JobOrchestrator;
use crate::providers::model::anthropic_provider::AnthropicProvider;
use crate::providers::model::mimo_provider::MimoProvider;
use crate::providers::model::openai_provider::OpenAIProvider;
use crate::providers::model::prompt_template::{
    page_analysis_prompt, page_analysis_repair_prompt, visual_module_analysis_prompt,
    visual_module_analysis_repair_prompt,
};
use crate::providers::model::provider::{
    ModelAnalysisRequest, ModelAnalysisResponse, ModelProvider,
};
use crate::providers::model::schema_validator::{
    validate_page_analysis_v1, validate_visual_module_analysis_v1, ExpectedPageContext,
    ExpectedVisualModuleContext,
};
use crate::providers::model::siliconflow_provider::SiliconFlowProvider;
use crate::repositories::analysis_repository::AnalysisRepository;
use crate::repositories::db::{block_on_db, database_error};
use crate::repositories::document_repository::DocumentRepository;
use crate::repositories::pdf_structure_repository::PdfStructureRepository;
use crate::services::settings_service::SettingsService;
use crate::services::workspace_service::WorkspaceService;
use chrono::Utc;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::GenericImageView;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::SqliteConnection;
use std::collections::VecDeque;
use std::fs;
use std::io::Cursor;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct AnalysisService;

const MODEL_IMAGE_MAX_SIDE: u32 = 1280;
const MODEL_IMAGE_JPEG_QUALITY: u8 = 75;
const MODEL_IMAGE_REENCODE_MIN_BYTES: usize = 512 * 1024;
const MODEL_IMAGE_MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
const MODEL_IMAGE_MAX_DIMENSION: u32 = 16_384;
const MODEL_IMAGE_MAX_PIXELS: u64 = 40_000_000;
const MODEL_IMAGE_MAX_ALLOC_BYTES: u64 = 256 * 1024 * 1024;

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
        Self::ensure_legacy_page_analysis_allowed(workspace, page_id)?;
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

        let mut conn = workspace
            .get_db_connection()
            .map_err(|err| Self::fail_batch_job(&orchestrator, &job_id, err))?;
        let pages = DocumentRepository::list_pages_needing_analysis(&mut conn)
            .map_err(|err| Self::fail_batch_job(&orchestrator, &job_id, err))?;
        let pages = Self::retain_legacy_pages(&mut conn, pages)
            .map_err(|err| Self::fail_batch_job(&orchestrator, &job_id, err))?;
        let visual_blocks =
            PdfStructureRepository::list_visual_blocks_needing_analysis(&mut conn, None)
                .map_err(|err| Self::fail_batch_job(&orchestrator, &job_id, err))?;
        drop(conn);

        if pages.is_empty() && visual_blocks.is_empty() {
            return Self::complete_empty_batch(&orchestrator, &job_id);
        }
        let (settings, context) = Self::build_analysis_context(workspace)
            .map_err(|err| Self::fail_batch_job(&orchestrator, &job_id, err))?;

        Self::run_batch_pages(
            workspace,
            &layout,
            &orchestrator,
            &job_id,
            pages,
            visual_blocks,
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
        let is_structured =
            PdfStructureRepository::document_has_canonical_pdf(&mut conn, document_id)?;
        let pages = if is_structured {
            Vec::new()
        } else {
            DocumentRepository::list_pages_by_document(&mut conn, document_id)?
        };
        let visual_blocks = if is_structured {
            PdfStructureRepository::list_all_visual_blocks_for_document(&mut conn, document_id)?
        } else {
            Vec::new()
        };
        drop(conn);

        let orchestrator = JobOrchestrator::new(layout.clone());
        let job = orchestrator.create_job("document_reanalysis")?;
        let job_id = job.job_id;
        if pages.is_empty() && visual_blocks.is_empty() {
            return Self::complete_empty_batch(&orchestrator, &job_id);
        }
        let (settings, context) = Self::build_analysis_context(workspace)
            .map_err(|err| Self::fail_batch_job(&orchestrator, &job_id, err))?;

        Self::run_batch_pages(
            workspace,
            &layout,
            &orchestrator,
            &job_id,
            pages,
            visual_blocks,
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
        let is_structured =
            PdfStructureRepository::document_has_canonical_pdf(&mut conn, document_id)?;
        let pages = if is_structured {
            Vec::new()
        } else {
            DocumentRepository::list_failed_pages_by_document(&mut conn, document_id)?
        };
        let visual_blocks = if is_structured {
            PdfStructureRepository::list_failed_visual_blocks_for_document(&mut conn, document_id)?
        } else {
            Vec::new()
        };
        drop(conn);

        let orchestrator = JobOrchestrator::new(layout.clone());
        let job = orchestrator.create_job("document_failed_reanalysis")?;
        let job_id = job.job_id;
        if pages.is_empty() && visual_blocks.is_empty() {
            return Self::complete_empty_batch(&orchestrator, &job_id);
        }
        let (settings, context) = Self::build_analysis_context(workspace)
            .map_err(|err| Self::fail_batch_job(&orchestrator, &job_id, err))?;

        Self::run_batch_pages(
            workspace,
            &layout,
            &orchestrator,
            &job_id,
            pages,
            visual_blocks,
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
        let pending_visual_modules =
            PdfStructureRepository::count_pending_visual_analyses(&mut conn)?;
        let affected = DocumentRepository::recover_analysis_pending_pages(
            &mut conn,
            "interrupted page analysis has been marked failed for retry",
        )?;
        drop(conn);

        if affected == 0 && pending_visual_modules == 0 {
            return Ok(0);
        }

        let error = AppError::new(
            "page_analysis_interrupted",
            "interrupted page analysis has been marked failed for retry",
            "analysis_recovery",
            true,
        );
        let visual_affected = if pending_visual_modules > 0 {
            let error_id = orchestrator.record_error(&error)?;
            let mut conn = workspace.get_db_connection()?;
            PdfStructureRepository::recover_pending_visual_analyses(&mut conn, &error_id)?
        } else {
            0
        };
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

        Ok(affected + visual_affected)
    }

    fn retain_legacy_pages(
        conn: &mut SqliteConnection,
        pages: Vec<PageRecordDto>,
    ) -> AppResult<Vec<PageRecordDto>> {
        let mut legacy_pages = Vec::with_capacity(pages.len());
        for page in pages {
            if !PdfStructureRepository::document_has_canonical_pdf(conn, &page.document_id)? {
                legacy_pages.push(page);
            }
        }
        Ok(legacy_pages)
    }

    fn ensure_legacy_page_analysis_allowed(
        workspace: &WorkspaceService,
        page_id: &str,
    ) -> AppResult<()> {
        let mut conn = workspace.get_db_connection()?;
        let page = DocumentRepository::find_page_by_id(&mut conn, page_id)?
            .ok_or_else(|| AppError::new("page_not_found", "page not found", "analysis", false))?;
        if PdfStructureRepository::document_has_canonical_pdf(&mut conn, &page.document_id)? {
            return Err(AppError::new(
                "structured_pdf_page_analysis_disabled",
                "Structured PDF pages are analyzed by visual module, not as whole-page images.",
                "visual_module_analysis",
                false,
            ));
        }
        Ok(())
    }

    fn complete_empty_batch(
        orchestrator: &JobOrchestrator,
        job_id: &str,
    ) -> AppResult<AnalysisBatchResultDto> {
        orchestrator.update_progress(job_id, 100, Some("no analysis units need processing"))?;
        Ok(AnalysisBatchResultDto {
            job_id: job_id.to_string(),
            total_pages: 0,
            succeeded_pages: 0,
            failed_pages: 0,
            skipped_pages: 0,
            total_visual_modules: 0,
            succeeded_visual_modules: 0,
            failed_visual_modules: 0,
            skipped_visual_modules: 0,
            status: "succeeded".to_string(),
            error: None,
            updated_at: Utc::now().to_rfc3339(),
        })
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
            #[cfg(test)]
            "local_mock" => "local://mock".to_string(),
            "mimo" => MimoProvider::request_endpoint(&settings)?,
            "openai" => OpenAIProvider::request_endpoint(&settings)?,
            "anthropic" => AnthropicProvider::request_endpoint(&settings)?,
            "siliconflow" => SiliconFlowProvider::request_endpoint(&settings)?,
            _ => {
                return Err(AppError::new(
                    "model_provider_unsupported",
                    "模型 provider 不受支持，请选择硅基流动、MiMo、OpenAI 或 Anthropic。",
                    "analysis",
                    true,
                )
                .with_details(format!("provider={provider_name}")));
            }
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
        let (expected_page, image_bytes, image_mime_type) =
            Self::prepare_page_for_analysis(workspace, layout, page_id, force_reanalysis)?;
        let prompt = page_analysis_prompt(
            "中文",
            &expected_page,
            &context.provider_name,
            &context.model_name,
        );

        let request = ModelAnalysisRequest {
            image_bytes,
            image_mime_type,
            prompt,
            model_name: context.model_name.clone(),
            provider: context.provider_name.clone(),
            endpoint: context.endpoint.clone(),
            expected_page: expected_page.clone(),
        };

        #[cfg(test)]
        let default_mock = crate::providers::model::mock_provider::MockModelProvider;
        let default_mimo = MimoProvider;
        let default_openai = OpenAIProvider;
        let default_anthropic = AnthropicProvider;
        let default_siliconflow = SiliconFlowProvider;
        let provider: &dyn ModelProvider = if let Some(provider) = provider_override {
            provider
        } else {
            match context.provider_name.as_str() {
                #[cfg(test)]
                "local_mock" => &default_mock,
                "mimo" => &default_mimo,
                "openai" => &default_openai,
                "anthropic" => &default_anthropic,
                "siliconflow" => &default_siliconflow,
                _ => {
                    return Err(AppError::new(
                        "model_provider_unsupported",
                        "模型 provider 不受支持，请选择硅基流动、MiMo、OpenAI 或 Anthropic。",
                        "analysis",
                        true,
                    )
                    .with_details(format!("provider={}", context.provider_name)));
                }
            }
        };

        let provider_response = provider.analyze_page(&request)?;
        let analysis = Self::normalize_provider_response_with_retry(
            &request,
            provider,
            &provider_response,
            &expected_page,
        )?;
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

    fn analyze_visual_block_core(
        workspace: &WorkspaceService,
        layout: &crate::artifacts::workspace_layout::WorkspaceLayout,
        context: &AnalysisExecutionContext,
        block: &PdfContentBlockDto,
        attempt_count: i64,
        provider_override: Option<&dyn ModelProvider>,
    ) -> AppResult<()> {
        let (expected_page, image_bytes, image_mime_type) =
            Self::prepare_visual_module_input(workspace, layout, block)?;
        let expected_visual = ExpectedVisualModuleContext {
            block_id: block.block_id.clone(),
            provider: context.provider_name.clone(),
            model_name: context.model_name.clone(),
        };
        let prompt = visual_module_analysis_prompt("Chinese", &expected_visual, &block.block_type);
        let request = ModelAnalysisRequest {
            image_bytes,
            image_mime_type,
            prompt,
            model_name: context.model_name.clone(),
            provider: context.provider_name.clone(),
            endpoint: context.endpoint.clone(),
            expected_page,
        };

        #[cfg(test)]
        let default_mock = crate::providers::model::mock_provider::MockModelProvider;
        let default_mimo = MimoProvider;
        let default_openai = OpenAIProvider;
        let default_anthropic = AnthropicProvider;
        let default_siliconflow = SiliconFlowProvider;
        let provider: &dyn ModelProvider = if let Some(provider) = provider_override {
            provider
        } else {
            match context.provider_name.as_str() {
                #[cfg(test)]
                "local_mock" => &default_mock,
                "mimo" => &default_mimo,
                "openai" => &default_openai,
                "anthropic" => &default_anthropic,
                "siliconflow" => &default_siliconflow,
                _ => {
                    return Err(AppError::new(
                        "model_provider_unsupported",
                        "Configured model provider is not supported.",
                        "visual_module_analysis",
                        true,
                    )
                    .with_details(format!("provider={}", context.provider_name)));
                }
            }
        };

        let response = provider.analyze_page(&request)?;
        let enrichment = Self::normalize_visual_response_with_retry(
            &request,
            provider,
            &response,
            &expected_visual,
            &block.block_type,
        )?;
        let enrichment_json = serde_json::to_string(&enrichment).map_err(|err| {
            AppError::new(
                "visual_module_json_serialize_failed",
                "Visual-module enrichment could not be serialized.",
                "visual_module_analysis",
                false,
            )
            .with_details(err.to_string())
        })?;
        let mut conn = workspace.get_db_connection()?;
        PdfStructureRepository::save_visual_success(
            &mut conn,
            &block.block_id,
            attempt_count,
            &context.provider_name,
            &context.model_name,
            &enrichment_json,
        )
    }

    fn normalize_visual_response_with_retry(
        request: &ModelAnalysisRequest,
        provider: &dyn ModelProvider,
        response: &ModelAnalysisResponse,
        expected: &ExpectedVisualModuleContext,
        block_type: &str,
    ) -> AppResult<crate::domain::pdf_structure::VisualModuleAnalysisV1> {
        match validate_visual_module_analysis_v1(&response.raw_json, expected) {
            Ok(analysis) => Ok(analysis),
            Err(first_error) if first_error.stage == "visual_module_validation" => {
                let mut retry_request = request.clone();
                retry_request.prompt = visual_module_analysis_repair_prompt(
                    "Chinese",
                    expected,
                    block_type,
                    &Self::validation_retry_summary(&first_error),
                );
                let retry_response = provider.analyze_page(&retry_request)?;
                validate_visual_module_analysis_v1(&retry_response.raw_json, expected).map_err(
                    |retry_error| {
                        let retry_summary = Self::validation_retry_summary(&retry_error);
                        retry_error.with_details(format!(
                            "first_validation_error={}; retry_validation_error={}",
                            Self::validation_retry_summary(&first_error),
                            retry_summary
                        ))
                    },
                )
            }
            Err(error) => Err(error),
        }
    }

    fn prepare_visual_module_input(
        workspace: &WorkspaceService,
        layout: &crate::artifacts::workspace_layout::WorkspaceLayout,
        block: &PdfContentBlockDto,
    ) -> AppResult<(ExpectedPageContext, Vec<u8>, String)> {
        let input_path = block.source_image_path.as_deref().ok_or_else(|| {
            AppError::new(
                "visual_module_source_image_missing",
                "Visual module has no OpenDataLoader-extracted image.",
                "visual_module_input",
                false,
            )
            .with_details(format!("block_id={}", block.block_id))
        })?;
        let (raw_image, artifact_hash) =
            Self::read_validated_structure_image(workspace, layout, block, input_path)?;
        let (image_bytes, image_mime_type) = Self::optimize_image_for_model(&raw_image)?;

        let mut conn = workspace.get_db_connection()?;
        let page = DocumentRepository::find_page_by_id(&mut conn, &block.page_id)?
            .ok_or_else(|| AppError::new("page_not_found", "page not found", "analysis", false))?;
        if page.document_id != block.document_id || page.page_number != block.page_number {
            return Err(AppError::new(
                "visual_module_page_identity_mismatch",
                "Visual module does not match its page record.",
                "visual_module_analysis",
                false,
            ));
        }
        Ok((
            ExpectedPageContext {
                page_id: page.page_id,
                document_id: page.document_id,
                page_number: page.page_number,
                image_hash: artifact_hash,
                image_path: input_path.to_string(),
            },
            image_bytes,
            image_mime_type,
        ))
    }

    fn read_validated_structure_image(
        workspace: &WorkspaceService,
        layout: &crate::artifacts::workspace_layout::WorkspaceLayout,
        block: &PdfContentBlockDto,
        relative_path: &str,
    ) -> AppResult<(Vec<u8>, String)> {
        let mut conn = workspace.get_db_connection()?;
        let expected_hash = PdfStructureRepository::find_document_artifact_content_hash(
            &mut conn,
            &block.document_id,
            "pdf_structure_image",
            relative_path,
        )?
        .ok_or_else(|| {
            AppError::new(
                "visual_module_source_image_unregistered",
                "Visual-module source image is not a registered PDF artifact.",
                "visual_module_input",
                false,
            )
        })?;
        let path = Self::validated_workspace_file(layout.root(), relative_path)?;
        let metadata = fs::metadata(&path).map_err(|err| {
            AppError::io("visual_module_input", "source_image_metadata_failed", err)
        })?;
        if metadata.len() > MODEL_IMAGE_MAX_INPUT_BYTES as u64 {
            return Err(AppError::new(
                "visual_module_source_image_too_large",
                "Visual-module source image exceeds the model input size limit.",
                "visual_module_input",
                false,
            )
            .with_details(format!(
                "block_id={}; bytes={}; max_bytes={MODEL_IMAGE_MAX_INPUT_BYTES}",
                block.block_id,
                metadata.len()
            )));
        }
        let bytes = fs::read(path)
            .map_err(|err| AppError::io("visual_module_input", "source_image_read_failed", err))?;
        let actual_hash = format!("{:x}", Sha256::digest(&bytes));
        if actual_hash != expected_hash {
            return Err(AppError::new(
                "visual_module_source_image_hash_mismatch",
                "Visual-module source image no longer matches its registered artifact hash.",
                "visual_module_input",
                false,
            )
            .with_details(format!(
                "block_id={}; expected={expected_hash}; actual={actual_hash}",
                block.block_id
            )));
        }
        Self::decode_image_with_limits(
            &bytes,
            "visual_module_source_image_invalid",
            "Visual-module source image could not be decoded safely.",
            "visual_module_input",
        )?;
        Ok((bytes, expected_hash))
    }

    fn validated_workspace_file(workspace_root: &Path, relative_path: &str) -> AppResult<PathBuf> {
        let relative = Path::new(relative_path);
        if relative.as_os_str().is_empty()
            || relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(AppError::new(
                "visual_module_path_invalid",
                "Visual-module input path is not a safe workspace-relative path.",
                "visual_module_input",
                false,
            ));
        }
        let canonical_root = fs::canonicalize(workspace_root)
            .map_err(|err| AppError::io("visual_module_input", "workspace_path_invalid", err))?;
        let candidate = fs::canonicalize(workspace_root.join(relative)).map_err(|err| {
            AppError::io("visual_module_input", "visual_module_file_missing", err)
        })?;
        if !candidate.starts_with(&canonical_root) || !candidate.is_file() {
            return Err(AppError::new(
                "visual_module_path_outside_workspace",
                "Visual-module input path resolves outside the workspace.",
                "visual_module_input",
                false,
            ));
        }
        Ok(candidate)
    }

    fn normalize_provider_response_with_retry(
        request: &ModelAnalysisRequest,
        provider: &dyn ModelProvider,
        response: &ModelAnalysisResponse,
        expected_page: &ExpectedPageContext,
    ) -> AppResult<PageAnalysisV1> {
        match Self::normalize_provider_response(response, expected_page) {
            Ok(analysis) => Ok(analysis),
            Err(err) if Self::should_retry_format(&err) => {
                let mut retry_request = request.clone();
                retry_request.prompt = page_analysis_repair_prompt(
                    "中文",
                    expected_page,
                    &request.provider,
                    &request.model_name,
                    &Self::validation_retry_summary(&err),
                );
                let retry_response = provider.analyze_page(&retry_request)?;
                match Self::normalize_provider_response(&retry_response, expected_page) {
                    Ok(analysis) => Ok(analysis),
                    Err(retry_err) if Self::can_fallback_to_text_analysis(&retry_err) => {
                        let fallback_response = if retry_response.raw_json.trim().is_empty() {
                            response
                        } else {
                            &retry_response
                        };
                        tracing::warn!(
                            target: "analysis",
                            provider = %request.provider,
                            model = %request.model_name,
                            page_id = %expected_page.page_id,
                            "model returned non-json page analysis after repair retry; saving text fallback"
                        );
                        Self::fallback_text_analysis(fallback_response, expected_page)
                    }
                    Err(retry_err) => {
                        let retry_summary = Self::validation_retry_summary(&retry_err);
                        Err(retry_err.with_details(format!(
                            "first_validation_error={}; retry_validation_error={}",
                            Self::validation_retry_summary(&err),
                            retry_summary
                        )))
                    }
                }
            }
            Err(err) => Err(err),
        }
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
            Err(err) => Err(err),
        }
    }

    fn should_retry_format(error: &AppError) -> bool {
        matches!(
            error.code.as_str(),
            "analysis_json_invalid"
                | "analysis_field_missing"
                | "analysis_schema_version_unsupported"
                | "analysis_field_invalid"
                | "analysis_retrieval_text_missing"
        )
    }

    fn can_fallback_to_text_analysis(error: &AppError) -> bool {
        error.code == "analysis_json_invalid"
            && error
                .details
                .as_deref()
                .is_some_and(|details| details.contains("no complete JSON object found"))
    }

    fn fallback_text_analysis(
        response: &ModelAnalysisResponse,
        expected_page: &ExpectedPageContext,
    ) -> AppResult<PageAnalysisV1> {
        let content = response.raw_json.trim();
        let visible_text = if content.is_empty() {
            "模型未返回结构化 JSON，原始响应已保留。".to_string()
        } else {
            Self::truncate_chars(content, 50_000)
        };
        let bm25_text = if visible_text.trim().is_empty() {
            format!("第 {} 页 页面图片", expected_page.page_number)
        } else {
            visible_text.clone()
        };
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
                title: Some(format!("第 {} 页分析", expected_page.page_number)),
                summary: Some(Self::summarize_model_content(&visible_text)),
                visible_text: Some(visible_text),
                topics: vec!["页面分析".to_string()],
                keywords: vec![],
            },
            retrieval: PageRetrievalFields { bm25_text },
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

    fn validation_retry_summary(error: &AppError) -> String {
        match &error.details {
            Some(details) => format!(
                "code={}; details={}",
                error.code,
                Self::truncate_chars(details, 500)
            ),
            None => format!("code={}", error.code),
        }
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
    ) -> AppResult<(ExpectedPageContext, Vec<u8>, String)> {
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
            let image_hash = page.image_hash.clone().ok_or_else(|| {
                AppError::new(
                    "structured_pdf_page_analysis_disabled",
                    "Structured PDF pages have no whole-page image; analyze their visual modules instead.",
                    "visual_module_analysis",
                    false,
                )
            })?;
            let image_asset = DocumentRepository::find_image_asset_by_hash(&mut conn, &image_hash)?
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
                    image_hash,
                    image_path: image_asset.file_path.clone(),
                },
                layout.root().join(&image_asset.file_path),
                image_asset.file_path,
            )
        };

        let original_image_bytes = fs::read(&image_path).map_err(|err| {
            AppError::io("analysis", "page_image_read_failed", err).with_details(format!(
                "page_id={page_id}; image_path={relative_image_path}"
            ))
        })?;
        let (image_bytes, image_mime_type) = Self::optimize_image_for_model(&original_image_bytes)?;

        Ok((expected_page, image_bytes, image_mime_type))
    }

    fn decode_image_with_limits(
        image_bytes: &[u8],
        code: &str,
        message: &str,
        stage: &str,
    ) -> AppResult<image::DynamicImage> {
        if image_bytes.len() > MODEL_IMAGE_MAX_INPUT_BYTES {
            return Err(
                AppError::new(code, message, stage, false).with_details(format!(
                    "bytes={}; max_bytes={MODEL_IMAGE_MAX_INPUT_BYTES}",
                    image_bytes.len()
                )),
            );
        }
        let dimensions_reader = image::ImageReader::new(Cursor::new(image_bytes))
            .with_guessed_format()
            .map_err(|err| {
                AppError::new(code, message, stage, false).with_details(err.to_string())
            })?;
        let (width, height) = dimensions_reader.into_dimensions().map_err(|err| {
            AppError::new(code, message, stage, false).with_details(err.to_string())
        })?;
        let pixels = u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or_else(|| AppError::new(code, message, stage, false))?;
        if width > MODEL_IMAGE_MAX_DIMENSION
            || height > MODEL_IMAGE_MAX_DIMENSION
            || pixels > MODEL_IMAGE_MAX_PIXELS
        {
            return Err(AppError::new(code, message, stage, false).with_details(format!(
                "width={width}; height={height}; pixels={pixels}; max_dimension={MODEL_IMAGE_MAX_DIMENSION}; max_pixels={MODEL_IMAGE_MAX_PIXELS}"
            )));
        }

        let mut reader = image::ImageReader::new(Cursor::new(image_bytes))
            .with_guessed_format()
            .map_err(|err| {
                AppError::new(code, message, stage, false).with_details(err.to_string())
            })?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(MODEL_IMAGE_MAX_DIMENSION);
        limits.max_image_height = Some(MODEL_IMAGE_MAX_DIMENSION);
        limits.max_alloc = Some(MODEL_IMAGE_MAX_ALLOC_BYTES);
        reader.limits(limits);
        reader
            .decode()
            .map_err(|err| AppError::new(code, message, stage, false).with_details(err.to_string()))
    }

    fn optimize_image_for_model(image_bytes: &[u8]) -> AppResult<(Vec<u8>, String)> {
        let image = Self::decode_image_with_limits(
            image_bytes,
            "model_image_decode_failed",
            "Page image could not be decoded safely for model optimization.",
            "analysis",
        )?;
        let (width, height) = image.dimensions();
        let should_resize = width.max(height) > MODEL_IMAGE_MAX_SIDE;
        let should_reencode = should_resize || image_bytes.len() > MODEL_IMAGE_REENCODE_MIN_BYTES;
        if !should_reencode {
            let mime_type = image::guess_format(image_bytes)
                .map_err(|err| {
                    AppError::new(
                        "model_image_format_unknown",
                        "Page image format could not be detected for model input.",
                        "analysis",
                        false,
                    )
                    .with_details(err.to_string())
                })?
                .to_mime_type()
                .to_string();
            return Ok((image_bytes.to_vec(), mime_type));
        }

        let optimized = if should_resize {
            image.resize(
                MODEL_IMAGE_MAX_SIDE,
                MODEL_IMAGE_MAX_SIDE,
                FilterType::Triangle,
            )
        } else {
            image
        };

        let rgb = optimized.to_rgb8();
        let mut jpeg_bytes = Vec::new();
        {
            let mut cursor = Cursor::new(&mut jpeg_bytes);
            let mut encoder = JpegEncoder::new_with_quality(&mut cursor, MODEL_IMAGE_JPEG_QUALITY);
            encoder.encode_image(&rgb).map_err(|err| {
                AppError::new(
                    "model_image_encode_failed",
                    "page image could not be encoded for model input",
                    "analysis",
                    true,
                )
                .with_details(err.to_string())
            })?;
        }

        Ok((jpeg_bytes, "image/jpeg".to_string()))
    }

    fn run_batch_pages(
        workspace: &WorkspaceService,
        layout: &crate::artifacts::workspace_layout::WorkspaceLayout,
        orchestrator: &JobOrchestrator,
        job_id: &str,
        pages: Vec<PageRecordDto>,
        visual_blocks: Vec<PdfContentBlockDto>,
        force_reanalysis: bool,
        analysis_concurrency: u8,
        context: AnalysisExecutionContext,
        provider_override: Option<&dyn ModelProvider>,
    ) -> AppResult<AnalysisBatchResultDto> {
        let total_pages = pages.len() as i64;
        let total_visual_modules = visual_blocks.len() as i64;
        let total_units = total_pages + total_visual_modules;
        let mut items = Vec::with_capacity(pages.len() + visual_blocks.len());
        items.extend(pages.into_iter().map(BatchAnalysisItem::Page));
        items.extend(
            visual_blocks
                .into_iter()
                .map(BatchAnalysisItem::VisualModule),
        );
        if total_units == 0 {
            orchestrator.update_progress(job_id, 100, Some("no analysis units need processing"))?;
            return Ok(AnalysisBatchResultDto {
                job_id: job_id.to_string(),
                total_pages,
                succeeded_pages: 0,
                failed_pages: 0,
                skipped_pages: 0,
                total_visual_modules,
                succeeded_visual_modules: 0,
                failed_visual_modules: 0,
                skipped_visual_modules: 0,
                status: "succeeded".to_string(),
                error: None,
                updated_at: Utc::now().to_rfc3339(),
            });
        }

        orchestrator.update_progress(
            job_id,
            1,
            Some(&Self::batch_progress_message(
                "batch analysis started",
                total_units,
                0,
                0,
                0,
                None,
            )),
        )?;

        let worker_count = usize::from(analysis_concurrency.clamp(1, 8)).min(items.len());
        let queue = Arc::new(Mutex::new(VecDeque::from(
            items.into_iter().enumerate().collect::<Vec<_>>(),
        )));
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
                        let item = {
                            let mut queue = queue.lock().expect("analysis queue lock");
                            queue.pop_front()
                        };
                        let Some((item_index, item)) = item else {
                            break;
                        };
                        let item_id = item.id().to_string();
                        let item_kind = item.kind();
                        let outcome = match item {
                            BatchAnalysisItem::Page(page) => Self::run_batch_page(
                                &workspace,
                                &layout,
                                &orchestrator,
                                &context,
                                &page.page_id,
                                force_reanalysis,
                                provider_override,
                            ),
                            BatchAnalysisItem::VisualModule(block) => {
                                Self::run_batch_visual_module(
                                    &workspace,
                                    &layout,
                                    &context,
                                    &block,
                                    provider_override,
                                )
                            }
                        };

                        let progress = {
                            let mut counters = counters.lock().expect("analysis counters lock");
                            counters.record(item_index, item_kind, outcome);
                            counters.last_unit_id = Some(item_id);
                            ((counters.completed_units * 98 / total_units) + 1)
                                .min(99)
                                .max(1) as u8
                        };

                        let counters_snapshot =
                            counters.lock().expect("analysis counters lock").clone();
                        let message = Self::batch_progress_message(
                            "batch analysis running",
                            total_units,
                            counters_snapshot.succeeded_units(),
                            counters_snapshot.failed_units(),
                            counters_snapshot.skipped_units(),
                            counters_snapshot.last_unit_id.as_deref(),
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
        let final_status = if counters.failed_units() == 0 {
            "succeeded"
        } else if counters.succeeded_units() == 0 {
            "failed"
        } else {
            "succeeded_with_failures"
        };
        let message = Self::batch_progress_message(
            "批量分析完成",
            total_units,
            counters.succeeded_units(),
            counters.failed_units(),
            counters.skipped_units(),
            counters.last_unit_id.as_deref(),
        );

        if final_status == "failed" || final_status == "succeeded_with_failures" {
            let err = AppError::new(
                if final_status == "failed" {
                    "analysis_batch_failed"
                } else {
                    "analysis_batch_succeeded_with_failures"
                },
                if final_status == "failed" {
                    "batch analysis failed; no processed analysis units succeeded"
                } else {
                    "batch analysis completed with failed analysis units"
                },
                "analysis",
                true,
            );
            let _ = orchestrator.mark_failed(job_id, &err, &message)?;
        } else {
            orchestrator.update_progress(job_id, 100, Some(&message))?;
        }

        if counters.pages.succeeded > 0 {
            Self::refresh_page_jsonl_artifact(workspace);
        }

        Ok(AnalysisBatchResultDto {
            job_id: job_id.to_string(),
            total_pages,
            succeeded_pages: counters.pages.succeeded,
            failed_pages: counters.pages.failed,
            skipped_pages: counters.pages.skipped,
            total_visual_modules,
            succeeded_visual_modules: counters.visual_modules.succeeded,
            failed_visual_modules: counters.visual_modules.failed,
            skipped_visual_modules: counters.visual_modules.skipped,
            status: final_status.to_string(),
            error: counters.first_error.map(|(_, error)| error),
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
                BatchPageOutcome::Failed(err)
            }
        }
    }

    fn run_batch_visual_module(
        workspace: &WorkspaceService,
        layout: &crate::artifacts::workspace_layout::WorkspaceLayout,
        context: &AnalysisExecutionContext,
        block: &PdfContentBlockDto,
        provider_override: Option<&dyn ModelProvider>,
    ) -> BatchPageOutcome {
        let attempt_count = match Self::begin_visual_module_analysis(workspace, context, block) {
            Ok(attempt_count) => attempt_count,
            Err(error) if error.code == "visual_module_analysis_already_running" => {
                return BatchPageOutcome::Skipped;
            }
            Err(error) => return BatchPageOutcome::Failed(error),
        };
        match Self::analyze_visual_block_core(
            workspace,
            layout,
            context,
            block,
            attempt_count,
            provider_override,
        ) {
            Ok(()) => BatchPageOutcome::Succeeded,
            Err(error) => {
                if let Err(persist_error) =
                    Self::record_visual_module_failure(workspace, block, attempt_count, &error)
                {
                    tracing::error!(
                        target: "analysis",
                        block_id = %block.block_id,
                        code = %persist_error.code,
                        correlation_id = %persist_error.correlation_id,
                        "visual-module failure could not be persisted"
                    );
                }
                BatchPageOutcome::Failed(error)
            }
        }
    }

    fn begin_visual_module_analysis(
        workspace: &WorkspaceService,
        context: &AnalysisExecutionContext,
        block: &PdfContentBlockDto,
    ) -> AppResult<i64> {
        if !block.is_visual || block.is_decorative {
            return Err(AppError::new(
                "visual_module_not_eligible",
                "Only non-decorative visual PDF blocks can be analyzed.",
                "visual_module_analysis",
                false,
            ));
        }
        let mut conn = workspace.get_db_connection()?;
        PdfStructureRepository::try_mark_visual_pending(
            &mut conn,
            &block.block_id,
            &context.provider_name,
            &context.model_name,
        )?
        .ok_or_else(|| {
            AppError::new(
                "visual_module_analysis_already_running",
                "Visual-module analysis is already running.",
                "visual_module_analysis",
                true,
            )
        })
    }

    fn record_visual_module_failure(
        workspace: &WorkspaceService,
        block: &PdfContentBlockDto,
        attempt_count: i64,
        error: &AppError,
    ) -> AppResult<()> {
        let mut conn = workspace.get_db_connection()?;
        PdfStructureRepository::save_visual_failure(
            &mut conn,
            &block.block_id,
            attempt_count,
            error,
        )
    }

    fn batch_progress_message(
        phase: &str,
        total_units: i64,
        succeeded_units: i64,
        failed_units: i64,
        skipped_units: i64,
        last_unit_id: Option<&str>,
    ) -> String {
        let last_unit = last_unit_id.unwrap_or("-");
        format!(
            "{phase}: total_units={total_units}; succeeded_units={succeeded_units}; failed_units={failed_units}; skipped_units={skipped_units}; last_unit={last_unit}; updated_at={}",
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

    fn truncate_chars(value: &str, max_chars: usize) -> String {
        value.chars().take(max_chars).collect()
    }

    fn summarize_model_content(content: &str) -> String {
        let first_line = content
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or(content.trim());
        Self::truncate_chars(first_line, 240)
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

enum BatchAnalysisItem {
    Page(PageRecordDto),
    VisualModule(PdfContentBlockDto),
}

impl BatchAnalysisItem {
    fn id(&self) -> &str {
        match self {
            Self::Page(page) => &page.page_id,
            Self::VisualModule(block) => &block.block_id,
        }
    }

    fn kind(&self) -> BatchAnalysisItemKind {
        match self {
            Self::Page(_) => BatchAnalysisItemKind::Page,
            Self::VisualModule(_) => BatchAnalysisItemKind::VisualModule,
        }
    }
}

#[derive(Clone, Copy)]
enum BatchAnalysisItemKind {
    Page,
    VisualModule,
}

#[derive(Default, Clone)]
struct BatchCounters {
    completed_units: i64,
    pages: BatchOutcomeCounters,
    visual_modules: BatchOutcomeCounters,
    last_unit_id: Option<String>,
    first_error: Option<(usize, AppError)>,
}

impl BatchCounters {
    fn record(
        &mut self,
        item_index: usize,
        kind: BatchAnalysisItemKind,
        outcome: BatchPageOutcome,
    ) {
        self.completed_units += 1;
        let counters = match kind {
            BatchAnalysisItemKind::Page => &mut self.pages,
            BatchAnalysisItemKind::VisualModule => &mut self.visual_modules,
        };
        match outcome {
            BatchPageOutcome::Succeeded => counters.succeeded += 1,
            BatchPageOutcome::Failed(error) => {
                counters.failed += 1;
                let should_replace = match self.first_error.as_ref() {
                    Some((first_index, _)) => item_index < *first_index,
                    None => true,
                };
                if should_replace {
                    self.first_error = Some((item_index, error));
                }
            }
            BatchPageOutcome::Skipped => counters.skipped += 1,
        }
    }

    fn succeeded_units(&self) -> i64 {
        self.pages.succeeded + self.visual_modules.succeeded
    }

    fn failed_units(&self) -> i64 {
        self.pages.failed + self.visual_modules.failed
    }

    fn skipped_units(&self) -> i64 {
        self.pages.skipped + self.visual_modules.skipped
    }
}

#[derive(Default, Clone)]
struct BatchOutcomeCounters {
    succeeded: i64,
    failed: i64,
    skipped: i64,
}

enum BatchPageOutcome {
    Succeeded,
    Failed(AppError),
    Skipped,
}

#[cfg(test)]
mod tests {
    use super::{
        AnalysisService, BatchAnalysisItemKind, BatchCounters, BatchPageOutcome,
        MODEL_IMAGE_MAX_SIDE, MODEL_IMAGE_REENCODE_MIN_BYTES,
    };
    use crate::api::state::ApiAppState;
    use crate::domain::analysis::PAGE_ANALYSIS_SCHEMA_VERSION;
    use crate::domain::pdf_structure::{
        DocumentArtifactInput, NormalizedBbox, PdfContentBlockDto, PdfParseRun,
        VISUAL_MODULE_ANALYSIS_SCHEMA_VERSION,
    };
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
    use crate::repositories::pdf_structure_repository::PdfStructureRepository;
    use crate::repositories::workspace_settings_repository::WorkspaceSettingsRepository;
    use crate::services::api_server_service::ApiServerService;
    use crate::services::workspace_service::WorkspaceService;
    use image::GenericImageView;
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::sync::{Arc, Mutex};

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

    #[test]
    fn model_image_optimization_downscales_and_reencodes_to_jpeg() {
        let image = image::RgbImage::from_pixel(2400, 1600, image::Rgb([255, 255, 255]));
        let mut png_bytes = Vec::new();
        image::DynamicImage::ImageRgb8(image)
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            )
            .expect("encode png");

        let (optimized, mime_type) =
            AnalysisService::optimize_image_for_model(&png_bytes).expect("optimize image");
        let decoded = image::load_from_memory(&optimized).expect("decode optimized image");

        assert_eq!(mime_type, "image/jpeg");
        assert_eq!(
            decoded.dimensions().0.max(decoded.dimensions().1),
            MODEL_IMAGE_MAX_SIDE
        );
    }

    #[test]
    fn model_image_optimization_keeps_small_png_without_reencoding() {
        let image = image::RgbImage::from_pixel(320, 240, image::Rgb([255, 255, 255]));
        let mut png_bytes = Vec::new();
        image::DynamicImage::ImageRgb8(image)
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            )
            .expect("encode png");
        assert!(png_bytes.len() < MODEL_IMAGE_REENCODE_MIN_BYTES);

        let (optimized, mime_type) =
            AnalysisService::optimize_image_for_model(&png_bytes).expect("optimize image");

        assert_eq!(mime_type, "image/png");
        assert_eq!(optimized, png_bytes);
    }

    #[test]
    fn model_image_optimization_preserves_small_jpeg_mime_type() {
        let image = image::RgbImage::from_pixel(320, 240, image::Rgb([255, 255, 255]));
        let mut jpeg_bytes = Vec::new();
        image::DynamicImage::ImageRgb8(image)
            .write_to(
                &mut std::io::Cursor::new(&mut jpeg_bytes),
                image::ImageFormat::Jpeg,
            )
            .expect("encode jpeg");
        assert!(jpeg_bytes.len() < MODEL_IMAGE_REENCODE_MIN_BYTES);

        let (optimized, mime_type) =
            AnalysisService::optimize_image_for_model(&jpeg_bytes).expect("optimize image");

        assert_eq!(mime_type, "image/jpeg");
        assert_eq!(optimized, jpeg_bytes);
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
        let image_bytes = png_bytes(16, 16, [240, 240, 240]);
        if write_image {
            let layout = service.current_layout().expect("layout");
            let absolute = layout.root().join(&image_path);
            fs::create_dir_all(absolute.parent().expect("parent")).expect("page dir");
            fs::write(&absolute, &image_bytes).expect("image");
        }
        DocumentRepository::create_image_asset(
            &mut conn,
            image_hash,
            &image_path,
            image_bytes.len() as i64,
        )
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

    #[derive(Clone)]
    struct VisualBlockSpec {
        block_id: &'static str,
        source_text: &'static str,
        source_image_path: Option<String>,
        source_image: Option<(u32, u32, [u8; 3])>,
        bbox: Option<NormalizedBbox>,
        is_visual: bool,
        is_decorative: bool,
    }

    fn png_bytes(width: u32, height: u32, color: [u8; 3]) -> Vec<u8> {
        let image = image::RgbImage::from_pixel(width, height, image::Rgb(color));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgb8(image)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .expect("encode png");
        bytes
    }

    fn seed_structured_document(
        service: &WorkspaceService,
        specs: Vec<VisualBlockSpec>,
    ) -> (String, String) {
        let mut conn = service.get_db_connection().expect("connection");
        let document = DocumentRepository::create_document(
            &mut conn,
            "structured.pdf",
            "pdf",
            "structured-file-hash",
            "originals/structured.pdf",
            None,
        )
        .expect("document");
        let document_id = document.document_id;
        let page_id = DocumentRepository::create_structured_page_record(&mut conn, &document_id, 1)
            .expect("structured page")
            .page_id;
        DocumentRepository::update_document_status(&mut conn, &document_id, "ready", Some(1), None)
            .expect("document ready");
        let layout = service.current_layout().expect("layout");

        let canonical_relative = format!("pdfs/{document_id}/canonical.pdf");
        let canonical_path = layout.root().join(&canonical_relative);
        fs::create_dir_all(canonical_path.parent().expect("pdf parent")).expect("pdf dir");
        fs::write(&canonical_path, b"%PDF-1.7 test").expect("canonical pdf");

        let parse_id = format!("parse-{document_id}");
        let raw_relative = format!("structure/{document_id}/{parse_id}/document.json");
        let raw_path = layout.root().join(&raw_relative);
        fs::create_dir_all(raw_path.parent().expect("structure parent")).expect("structure dir");
        fs::write(&raw_path, b"[]").expect("structure json");

        PdfStructureRepository::upsert_canonical_pdf(
            &mut conn,
            &DocumentArtifactInput {
                artifact_id: format!("canonical-{document_id}"),
                document_id: document_id.clone(),
                kind: "canonical_pdf".to_string(),
                relative_path: canonical_relative,
                content_hash: "canonical-hash".to_string(),
                parser_name: None,
                parser_version: None,
                parser_options_json: None,
            },
        )
        .expect("canonical artifact");

        let mut artifacts = vec![DocumentArtifactInput {
            artifact_id: format!("structure-json-{document_id}"),
            document_id: document_id.clone(),
            kind: "pdf_structure_json".to_string(),
            relative_path: raw_relative.clone(),
            content_hash: "json-hash".to_string(),
            parser_name: Some("opendataloader-pdf".to_string()),
            parser_version: Some("2.5.0".to_string()),
            parser_options_json: Some("{}".to_string()),
        }];
        let mut blocks = Vec::with_capacity(specs.len());
        for (ordinal, spec) in specs.into_iter().enumerate() {
            if let (Some(relative_path), Some((width, height, color))) =
                (spec.source_image_path.as_deref(), spec.source_image)
            {
                let source_path = layout.root().join(relative_path);
                fs::create_dir_all(source_path.parent().expect("source parent"))
                    .expect("source dir");
                let source_bytes = png_bytes(width, height, color);
                fs::write(&source_path, &source_bytes).expect("source image");
                artifacts.push(DocumentArtifactInput {
                    artifact_id: format!("structure-image-{ordinal}-{document_id}"),
                    document_id: document_id.clone(),
                    kind: "pdf_structure_image".to_string(),
                    relative_path: relative_path.to_string(),
                    content_hash: format!("{:x}", Sha256::digest(&source_bytes)),
                    parser_name: Some("opendataloader-pdf".to_string()),
                    parser_version: Some("2.5.0".to_string()),
                    parser_options_json: Some("{}".to_string()),
                });
            }
            blocks.push(PdfContentBlockDto {
                block_id: spec.block_id.to_string(),
                parse_id: parse_id.clone(),
                document_id: document_id.clone(),
                page_id: page_id.clone(),
                page_number: 1,
                parent_block_id: None,
                source_element_id: Some(ordinal.to_string()),
                ordinal: ordinal as i64,
                block_type: if spec.is_visual { "image" } else { "paragraph" }.to_string(),
                source_text: spec.source_text.to_string(),
                enrichment_json: None,
                raw_json: format!("{{\"id\":{ordinal}}}"),
                source_image_path: spec.source_image_path,
                is_indexable: true,
                is_visual: spec.is_visual,
                is_decorative: spec.is_decorative,
                bbox: spec.bbox,
            });
        }
        PdfStructureRepository::replace_document_structure(
            &mut conn,
            &PdfParseRun {
                parse_id,
                document_id: document_id.clone(),
                parser_name: "opendataloader-pdf".to_string(),
                parser_version: "2.5.0".to_string(),
                schema_version: "opendataloader_pdf_json_v2".to_string(),
                parser_options_json: "{}".to_string(),
                raw_json_path: raw_relative,
            },
            &artifacts,
            &blocks,
        )
        .expect("structured document");
        (document_id, page_id)
    }

    fn analysis_context(provider_name: &str, model_name: &str) -> super::AnalysisExecutionContext {
        super::AnalysisExecutionContext {
            provider_name: provider_name.to_string(),
            model_name: model_name.to_string(),
            endpoint: format!("test://{provider_name}"),
        }
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
    fn invalid_model_format_is_retried_with_repair_prompt() {
        let (service, root) = test_workspace();
        let page_id = seed_page(&service, true);
        let provider = FormatRetryProvider::new();
        let layout = service.current_layout().expect("layout");
        let context = analysis_context("siliconflow", "zai-org/GLM-4.6V");

        let result = AnalysisService::analyze_page_core(
            &service,
            &layout,
            &context,
            &page_id,
            true,
            true,
            Some(&provider),
        )
        .expect("retried analysis");

        assert_eq!(result.status, "succeeded");
        let result_json = result.result_json.expect("result json");
        let parsed: serde_json::Value = serde_json::from_str(&result_json).expect("analysis json");
        assert_eq!(parsed["schema_version"], PAGE_ANALYSIS_SCHEMA_VERSION);
        assert_eq!(parsed["page_id"], page_id);
        assert_eq!(parsed["analysis"]["summary"], "修正后的中文摘要");
        assert_eq!(parsed["provider_response"]["endpoint_kind"], "siliconflow");
        let raw_response = parsed["provider_response"]["raw_json"]
            .as_str()
            .expect("raw provider json");
        assert!(raw_response.contains("retried"));
        assert!(!raw_response.contains("data:image/png;base64"));
        assert_eq!(
            provider
                .call_count
                .load(std::sync::atomic::Ordering::SeqCst),
            2
        );
        assert!(provider
            .last_prompt
            .lock()
            .expect("prompt")
            .as_deref()
            .unwrap_or_default()
            .contains("上一次输出未通过"));

        let mut conn = service.get_db_connection().expect("connection");
        let page = DocumentRepository::find_page_by_id(&mut conn, &page_id)
            .expect("page lookup")
            .expect("page");
        assert_eq!(page.status, "analyzed");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn non_json_after_repair_retry_is_saved_as_text_fallback() {
        let (service, root) = test_workspace();
        let page_id = seed_page(&service, true);
        let provider = AlwaysPlainTextProvider::new();
        let layout = service.current_layout().expect("layout");
        let context = analysis_context("mimo", "mimo-v2.5");

        let result = AnalysisService::analyze_page_core(
            &service,
            &layout,
            &context,
            &page_id,
            true,
            true,
            Some(&provider),
        )
        .expect("fallback analysis");

        assert_eq!(result.status, "succeeded");
        let result_json = result.result_json.expect("result json");
        let parsed: serde_json::Value = serde_json::from_str(&result_json).expect("analysis json");
        assert_eq!(parsed["schema_version"], PAGE_ANALYSIS_SCHEMA_VERSION);
        assert_eq!(parsed["page_id"], page_id);
        assert_eq!(parsed["analysis"]["title"], "第 1 页分析");
        assert_eq!(parsed["analysis"]["topics"][0], "页面分析");
        assert!(parsed["analysis"]["visible_text"]
            .as_str()
            .expect("visible text")
            .contains("第二次仍然只返回自然语言描述"));
        assert!(parsed["retrieval"]["bm25_text"]
            .as_str()
            .expect("bm25")
            .contains("第二次仍然只返回自然语言描述"));
        assert_eq!(parsed["model"]["provider"], "mimo");
        assert_eq!(parsed["provider_response"]["endpoint_kind"], "mimo");
        assert_eq!(
            provider
                .call_count
                .load(std::sync::atomic::Ordering::SeqCst),
            2
        );

        let mut conn = service.get_db_connection().expect("connection");
        let page = DocumentRepository::find_page_by_id(&mut conn, &page_id)
            .expect("lookup")
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
    fn batch_counters_choose_the_first_input_error_not_the_first_completed_error() {
        let mut counters = BatchCounters::default();
        counters.record(
            1,
            BatchAnalysisItemKind::Page,
            BatchPageOutcome::Failed(AppError::new(
                "second_input_failed_first",
                "second input failed first",
                "analysis",
                true,
            )),
        );
        counters.record(
            0,
            BatchAnalysisItemKind::Page,
            BatchPageOutcome::Failed(AppError::new(
                "first_input_failed_second",
                "first input failed second",
                "analysis",
                true,
            )),
        );

        assert_eq!(
            counters
                .first_error
                .as_ref()
                .map(|(_, error)| error.code.as_str()),
            Some("first_input_failed_second")
        );
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
        assert_eq!(
            result.error.as_ref().map(|error| error.code.as_str()),
            Some("page_image_read_failed")
        );
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
            .contains("failed_units=1"));
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

    #[test]
    fn structured_text_only_pdf_requires_no_model_configuration_or_calls() {
        let (service, root) = test_workspace();
        let (document_id, _) = seed_structured_document(
            &service,
            vec![VisualBlockSpec {
                block_id: "text-block",
                source_text: "Searchable structured text",
                source_image_path: None,
                source_image: None,
                bbox: Some(NormalizedBbox {
                    x: 0.1,
                    y: 0.1,
                    width: 0.8,
                    height: 0.2,
                }),
                is_visual: false,
                is_decorative: false,
            }],
        );

        let result = AnalysisService::analyze_new_pages(&service)
            .expect("text-only structured PDF should not require a model");
        assert_eq!(result.total_pages, 0);
        assert_eq!(result.status, "succeeded");
        assert!(result.error.is_none());

        let mut conn = service.get_db_connection().expect("connection");
        let pages = AnalysisRepository::list_workbench_pages(&mut conn, &document_id)
            .expect("workbench pages");
        assert_eq!(pages[0].visual_module_count, Some(0));
        assert_eq!(pages[0].pending_visual_module_count, Some(0));
        let page_analysis_count = block_on_db(async {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM analysis_results")
                .fetch_one(&mut conn)
                .await
                .map_err(|err| {
                    crate::repositories::db::database_error("test", "analysis_count", err)
                })
        })
        .expect("analysis count");
        assert_eq!(page_analysis_count, 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn structured_visuals_use_only_registered_odl_images_without_page_fallback() {
        let (service, root) = test_workspace();
        configure_mock_with_concurrency(&service, 1);
        let source_path = "structure/source/images/odl.png".to_string();
        let (document_id, page_id) = seed_structured_document(
            &service,
            vec![
                VisualBlockSpec {
                    block_id: "block-source",
                    source_text: "ODL caption",
                    source_image_path: Some(source_path),
                    source_image: Some((40, 20, [220, 20, 20])),
                    bbox: Some(NormalizedBbox {
                        x: 0.0,
                        y: 0.0,
                        width: 1.0,
                        height: 1.0,
                    }),
                    is_visual: true,
                    is_decorative: false,
                },
                VisualBlockSpec {
                    block_id: "block-missing",
                    source_text: "Missing image caption",
                    source_image_path: None,
                    source_image: None,
                    bbox: Some(NormalizedBbox {
                        x: 0.25,
                        y: 0.25,
                        width: 0.5,
                        height: 0.5,
                    }),
                    is_visual: true,
                    is_decorative: false,
                },
            ],
        );
        let provider = VisualModuleProvider::new(None);

        let result = AnalysisService::analyze_new_pages_with_provider(&service, Some(&provider))
            .expect("visual analysis");
        assert_eq!(result.total_pages, 0);
        assert_eq!(result.succeeded_pages, 0);
        assert_eq!(result.total_visual_modules, 2);
        assert_eq!(result.succeeded_visual_modules, 1);
        assert_eq!(result.failed_visual_modules, 1);
        assert_eq!(
            result.error.as_ref().map(|error| error.code.as_str()),
            Some("visual_module_source_image_missing")
        );
        assert_eq!(provider.call_count(), 1);
        assert_eq!(
            *provider.dimensions.lock().expect("dimensions"),
            vec![("block-source".to_string(), 40, 20)]
        );

        let mut conn = service.get_db_connection().expect("connection");
        assert!(
            AnalysisRepository::find_current_by_page_id(&mut conn, &page_id)
                .expect("page analysis lookup")
                .is_none()
        );
        let source = PdfStructureRepository::find_block_by_id(&mut conn, "block-source")
            .expect("block lookup")
            .expect("source block");
        assert_eq!(source.source_text, "ODL caption");
        assert_eq!(source.bbox.expect("bbox").width, 1.0);
        assert!(source
            .enrichment_json
            .as_deref()
            .expect("enrichment")
            .contains(VISUAL_MODULE_ANALYSIS_SCHEMA_VERSION));
        let pages = AnalysisRepository::list_workbench_pages(&mut conn, &document_id)
            .expect("workbench pages");
        assert!(pages[0].image_hash.is_none());
        assert!(pages[0].image_path.is_none());
        assert_eq!(pages[0].visual_module_count, Some(2));
        assert_eq!(pages[0].succeeded_visual_module_count, Some(1));
        assert_eq!(pages[0].failed_visual_module_count, Some(1));
        let missing = PdfStructureRepository::find_block_by_id(&mut conn, "block-missing")
            .expect("missing block lookup")
            .expect("missing block");
        assert_eq!(missing.source_text, "Missing image caption");
        assert!(missing.enrichment_json.is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn tampered_odl_image_is_rejected_before_the_model_call() {
        let (service, root) = test_workspace();
        configure_mock_with_concurrency(&service, 1);
        let source_path = "structure/tampered/images/source.png".to_string();
        seed_structured_document(
            &service,
            vec![VisualBlockSpec {
                block_id: "block-tampered",
                source_text: "registered source",
                source_image_path: Some(source_path.clone()),
                source_image: Some((20, 20, [20, 40, 60])),
                bbox: None,
                is_visual: true,
                is_decorative: false,
            }],
        );
        let layout = service.current_layout().expect("layout");
        fs::write(
            layout.root().join(source_path),
            png_bytes(20, 20, [60, 40, 20]),
        )
        .expect("tamper source image");
        let provider = VisualModuleProvider::new(None);

        let result = AnalysisService::analyze_new_pages_with_provider(&service, Some(&provider))
            .expect("tampered image becomes an independent block failure");

        assert_eq!(result.failed_visual_modules, 1);
        assert_eq!(result.status, "failed");
        assert_eq!(
            result.error.as_ref().map(|error| error.code.as_str()),
            Some("visual_module_source_image_hash_mismatch")
        );
        assert_eq!(provider.call_count(), 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failed_reanalysis_preserves_the_last_successful_enrichment() {
        let (service, root) = test_workspace();
        configure_mock_with_concurrency(&service, 1);
        let (document_id, _) = seed_structured_document(
            &service,
            vec![VisualBlockSpec {
                block_id: "block-preserved",
                source_text: "preserve this source",
                source_image_path: Some("structure/preserved/images/source.png".to_string()),
                source_image: Some((20, 20, [30, 60, 90])),
                bbox: None,
                is_visual: true,
                is_decorative: false,
            }],
        );
        let succeeding = VisualModuleProvider::new(None);
        AnalysisService::analyze_new_pages_with_provider(&service, Some(&succeeding))
            .expect("initial visual analysis");
        let mut conn = service.get_db_connection().expect("connection");
        let before = PdfStructureRepository::find_block_by_id(&mut conn, "block-preserved")
            .expect("block lookup")
            .expect("block")
            .enrichment_json
            .expect("initial enrichment");
        drop(conn);

        let failing = VisualModuleProvider::new(Some("block-preserved"));
        let result = AnalysisService::reanalyze_document_with_provider(
            &service,
            &document_id,
            Some(&failing),
        )
        .expect("failed reanalysis remains a completed batch");
        assert_eq!(result.failed_visual_modules, 1);

        let mut conn = service.get_db_connection().expect("connection");
        let after = PdfStructureRepository::find_block_by_id(&mut conn, "block-preserved")
            .expect("block lookup")
            .expect("block")
            .enrichment_json
            .expect("preserved enrichment");
        assert_eq!(after, before);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn visual_block_failure_is_independent_and_failed_retry_targets_only_that_block() {
        let (service, root) = test_workspace();
        configure_mock_with_concurrency(&service, 1);
        let (document_id, _) = seed_structured_document(
            &service,
            vec![
                VisualBlockSpec {
                    block_id: "block-ok",
                    source_text: "keep this source",
                    source_image_path: Some("structure/retry/images/block-ok.png".to_string()),
                    source_image: Some((20, 20, [20, 180, 20])),
                    bbox: Some(NormalizedBbox {
                        x: 0.0,
                        y: 0.0,
                        width: 0.5,
                        height: 0.5,
                    }),
                    is_visual: true,
                    is_decorative: false,
                },
                VisualBlockSpec {
                    block_id: "block-fail",
                    source_text: "retry this source",
                    source_image_path: Some("structure/retry/images/block-fail.png".to_string()),
                    source_image: Some((20, 20, [180, 20, 20])),
                    bbox: Some(NormalizedBbox {
                        x: 0.5,
                        y: 0.5,
                        width: 0.5,
                        height: 0.5,
                    }),
                    is_visual: true,
                    is_decorative: false,
                },
            ],
        );
        let failing = VisualModuleProvider::new(Some("block-fail"));
        let first = AnalysisService::analyze_new_pages_with_provider(&service, Some(&failing))
            .expect("partial visual batch");
        assert_eq!(first.succeeded_pages, 0);
        assert_eq!(first.failed_pages, 0);
        assert_eq!(first.succeeded_visual_modules, 1);
        assert_eq!(first.failed_visual_modules, 1);
        assert_eq!(first.status, "succeeded_with_failures");
        assert_eq!(
            first.error.as_ref().map(|error| error.code.as_str()),
            Some("test_visual_provider_failure")
        );

        let mut conn = service.get_db_connection().expect("connection");
        let pages = AnalysisRepository::list_workbench_pages(&mut conn, &document_id)
            .expect("workbench pages");
        assert_eq!(pages[0].succeeded_visual_module_count, Some(1));
        assert_eq!(pages[0].failed_visual_module_count, Some(1));
        drop(conn);

        let default_batch = VisualModuleProvider::new(None);
        let no_retry =
            AnalysisService::analyze_new_pages_with_provider(&service, Some(&default_batch))
                .expect("failed blocks stay out of the default batch");
        assert_eq!(no_retry.total_visual_modules, 0);
        assert!(no_retry.error.is_none());
        assert_eq!(default_batch.call_count(), 0);

        let retry = VisualModuleProvider::new(None);
        let second = AnalysisService::reanalyze_failed_pages_with_provider(
            &service,
            &document_id,
            Some(&retry),
        )
        .expect("failed visual retry");
        assert_eq!(second.total_pages, 0);
        assert_eq!(second.succeeded_pages, 0);
        assert_eq!(second.total_visual_modules, 1);
        assert_eq!(second.succeeded_visual_modules, 1);
        assert_eq!(retry.call_count(), 1);

        let mut conn = service.get_db_connection().expect("connection");
        let rows = block_on_db(async {
            sqlx::query_as::<_, (String, String, i64)>(
                "SELECT block_id, status, attempt_count FROM visual_module_analysis ORDER BY block_id",
            )
            .fetch_all(&mut conn)
            .await
            .map_err(|err| {
                crate::repositories::db::database_error("test", "visual_status", err)
            })
        })
        .expect("visual statuses");
        assert_eq!(
            rows,
            vec![
                ("block-fail".to_string(), "succeeded".to_string(), 2),
                ("block-ok".to_string(), "succeeded".to_string(), 1),
            ]
        );
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

    struct FormatRetryProvider {
        call_count: std::sync::atomic::AtomicUsize,
        last_prompt: std::sync::Mutex<Option<String>>,
    }

    impl FormatRetryProvider {
        fn new() -> Self {
            Self {
                call_count: std::sync::atomic::AtomicUsize::new(0),
                last_prompt: std::sync::Mutex::new(None),
            }
        }
    }

    impl ModelProvider for FormatRetryProvider {
        fn analyze_page(&self, request: &ModelAnalysisRequest) -> AppResult<ModelAnalysisResponse> {
            let call = self
                .call_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                + 1;
            *self.last_prompt.lock().expect("prompt lock") = Some(request.prompt.clone());
            if call == 1 {
                return Ok(ModelAnalysisResponse {
                    raw_json: "这里只是一段纯文本，不是 JSON。".to_string(),
                    provider: request.provider.clone(),
                    model_name: request.model_name.clone(),
                    provider_response_json: Some("{\"first\":true}".to_string()),
                });
            }

            let expected = &request.expected_page;
            let raw_json = serde_json::json!({
                "schema_version": PAGE_ANALYSIS_SCHEMA_VERSION,
                "page_id": expected.page_id,
                "image_hash": expected.image_hash,
                "image_path": expected.image_path,
                "source": {
                    "document_id": expected.document_id,
                    "page_number": expected.page_number,
                    "original_filename": null
                },
                "analysis": {
                    "title": "修正后的标题",
                    "summary": "修正后的中文摘要",
                    "visible_text": "修正后的可见文字",
                    "topics": ["修正"],
                    "keywords": ["JSON"]
                },
                "retrieval": {
                    "bm25_text": "修正后的可检索文本"
                },
                "model": {
                    "provider": request.provider,
                    "model_name": request.model_name
                }
            })
            .to_string();
            let provider_response_json = serde_json::json!({
                "id": "retried",
                "object": "chat.completion",
                "created": 1768897758_i64,
                "model": request.model_name,
                "choices": [
                    {
                        "index": 0,
                        "message": {
                            "role": "assistant",
                            "content": raw_json
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
                raw_json,
                provider: request.provider.clone(),
                model_name: request.model_name.clone(),
                provider_response_json: Some(provider_response_json),
            })
        }
    }

    struct AlwaysPlainTextProvider {
        call_count: std::sync::atomic::AtomicUsize,
    }

    impl AlwaysPlainTextProvider {
        fn new() -> Self {
            Self {
                call_count: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    impl ModelProvider for AlwaysPlainTextProvider {
        fn analyze_page(&self, request: &ModelAnalysisRequest) -> AppResult<ModelAnalysisResponse> {
            let call = self
                .call_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                + 1;
            let raw_json = if call == 1 {
                "第一次返回自然语言描述，没有 JSON。"
            } else {
                "第二次仍然只返回自然语言描述，用于验证兜底分析。"
            };
            let provider_response_json = serde_json::json!({
                "id": format!("plain-{call}"),
                "choices": [
                    {
                        "message": {
                            "content": raw_json
                        }
                    }
                ]
            })
            .to_string();

            Ok(ModelAnalysisResponse {
                raw_json: raw_json.to_string(),
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

    struct VisualModuleProvider {
        calls: std::sync::atomic::AtomicUsize,
        dimensions: Mutex<Vec<(String, u32, u32)>>,
        fail_block: Option<&'static str>,
    }

    impl VisualModuleProvider {
        fn new(fail_block: Option<&'static str>) -> Self {
            Self {
                calls: std::sync::atomic::AtomicUsize::new(0),
                dimensions: Mutex::new(Vec::new()),
                fail_block,
            }
        }

        fn call_count(&self) -> usize {
            self.calls.load(std::sync::atomic::Ordering::SeqCst)
        }

        fn block_id(prompt: &str) -> AppResult<String> {
            let marker = "\"block_id\": \"";
            prompt
                .split_once(marker)
                .and_then(|(_, rest)| rest.split_once('"'))
                .map(|(block_id, _)| block_id.to_string())
                .ok_or_else(|| {
                    AppError::new(
                        "test_visual_block_id_missing",
                        "visual block id missing from prompt",
                        "test",
                        false,
                    )
                })
        }
    }

    impl ModelProvider for VisualModuleProvider {
        fn analyze_page(&self, request: &ModelAnalysisRequest) -> AppResult<ModelAnalysisResponse> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let block_id = Self::block_id(&request.prompt)?;
            if self.fail_block == Some(block_id.as_str()) {
                return Err(AppError::new(
                    "test_visual_provider_failure",
                    "visual provider failed for this block",
                    "analysis_provider",
                    true,
                ));
            }
            let image = image::load_from_memory(&request.image_bytes).map_err(|err| {
                AppError::new(
                    "test_visual_image_invalid",
                    "visual input was not a decodable image",
                    "test",
                    false,
                )
                .with_details(err.to_string())
            })?;
            let (width, height) = image.dimensions();
            self.dimensions.lock().expect("dimensions lock").push((
                block_id.clone(),
                width,
                height,
            ));
            let raw_json = serde_json::json!({
                "schema_version": VISUAL_MODULE_ANALYSIS_SCHEMA_VERSION,
                "block_id": block_id,
                "description": "Structured visual description",
                "visible_text": "visible module text",
                "keywords": ["visual", "module"],
                "model": {
                    "provider": request.provider,
                    "model_name": request.model_name,
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
