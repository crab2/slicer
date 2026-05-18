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
