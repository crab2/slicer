export type WorkspaceStatusValue =
  | "not_selected"
  | "loading"
  | "ready"
  | "missing"
  | "invalid"
  | "error";

export type ErrorStage =
  | "restore"
  | "select"
  | "initialize"
  | "validate"
  | "workspace"
  | "settings"
  | "ledger"
  | "migration";

export interface AppErrorDto {
  code: string;
  message: string;
  stage: ErrorStage | string;
  retryable: boolean;
  details?: string | null;
  correlation_id: string;
}

export interface WorkspaceStatusDto {
  status: WorkspaceStatusValue;
  workspace_path?: string | null;
  error?: AppErrorDto | null;
}

export interface AppSettingsDto {
  workspace_path?: string | null;
  libreoffice_path?: string | null;
  model_provider: string;
  api_key_configured: boolean;
  base_url: string;
  custom_endpoint: string;
  model_name: string;
  default_image_dpi: number;
  conversion_concurrency: number;
  analysis_concurrency: number;
  api_enabled: boolean;
  api_bind_address: string;
  api_port: number;
}

export interface ModelConfigurationStatusDto {
  configured: boolean;
  missing: string[];
  privacy_notice_accepted: boolean;
  requires_privacy_notice: boolean;
}

export interface ModelInfoDto {
  id: string;
  display_name?: string | null;
  owned_by?: string | null;
}

export interface ModelListDto {
  provider: string;
  models: ModelInfoDto[];
}

export interface PrivacyNoticeStatusDto {
  accepted: boolean;
  requires_notice: boolean;
}

export interface CoreStatusCatalogDto {
  document_statuses: string[];
  page_statuses: string[];
  job_statuses: string[];
}

export type JobStatusValue =
  | "queued"
  | "running"
  | "succeeded"
  | "failed"
  | "cancelled";

export interface JobDto {
  job_id: string;
  job_type: string;
  status: JobStatusValue | string;
  progress: number;
  created_at: string;
  updated_at: string;
  error_id?: string | null;
  error_summary?: string | null;
  last_event_message?: string | null;
}

export interface CreateJobRequestDto {
  job_type: string;
}

export interface UpdateJobProgressRequestDto {
  job_id: string;
  progress: number;
  message?: string | null;
}

export interface DocumentDto {
  document_id: string;
  original_filename: string;
  file_type: string;
  file_hash: string;
  original_path: string;
  page_count: number | null;
  status: string;
  error_summary: string | null;
  job_id: string | null;
  analysis_succeeded_pages: number;
  analysis_failed_pages: number;
  last_analyzed_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface PageRecordDto {
  page_id: string;
  document_id: string;
  page_number: number;
  image_hash: string;
  status: string;
  error_summary: string | null;
  created_at: string;
  updated_at: string;
}

export interface PageAnalysisSummaryDto {
  title: string | null;
  summary: string | null;
  keywords: string[];
  topic_count: number;
  visible_text_char_count: number;
}

export interface PageWorkbenchDto {
  page_id: string;
  document_id: string;
  page_number: number;
  image_hash: string;
  image_path: string | null;
  status: string;
  error_summary: string | null;
  created_at: string;
  updated_at: string;
  analysis_summary: PageAnalysisSummaryDto | null;
}

export interface AnalysisResultDto {
  analysis_id: string;
  page_id: string;
  schema_version: string;
  provider: string;
  model_name: string;
  status: "succeeded" | "failed" | string;
  result_json: string | null;
  error_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface AnalysisBatchResultDto {
  job_id: string;
  total_pages: number;
  succeeded_pages: number;
  failed_pages: number;
  skipped_pages: number;
  status: string;
  updated_at: string;
}

export type ImportResultStatus = "success" | "duplicate" | "unsupported" | "failed";

export interface ImportResultDto {
  file_name: string;
  status: ImportResultStatus;
  document?: DocumentDto | null;
  error?: string | null;
}

export interface IndexStatusDto {
  status: string;
  provider: string;
  active_version_id: string | null;
  indexed_page_count: number;
  analyzable_page_count: number;
  pending_index_page_count: number;
  building_version_id: string | null;
  building_job_id: string | null;
  error_summary: string | null;
  correlation_id: string | null;
  can_search: boolean;
  can_rebuild: boolean;
  stale: boolean;
  stale_reason: string | null;
  search_uses_stale_index: boolean;
}

export interface IndexRebuildStartDto {
  job_id: string;
  version_id: string;
}

export interface SearchResultItemDto {
  page_id: string;
  document_id: string;
  page_number: number;
  original_filename: string | null;
  score: number;
  title: string | null;
  summary: string | null;
  image_path: string | null;
  image_available: boolean;
  page_json: string;
}

export interface SearchResponseDto {
  items: SearchResultItemDto[];
  query: string;
  limit: number;
}

export type ApiServerRuntimeStatus = "running" | "stopped" | "failed" | "disabled";

export interface ApiServerStatusDto {
  runtime_status: ApiServerRuntimeStatus;
  bind_address: string;
  port: number;
  enabled: boolean;
  last_error: AppErrorDto | null;
}

export interface ApiKeyRecordDto {
  key_id: string;
  provider: string;
  label: string;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface ApiKeyListDto {
  keys: ApiKeyRecordDto[];
}

export interface MediaExportResultDto {
  markdown_path: string;
  export_dir: string;
  document_count: number;
  media_count: number;
}
