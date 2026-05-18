import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  AppErrorDto,
  AppSettingsDto,
  CoreStatusCatalogDto,
  CreateJobRequestDto,
  JobDto,
  UpdateJobProgressRequestDto,
  WorkspaceStatusDto,
} from "../types/app";

export type TauriCommandArgs = Record<string, unknown>;

export async function callTauriCommand<TResult>(
  command: string,
  args?: TauriCommandArgs,
): Promise<TResult> {
  return invoke<TResult>(command, args);
}

function createJob(jobType: string) {
  const request: CreateJobRequestDto = { job_type: jobType };
  return callTauriCommand<JobDto>("create_job", { request });
}

function updateJobProgress(jobId: string, progress: number, message?: string | null) {
  const request: UpdateJobProgressRequestDto = {
    job_id: jobId,
    progress,
    message: message ?? null,
  };
  return callTauriCommand<JobDto>("update_job_progress", { request });
}

export const tauriClient = {
  call: callTauriCommand,
  getWorkspaceStatus: () => callTauriCommand<WorkspaceStatusDto>("get_workspace_status"),
  selectWorkspace: (path: string) =>
    callTauriCommand<WorkspaceStatusDto>("select_workspace", { path }),
  openWorkspaceDialog: async () => {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected !== "string") {
      return callTauriCommand<WorkspaceStatusDto>("get_workspace_status");
    }
    return callTauriCommand<WorkspaceStatusDto>("select_workspace", { path: selected });
  },
  getAppSettings: () => callTauriCommand<AppSettingsDto>("get_app_settings"),
  saveApiKey: (key: string) => callTauriCommand<void>("save_api_key", { key }),
  deleteApiKey: () => callTauriCommand<void>("delete_api_key"),
  getCoreStatusCatalog: () =>
    callTauriCommand<CoreStatusCatalogDto>("get_core_status_catalog"),
  listJobs: () => callTauriCommand<JobDto[]>("list_jobs"),
  createJob,
  createPlaceholderJob: createJob,
  updateJobProgress,
  failJob: (jobId: string, code: string, message: string) =>
    callTauriCommand<JobDto>("fail_job", { job_id: jobId, code, message }),
  recoverInterruptedJobs: () => callTauriCommand<JobDto[]>("recover_interrupted_jobs"),
  recordDiagnosticError: (code: string, message: string, stage: string) =>
    callTauriCommand<AppErrorDto>("record_diagnostic_error", { code, message, stage }),
};
