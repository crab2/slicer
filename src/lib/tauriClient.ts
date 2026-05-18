import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  AppErrorDto,
  AppSettingsDto,
  CoreStatusCatalogDto,
  JobDto,
  WorkspaceStatusDto,
} from "../types/app";

export type TauriCommandArgs = Record<string, unknown>;

export async function callTauriCommand<TResult>(
  command: string,
  args?: TauriCommandArgs,
): Promise<TResult> {
  return invoke<TResult>(command, args);
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
  getCoreStatusCatalog: () =>
    callTauriCommand<CoreStatusCatalogDto>("get_core_status_catalog"),
  listJobs: () => callTauriCommand<JobDto[]>("list_jobs"),
  createPlaceholderJob: (jobType: string) =>
    callTauriCommand<JobDto>("create_placeholder_job", { job_type: jobType }),
  recordDiagnosticError: (code: string, message: string, stage: string) =>
    callTauriCommand<AppErrorDto>("record_diagnostic_error", { code, message, stage }),
};
