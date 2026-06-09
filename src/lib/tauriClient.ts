import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import type {
  AnalysisBatchResultDto,
  AnalysisResultDto,
  ApiServerStatusDto,
  ApiKeyListDto,
  AppErrorDto,
  AppSettingsDto,
  ModelConfigurationStatusDto,
  PrivacyNoticeStatusDto,
  CoreStatusCatalogDto,
  CreateJobRequestDto,
  DocumentDto,
  ImportResultDto,
  JobDto,
  MediaExportResultDto,
  ModelListDto,
  PageRecordDto,
  PageWorkbenchDto,
  IndexRebuildStartDto,
  IndexStatusDto,
  SearchResponseDto,
  UpdateJobProgressRequestDto,
  WorkspaceStatusDto,
} from "../types/app";
import {
  getUnsupportedDocumentReason,
  getUnsupportedImageReason,
  getUnsupportedReason,
  isDocumentFileType,
  isImageFileType,
  isSupportedFileType,
} from "./fileValidation";

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
  openPdfDialog: async () => {
    const selected = await open({
      multiple: false,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    return selected;
  },
  openMultiPdfDialog: async () => {
    const selected = await open({
      multiple: true,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    return selected;
  },
  openImportDialog: async () => {
    const selected = await open({
      multiple: true,
      filters: [
        {
          name: "文档",
          extensions: ["pdf", "doc", "docx", "ppt", "pptx"],
        },
      ],
    });
    return selected;
  },
  openImageImportDialog: async () => {
    const selected = await open({
      multiple: true,
      filters: [
        {
          name: "图片",
          extensions: ["png", "jpg", "jpeg"],
        },
      ],
    });
    return selected;
  },
  getAppSettings: () => callTauriCommand<AppSettingsDto>("get_app_settings"),
  saveAppSettings: (settings: AppSettingsDto) =>
    callTauriCommand<void>("save_app_settings", { settings }),
  findLibreOfficePath: () => callTauriCommand<string | null>("find_libreoffice_path"),
  openLibreOfficeDirectoryDialog: () =>
    open({ directory: true, multiple: false }),
  openExternalUrl: (url: string) => openUrl(url),
  saveApiKey: (key: string) => callTauriCommand<void>("save_api_key", { key }),
  saveProviderApiKey: (provider: string, key: string) =>
    callTauriCommand<void>("save_provider_api_key", { provider, key }),
  listApiKeys: () => callTauriCommand<ApiKeyListDto>("list_api_keys"),
  addApiKey: (provider: string, label: string, key: string, activate = true) =>
    callTauriCommand<ApiKeyListDto>("add_api_key", { provider, label, key, activate }),
  activateApiKey: (provider: string, keyId: string) =>
    callTauriCommand<ApiKeyListDto>("activate_api_key", { provider, keyId }),
  deleteApiKeyRecord: (provider: string, keyId: string) =>
    callTauriCommand<ApiKeyListDto>("delete_api_key_record", { provider, keyId }),
  deleteApiKey: () => callTauriCommand<void>("delete_api_key"),
  deleteProviderApiKey: (provider: string) =>
    callTauriCommand<void>("delete_provider_api_key", { provider }),
  getModelConfigurationStatus: () =>
    callTauriCommand<ModelConfigurationStatusDto>("get_model_configuration_status"),
  listOpenAIModels: (settings: AppSettingsDto) =>
    callTauriCommand<ModelListDto>("list_openai_models", { settings }),
  getPrivacyNoticeStatus: () =>
    callTauriCommand<PrivacyNoticeStatusDto>("get_privacy_notice_status"),
  acceptPrivacyNotice: () => callTauriCommand<void>("accept_privacy_notice"),
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
  importPdf: (filePath: string) =>
    callTauriCommand<DocumentDto>("import_pdf", { filePath }),
  importImage: (filePath: string) =>
    callTauriCommand<DocumentDto>("import_image", { filePath }),
  importFile: (filePath: string) =>
    isImageFileType(filePath)
      ? callTauriCommand<DocumentDto>("import_image", { filePath })
      : callTauriCommand<DocumentDto>("import_pdf", { filePath }),
  importMultipleFiles: async (filePaths: string[]): Promise<ImportResultDto[]> => {
    const results: ImportResultDto[] = [];
    let importedIds = new Set<string>();
    try {
      const existingDocuments = await tauriClient.listDocuments();
      importedIds = new Set(existingDocuments.map((doc) => doc.document_id));
    } catch {
      importedIds = new Set<string>();
    }
    for (const filePath of filePaths) {
      const fileName = filePath.split(/[/\\]/).pop() ?? filePath;
      if (!isSupportedFileType(filePath)) {
        results.push({
          file_name: fileName,
          status: "unsupported",
          error: getUnsupportedReason(filePath),
        });
        continue;
      }
      try {
        const doc = await tauriClient.importFile(filePath);
        const duplicate = importedIds.has(doc.document_id);
        importedIds.add(doc.document_id);
        results.push({
          file_name: fileName,
          status: duplicate ? "duplicate" : "success",
          document: doc,
        });
      } catch (error) {
        results.push({ file_name: fileName, status: "failed", error: extractImportError(error) });
      }
    }
    return results;
  },
  importMultipleDocuments: async (filePaths: string[]): Promise<ImportResultDto[]> => {
    const results: ImportResultDto[] = [];
    let importedIds = new Set<string>();
    try {
      const existingDocuments = await tauriClient.listDocuments();
      importedIds = new Set(existingDocuments.map((doc) => doc.document_id));
    } catch {
      importedIds = new Set<string>();
    }
    for (const filePath of filePaths) {
      const fileName = filePath.split(/[/\\]/).pop() ?? filePath;
      if (!isDocumentFileType(filePath)) {
        results.push({
          file_name: fileName,
          status: "unsupported",
          error: getUnsupportedDocumentReason(filePath),
        });
        continue;
      }
      try {
        const doc = await tauriClient.importPdf(filePath);
        const duplicate = importedIds.has(doc.document_id);
        importedIds.add(doc.document_id);
        results.push({
          file_name: fileName,
          status: duplicate ? "duplicate" : "success",
          document: doc,
        });
      } catch (error) {
        results.push({ file_name: fileName, status: "failed", error: extractImportError(error) });
      }
    }
    return results;
  },
  importMultipleImages: async (filePaths: string[]): Promise<ImportResultDto[]> => {
    const results: ImportResultDto[] = [];
    let importedIds = new Set<string>();
    try {
      const existingDocuments = await tauriClient.listDocuments();
      importedIds = new Set(existingDocuments.map((doc) => doc.document_id));
    } catch {
      importedIds = new Set<string>();
    }
    for (const filePath of filePaths) {
      const fileName = filePath.split(/[/\\]/).pop() ?? filePath;
      if (!isImageFileType(filePath)) {
        results.push({
          file_name: fileName,
          status: "unsupported",
          error: getUnsupportedImageReason(filePath),
        });
        continue;
      }
      try {
        const doc = await tauriClient.importImage(filePath);
        const duplicate = importedIds.has(doc.document_id);
        importedIds.add(doc.document_id);
        results.push({
          file_name: fileName,
          status: duplicate ? "duplicate" : "success",
          document: doc,
        });
      } catch (error) {
        results.push({ file_name: fileName, status: "failed", error: extractImportError(error) });
      }
    }
    return results;
  },
  importMultiplePdf: async (filePaths: string[]): Promise<ImportResultDto[]> =>
    tauriClient.importMultipleDocuments(filePaths),
  listDocuments: () => callTauriCommand<DocumentDto[]>("list_documents"),
  retryImport: (documentId: string) =>
    callTauriCommand<DocumentDto>("retry_import", { documentId }),
  deleteDocument: (documentId: string) =>
    callTauriCommand<void>("delete_document", { documentId }),
  revealDocumentInFolder: (path: string) => revealItemInDir(path),
  listPages: (documentId: string) =>
    callTauriCommand<PageRecordDto[]>(`list_pages`, { documentId }),
  listWorkbenchPages: (documentId: string) =>
    callTauriCommand<PageWorkbenchDto[]>("list_workbench_pages", { documentId }),
  analyzePage: (pageId: string) =>
    callTauriCommand<AnalysisResultDto>("analyze_page", { pageId }),
  analyzeNewPages: () =>
    callTauriCommand<AnalysisBatchResultDto>("analyze_new_pages"),
  reanalyzeDocument: (documentId: string) =>
    callTauriCommand<AnalysisBatchResultDto>("reanalyze_document", { documentId }),
  reanalyzeFailedPages: (documentId: string) =>
    callTauriCommand<AnalysisBatchResultDto>("reanalyze_failed_pages", { documentId }),
  recoverInterruptedAnalysisPages: () =>
    callTauriCommand<number>("recover_interrupted_analysis_pages"),
  getIndexStatus: () => callTauriCommand<IndexStatusDto>("get_index_status"),
  searchPages: (query: string, limit = 20) =>
    callTauriCommand<SearchResponseDto>("search_pages", { query, limit }),
  getPageImagePreview: (pageId: string) =>
    callTauriCommand<string | null>("get_page_image_preview", { pageId }),
  startIndexRebuild: () =>
    callTauriCommand<IndexRebuildStartDto>("start_index_rebuild"),
  getApiServerStatus: () =>
    callTauriCommand<ApiServerStatusDto>("get_api_server_status"),
  resetApiToken: () => callTauriCommand<string>("reset_api_token"),
  openExportFolderDialog: async () => {
    const selected = await open({ directory: true, multiple: false });
    return selected;
  },
  exportMedia: (destination: string) =>
    callTauriCommand<MediaExportResultDto>("export_media", { destination }),
};

function extractImportError(error: unknown) {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String((error as Record<string, unknown>).message);
  }
  if (typeof error === "string") {
    return error;
  }
  return "导入失败";
}
