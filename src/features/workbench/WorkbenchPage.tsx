import { useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Button } from "../../components/common/Button";
import { ErrorMessage } from "../../components/common/ErrorMessage";
import { EmptyState } from "../../components/common/EmptyState";
import { StatusBadge } from "../../components/common/StatusBadge";
import { PrivacyNotice } from "../settings/components/PrivacyNotice";
import { tauriClient } from "../../lib/tauriClient";
import { getUnsupportedDocumentReason, isDocumentFileType } from "../../lib/fileValidation";
import type {
  AnalysisBatchResultDto,
  DocumentDto,
  ImportResultDto,
  JobDto,
  MediaExportResultDto,
  ModelConfigurationStatusDto,
  PageWorkbenchDto,
  WorkspaceStatusDto,
} from "../../types/app";
import { IndexStatusPanel } from "../search/components/IndexStatusPanel";
import { AnalysisJobList, isAnalysisJobType } from "./components/AnalysisJobList";
import { DocumentList } from "./components/DocumentList";
import { ImportResultList } from "./components/ImportResultList";
import { JobList } from "./components/JobList";
import { WorkspacePicker } from "./components/WorkspacePicker";

const demoJobType = "placeholder_import";

interface WorkbenchPageProps {
  workspaceStatus: WorkspaceStatusDto;
  isWorkspaceLoading: boolean;
  isActive: boolean;
  onChooseWorkspace: () => void;
  onOpenSettings: () => void;
}

export function WorkbenchPage({
  workspaceStatus,
  isWorkspaceLoading,
  isActive,
  onChooseWorkspace,
  onOpenSettings,
}: WorkbenchPageProps) {
  const workspaceReady = workspaceStatus.status === "ready";
  const workspaceKey = workspaceStatus.workspace_path ?? "current";
  const recoveredWorkspaceRef = useRef<string | null>(null);
  const jobsGenRef = useRef(0);
  const docsGenRef = useRef(0);
  const [jobs, setJobs] = useState<JobDto[]>([]);
  const [isJobsLoading, setIsJobsLoading] = useState(false);
  const [isCreatingDemo, setIsCreatingDemo] = useState(false);
  const [jobsError, setJobsError] = useState<{ message: string; correlationId?: string | null } | null>(null);
  const [documents, setDocuments] = useState<DocumentDto[]>([]);
  const [pagesByDocument, setPagesByDocument] = useState<
    Record<string, PageWorkbenchDto[]>
  >({});
  const [isDocsLoading, setIsDocsLoading] = useState(false);
  const [isImporting, setIsImporting] = useState(false);
  const [isDragActive, setIsDragActive] = useState(false);
  const [importError, setImportError] = useState<{ message: string; correlationId?: string | null } | null>(null);
  const [importResults, setImportResults] = useState<ImportResultDto[]>([]);
  const [modelStatus, setModelStatus] = useState<ModelConfigurationStatusDto | null>(null);
  const [isModelStatusLoading, setIsModelStatusLoading] = useState(false);
  const [showPrivacyNotice, setShowPrivacyNotice] = useState(false);
  const [isAcceptingPrivacy, setIsAcceptingPrivacy] = useState(false);
  const [analysisReadyMessage, setAnalysisReadyMessage] = useState<string | null>(null);
  const [analysisError, setAnalysisError] = useState<{ message: string; correlationId?: string | null } | null>(null);
  const [analyzingPageId, setAnalyzingPageId] = useState<string | null>(null);
  const [pendingAnalysisPageId, setPendingAnalysisPageId] = useState<string | null>(null);
  const [pendingBatchAction, setPendingBatchAction] = useState<"new-pages" | null>(null);
  const [pendingReanalysisDocumentId, setPendingReanalysisDocumentId] = useState<string | null>(null);
  const [pendingFailedReanalysisDocumentId, setPendingFailedReanalysisDocumentId] = useState<string | null>(null);
  const [isBatchAnalyzing, setIsBatchAnalyzing] = useState(false);
  const [reanalyzingDocumentId, setReanalyzingDocumentId] = useState<string | null>(null);
  const [reanalyzingFailedDocumentId, setReanalyzingFailedDocumentId] = useState<string | null>(null);
  const [deletingDocumentId, setDeletingDocumentId] = useState<string | null>(null);
  const [isExporting, setIsExporting] = useState(false);
  const [exportError, setExportError] = useState<{ message: string; correlationId?: string | null } | null>(null);
  const [exportResult, setExportResult] = useState<MediaExportResultDto | null>(null);
  const importLockRef = useRef(false);
  const workspaceReadyRef = useRef(workspaceReady);
  const isActiveRef = useRef(isActive);
  const documentsRef = useRef<DocumentDto[]>(documents);
  const importDocumentFilesRef = useRef<(filePaths: string[]) => Promise<void>>(async () => undefined);

  workspaceReadyRef.current = workspaceReady;
  isActiveRef.current = isActive;
  documentsRef.current = documents;

  async function refreshJobs(options: { recoverInterrupted?: boolean } = {}) {
    if (!workspaceReady) {
      setJobs([]);
      return;
    }

    const gen = ++jobsGenRef.current;
    setIsJobsLoading(true);
    setJobsError(null);
    try {
      if (options.recoverInterrupted) {
        recoveredWorkspaceRef.current = workspaceKey;
        await tauriClient.recoverInterruptedJobs();
        await tauriClient.recoverInterruptedAnalysisPages();
      }
      const jobsResult = await tauriClient.listJobs();
      if (gen === jobsGenRef.current) setJobs(jobsResult);
    } catch (error) {
      if (gen === jobsGenRef.current) setJobsError(extractError(error));
    } finally {
      if (gen === jobsGenRef.current) setIsJobsLoading(false);
    }
  }

  async function refreshDocuments() {
    if (!workspaceReady) {
      setDocuments([]);
      setPagesByDocument({});
      return;
    }
    const gen = ++docsGenRef.current;
    setIsDocsLoading(true);
    try {
      const docs = await tauriClient.listDocuments();
      if (gen !== docsGenRef.current) return;
      const pagesMap: Record<string, PageWorkbenchDto[]> = {};
      for (const doc of docs) {
        try {
          pagesMap[doc.document_id] = await tauriClient.listWorkbenchPages(
            doc.document_id,
          );
        } catch {
          pagesMap[doc.document_id] = [];
        }
        if (gen !== docsGenRef.current) return;
      }
      setDocuments(docs);
      setPagesByDocument(pagesMap);
    } catch {
      if (gen === docsGenRef.current) setDocuments([]);
    } finally {
      if (gen === docsGenRef.current) setIsDocsLoading(false);
    }
  }

  async function handleImportDocuments() {
    if (importLockRef.current) {
      return;
    }
    const selected = await tauriClient.openImportDialog();
    if (!selected) return;
    const filePaths = Array.isArray(selected) ? selected : [selected];
    await importDocumentFiles(filePaths);
  }

  async function importDocumentFiles(filePaths: string[]) {
    await importDocumentFilesWithInitialResults(filePaths, []);
  }

  async function importDocumentFilesWithInitialResults(
    filePaths: unknown[],
    initialResults: ImportResultDto[],
  ) {
    if (importLockRef.current) {
      return;
    }

    const normalizedPaths = normalizeImportPaths(filePaths);
    if (normalizedPaths.length === 0 && initialResults.length === 0) {
      return;
    }

    importLockRef.current = true;
    setIsImporting(true);
    setIsDragActive(false);
    setImportError(null);
    setImportResults(initialResults);

    let existingIds = new Set(documentsRef.current.map((d) => d.document_id));
    try {
      const latestDocuments = await tauriClient.listDocuments();
      existingIds = new Set(latestDocuments.map((doc) => doc.document_id));
    } catch {
      existingIds = new Set(documentsRef.current.map((d) => d.document_id));
    }
    const results: ImportResultDto[] = [...initialResults];

    try {
      for (const filePath of normalizedPaths) {
        const fileName = filePath.split(/[/\\]/).pop() ?? filePath;
        if (!isDocumentFileType(filePath)) {
          results.push({
            file_name: fileName,
            status: "unsupported",
            error: getUnsupportedDocumentReason(filePath),
          });
          setImportResults([...results]);
          continue;
        }

        try {
          const doc = await tauriClient.importPdf(filePath);
          results.push({
            file_name: fileName,
            status: existingIds.has(doc.document_id) ? "duplicate" : "success",
            document: doc,
          });
          existingIds.add(doc.document_id);
        } catch (error) {
          const errInfo = extractError(error);
          results.push({
            file_name: fileName,
            status: "failed",
            error: errInfo.message,
          });
        }
        setImportResults([...results]);
      }

      try {
        await Promise.all([refreshDocuments(), refreshJobs()]);
      } catch (error) {
        setImportError(extractError(error));
      }
    } catch (error) {
      setImportError(extractError(error));
    } finally {
      importLockRef.current = false;
      setIsImporting(false);
    }
  }

  importDocumentFilesRef.current = importDocumentFiles;

  function showImportBusyDropResult() {
    setImportError(null);
    setImportResults([
      {
        file_name: "拖拽内容",
        status: "failed",
        error: "已有导入正在进行，请等待当前批次完成后再拖入新文件。",
      },
    ]);
  }

  function canAcceptDocumentDrop() {
    return workspaceReady && isActive && !importLockRef.current;
  }

  function handleDragEnter(event: React.DragEvent<HTMLElement>) {
    event.preventDefault();
    event.stopPropagation();
    if (canAcceptDocumentDrop()) {
      setIsDragActive(true);
    }
  }

  function handleDragOver(event: React.DragEvent<HTMLElement>) {
    event.preventDefault();
    event.stopPropagation();
    if (canAcceptDocumentDrop()) {
      event.dataTransfer.dropEffect = "copy";
      setIsDragActive(true);
    } else {
      event.dataTransfer.dropEffect = "none";
    }
  }

  function handleDragLeave(event: React.DragEvent<HTMLElement>) {
    event.preventDefault();
    event.stopPropagation();
    const nextTarget = event.relatedTarget;
    if (!(nextTarget instanceof Node) || !event.currentTarget.contains(nextTarget)) {
      setIsDragActive(false);
    }
  }

  async function handleDrop(event: React.DragEvent<HTMLElement>) {
    event.preventDefault();
    event.stopPropagation();
    setIsDragActive(false);
    if (!canAcceptDocumentDrop()) {
      if (importLockRef.current) {
        showImportBusyDropResult();
      }
      return;
    }

    const files = Array.from(event.dataTransfer.files);
    const filePaths: string[] = [];
    const pathlessResults: ImportResultDto[] = [];
    for (const file of files) {
      const path = getDroppedFilePath(file);
      if (path) {
        filePaths.push(path);
      } else {
        pathlessResults.push({
          file_name: file.name || "拖拽文件",
          status: "failed",
          error: "无法从拖拽事件读取本地路径，请使用“选择文件”导入。",
        });
      }
    }

    if (filePaths.length === 0 && pathlessResults.length === 0) {
      setImportError(null);
      setImportResults([
        {
          file_name: "拖拽内容",
          status: "failed",
          error: "请拖入本地文件。",
        },
      ]);
      return;
    }

    await importDocumentFilesWithInitialResults(filePaths, pathlessResults);
  }

  async function handleRetryImport(documentId: string) {
    try {
      await tauriClient.retryImport(documentId);
      await Promise.all([refreshDocuments(), refreshJobs()]);
    } catch (error) {
      setImportError(extractError(error));
    }
  }

  async function handleCreateDemoJob() {
    setIsCreatingDemo(true);
    setJobsError(null);
    try {
      await tauriClient.createJob(demoJobType);
      await refreshJobs();
    } catch (error) {
      setJobsError(extractError(error));
    } finally {
      setIsCreatingDemo(false);
    }
  }

  async function handleAnalysisEntry() {
    setAnalysisError(null);
    setAnalysisReadyMessage(null);
    if (!modelStatus?.configured) {
      onOpenSettings();
      return;
    }
    if (
      modelStatus.requires_privacy_notice &&
      !modelStatus.privacy_notice_accepted
    ) {
      setPendingBatchAction("new-pages");
      setShowPrivacyNotice(true);
      return;
    }
    await executeAnalyzeNewPages();
  }

  async function handleAnalyzePage(pageId: string) {
    setAnalysisError(null);
    setAnalysisReadyMessage(null);
    if (!modelStatus?.configured) {
      onOpenSettings();
      return;
    }
    if (
      modelStatus.requires_privacy_notice &&
      !modelStatus.privacy_notice_accepted
    ) {
      setPendingAnalysisPageId(pageId);
      setShowPrivacyNotice(true);
      return;
    }

    await executeAnalyzePage(pageId);
  }

  async function handleReanalyzeDocument(documentId: string) {
    setAnalysisError(null);
    setAnalysisReadyMessage(null);
    if (!modelStatus?.configured) {
      onOpenSettings();
      return;
    }
    if (
      modelStatus.requires_privacy_notice &&
      !modelStatus.privacy_notice_accepted
    ) {
      setPendingReanalysisDocumentId(documentId);
      setShowPrivacyNotice(true);
      return;
    }

    await executeReanalyzeDocument(documentId);
  }

  async function handleReanalyzeFailedPages(documentId: string) {
    setAnalysisError(null);
    setAnalysisReadyMessage(null);
    if (!modelStatus?.configured) {
      onOpenSettings();
      return;
    }
    if (
      modelStatus.requires_privacy_notice &&
      !modelStatus.privacy_notice_accepted
    ) {
      setPendingFailedReanalysisDocumentId(documentId);
      setShowPrivacyNotice(true);
      return;
    }

    await executeReanalyzeFailedPages(documentId);
  }

  async function handleOpenSourceFile(path: string) {
    try {
      await tauriClient.revealDocumentInFolder(path);
    } catch (error) {
      setImportError(extractError(error));
    }
  }

  async function handleOpenDocumentImage(page: PageWorkbenchDto) {
    const imagePath = resolveWorkspacePath(page.image_path, workspaceStatus.workspace_path);
    if (!imagePath) {
      setImportError({ message: "页面图片不可用，可能尚未生成或路径无效。" });
      return;
    }

    try {
      await tauriClient.revealDocumentInFolder(imagePath);
    } catch (error) {
      setImportError(extractError(error));
    }
  }

  async function handleDeleteDocument(documentId: string) {
    setDeletingDocumentId(documentId);
    setImportError(null);
    try {
      await tauriClient.deleteDocument(documentId);
      await Promise.all([refreshDocuments(), refreshJobs()]);
    } catch (error) {
      setImportError(extractError(error));
      await Promise.all([refreshDocuments(), refreshJobs()]);
    } finally {
      setDeletingDocumentId(null);
    }
  }

  async function handleExport() {
    setExportError(null);
    setExportResult(null);
    try {
      const folder = await tauriClient.openExportFolderDialog();
      if (!folder) return;
      setIsExporting(true);
      const result = await tauriClient.exportMedia(folder);
      setExportResult(result);
    } catch (error) {
      setExportError(extractError(error));
    } finally {
      setIsExporting(false);
    }
  }

  async function executeAnalyzePage(pageId: string) {
    setAnalyzingPageId(pageId);
    try {
      await tauriClient.analyzePage(pageId);
      setAnalysisReadyMessage("页面分析完成，结果已写入本地账本。");
      await Promise.all([refreshDocuments(), refreshJobs()]);
    } catch (error) {
      setAnalysisError(extractError(error));
      await Promise.all([refreshDocuments(), refreshJobs()]);
    } finally {
      setAnalyzingPageId(null);
    }
  }

  async function executeAnalyzeNewPages() {
    setIsBatchAnalyzing(true);
    try {
      const result = await tauriClient.analyzeNewPages();
      setAnalysisReadyMessage(formatBatchMessage("新页面批量分析完成", result));
      await Promise.all([refreshDocuments(), refreshJobs()]);
    } catch (error) {
      setAnalysisError(extractError(error));
      await Promise.all([refreshDocuments(), refreshJobs()]);
    } finally {
      setIsBatchAnalyzing(false);
    }
  }

  async function executeReanalyzeDocument(documentId: string) {
    setReanalyzingDocumentId(documentId);
    try {
      const result = await tauriClient.reanalyzeDocument(documentId);
      setAnalysisReadyMessage(formatBatchMessage("文档重新分析完成", result));
      await Promise.all([refreshDocuments(), refreshJobs()]);
    } catch (error) {
      setAnalysisError(extractError(error));
      await Promise.all([refreshDocuments(), refreshJobs()]);
    } finally {
      setReanalyzingDocumentId(null);
    }
  }

  async function executeReanalyzeFailedPages(documentId: string) {
    setReanalyzingFailedDocumentId(documentId);
    try {
      const result = await tauriClient.reanalyzeFailedPages(documentId);
      setAnalysisReadyMessage(formatBatchMessage("失败页面重新分析完成", result));
      await Promise.all([refreshDocuments(), refreshJobs()]);
    } catch (error) {
      setAnalysisError(extractError(error));
      await Promise.all([refreshDocuments(), refreshJobs()]);
    } finally {
      setReanalyzingFailedDocumentId(null);
    }
  }

  async function handlePrivacyConfirm() {
    setIsAcceptingPrivacy(true);
    try {
      await tauriClient.acceptPrivacyNotice();
      setShowPrivacyNotice(false);
      const status = await tauriClient.getModelConfigurationStatus();
      setModelStatus(status);
      const pageId = pendingAnalysisPageId;
      const batchAction = pendingBatchAction;
      const documentId = pendingReanalysisDocumentId;
      const failedDocumentId = pendingFailedReanalysisDocumentId;
      setPendingAnalysisPageId(null);
      setPendingBatchAction(null);
      setPendingReanalysisDocumentId(null);
      setPendingFailedReanalysisDocumentId(null);
      if (pageId) {
        await executeAnalyzePage(pageId);
      } else if (documentId) {
        await executeReanalyzeDocument(documentId);
      } else if (failedDocumentId) {
        await executeReanalyzeFailedPages(failedDocumentId);
      } else if (batchAction === "new-pages") {
        await executeAnalyzeNewPages();
      } else {
        setAnalysisReadyMessage("隐私提示已确认。可以开始批量分析。");
      }
    } catch (error) {
      setJobsError(extractError(error));
    } finally {
      setIsAcceptingPrivacy(false);
    }
  }

  useEffect(() => {
    let cancelled = false;
    ++jobsGenRef.current;
    ++docsGenRef.current;
    if (!workspaceReady) {
      setJobs([]);
      setJobsError(null);
      setIsJobsLoading(false);
      setModelStatus(null);
      setShowPrivacyNotice(false);
      setIsAcceptingPrivacy(false);
      setAnalysisReadyMessage(null);
      setAnalysisError(null);
      setAnalyzingPageId(null);
      setPendingAnalysisPageId(null);
      setPendingBatchAction(null);
      setPendingReanalysisDocumentId(null);
      setPendingFailedReanalysisDocumentId(null);
      setIsBatchAnalyzing(false);
      setReanalyzingDocumentId(null);
      setReanalyzingFailedDocumentId(null);
      setDeletingDocumentId(null);
      setIsExporting(false);
      setExportError(null);
      setExportResult(null);
      return;
    }

    setShowPrivacyNotice(false);
    setIsAcceptingPrivacy(false);
    setAnalysisReadyMessage(null);
    setAnalysisError(null);
    setAnalyzingPageId(null);
    setPendingAnalysisPageId(null);
    setPendingBatchAction(null);
    setPendingReanalysisDocumentId(null);
    setPendingFailedReanalysisDocumentId(null);
    setIsBatchAnalyzing(false);
    setReanalyzingDocumentId(null);
    setReanalyzingFailedDocumentId(null);
    setDeletingDocumentId(null);
    setIsExporting(false);
    setExportError(null);
    setExportResult(null);
    void refreshJobs({
      recoverInterrupted: recoveredWorkspaceRef.current !== workspaceKey,
    });
    void refreshDocuments();
    void (async () => {
      setIsModelStatusLoading(true);
      try {
        const status = await tauriClient.getModelConfigurationStatus();
        if (!cancelled) setModelStatus(status);
      } catch {
        if (!cancelled) setModelStatus(null);
      } finally {
        if (!cancelled) setIsModelStatusLoading(false);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [workspaceReady, workspaceKey]);

  useEffect(() => {
    if (!workspaceReady || !isActive) {
      return;
    }
    let cancelled = false;
    void refreshJobs();
    void refreshDocuments();
    void (async () => {
      setIsModelStatusLoading(true);
      try {
        const status = await tauriClient.getModelConfigurationStatus();
        if (!cancelled) setModelStatus(status);
      } catch {
        if (!cancelled) setModelStatus(null);
      } finally {
        if (!cancelled) setIsModelStatusLoading(false);
      }
    })();
    return () => { cancelled = true; };
  }, [workspaceReady, isActive]);

  const hasRunningAnalysis = useMemo(() => {
    const hasRunningAnalysisJob = jobs.some(
      (job) => job.status === "running" && isAnalysisJobType(job.job_type),
    );
    const hasLocalAnalysis =
      isBatchAnalyzing ||
      analyzingPageId !== null ||
      reanalyzingDocumentId !== null ||
      reanalyzingFailedDocumentId !== null;
    return hasRunningAnalysisJob || hasLocalAnalysis;
  }, [
    jobs,
    isBatchAnalyzing,
    analyzingPageId,
    reanalyzingDocumentId,
    reanalyzingFailedDocumentId,
  ]);

  useEffect(() => {
    if (!workspaceReady || !isActive || !hasRunningAnalysis) {
      return;
    }
    const timer = window.setInterval(() => {
      void refreshJobs();
      void refreshDocuments();
    }, 2000);
    return () => window.clearInterval(timer);
  }, [workspaceReady, isActive, hasRunningAnalysis]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    try {
      void getCurrentWindow()
        .onDragDropEvent((event) => {
          const canAcceptDrop =
            workspaceReadyRef.current && isActiveRef.current && !importLockRef.current;
          if (event.payload.type === "enter" || event.payload.type === "over") {
            setIsDragActive(canAcceptDrop);
            return;
          }
          if (event.payload.type === "leave") {
            setIsDragActive(false);
            return;
          }
          setIsDragActive(false);
          if (event.payload.type !== "drop") {
            return;
          }
          if (!canAcceptDrop) {
            if (importLockRef.current) {
              showImportBusyDropResult();
            }
            return;
          }
          const paths = Array.isArray(event.payload.paths) ? event.payload.paths : [];
          if (paths.length > 0) {
            void importDocumentFilesRef.current(paths);
          }
        })
        .then((nextUnlisten) => {
          if (cancelled) {
            nextUnlisten();
          } else {
            unlisten = nextUnlisten;
          }
        })
        .catch(() => {
          unlisten = null;
        });
    } catch {
      unlisten = null;
    }
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if ((!workspaceReady || !isActive || isImporting) && isDragActive) {
      setIsDragActive(false);
    }
  }, [workspaceReady, isActive, isImporting, isDragActive]);

  const analysisStats = useMemo(
    () => computeAnalysisStats(documents, pagesByDocument),
    [documents, pagesByDocument],
  );
  const workbenchStats = useMemo(
    () => computeWorkbenchStats(documents, pagesByDocument, jobs),
    [documents, pagesByDocument, jobs],
  );
  const analysisConfigured =
    modelStatus?.configured &&
    (!modelStatus.requires_privacy_notice || modelStatus.privacy_notice_accepted);

  return (
    <div
      className="page-grid workbench-page"
      data-drag-active={isDragActive}
      onDragEnter={handleDragEnter}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={(event) => void handleDrop(event)}
    >
      <section className="panel panel-wide workbench-overview-panel">
        <div className="panel-header">
          <div>
            <p className="eyebrow">工作台</p>
            <h2>把本地文档变成页面资产</h2>
            <p className="muted-copy">
              导入 PDF 或 Office 文档后，SLICER 会在本地生成逐页图片，并保留分析、索引和搜索入口。
            </p>
          </div>
          <StatusBadge tone={workspaceReady ? "success" : "warning"}>
            {workspaceReady ? "工作区可用" : "尚未选择工作区"}
          </StatusBadge>
        </div>
        <div className="workbench-empty-block">
          <WorkspacePicker
            status={workspaceStatus}
            isLoading={isWorkspaceLoading}
            onChooseWorkspace={onChooseWorkspace}
          />
        </div>
        {workspaceReady ? null : (
          <EmptyState
            title="等待工作区"
            description="导入、转换、分析和索引任务会在工作区可用后接入。"
          />
        )}
      </section>

      {workspaceReady ? (
        <>
          <section className="panel panel-wide import-drop-panel">
            <div className="import-drop-zone" data-active={isDragActive}>
              <div className="import-drop-copy">
                <p className="eyebrow">批量导入</p>
                <h2>{isDragActive ? "松开即可导入文档" : "拖入文档，自动开始转换"}</h2>
                <p className="muted-copy">
                  支持 PDF、DOC、DOCX、PPT、PPTX。页面图片生成后会立即出现在下方资产列表。
                </p>
              </div>
              <div className="action-row workbench-actions">
                <Button
                  variant="primary"
                  onClick={() => void handleImportDocuments()}
                  disabled={isImporting}
                >
                  {isImporting ? "导入中..." : "选择文件"}
                </Button>
              </div>
            </div>
            {importError ? (
              <p className="job-error">导入失败：{importError.message}</p>
            ) : null}
            <ImportResultList results={importResults} />
          </section>

          <section className="panel panel-wide workbench-summary-panel" aria-label="工作台资产摘要">
            <div className="workbench-summary-grid">
              <WorkbenchMetric label="文档" value={workbenchStats.documentCount} helper="已进入工作区" />
              <WorkbenchMetric label="页面图片" value={workbenchStats.generatedPages} helper={`共 ${workbenchStats.totalPages} 页`} />
              <WorkbenchMetric label="待分析" value={analysisStats.pendingPages} helper="页面可查看" />
              <WorkbenchMetric label="失败" value={workbenchStats.failureCount} helper="可按项处理" tone={workbenchStats.failureCount > 0 ? "danger" : "neutral"} />
              <WorkbenchMetric label="处理中" value={workbenchStats.runningJobs} helper="任务队列" />
            </div>
          </section>

          <DocumentList
            documents={documents}
            pagesByDocument={pagesByDocument}
            jobs={jobs}
            isLoading={isDocsLoading}
            workspacePath={workspaceStatus.workspace_path}
            onRetry={(id) => void handleRetryImport(id)}
            onAnalyzePage={(pageId) => void handleAnalyzePage(pageId)}
            onReanalyzeDocument={(documentId) => void handleReanalyzeDocument(documentId)}
            onReanalyzeFailedPages={(documentId) => void handleReanalyzeFailedPages(documentId)}
            onOpenSourceFile={(path) => void handleOpenSourceFile(path)}
            onOpenDocumentImage={(page) => void handleOpenDocumentImage(page)}
            onDeleteDocument={(documentId) => void handleDeleteDocument(documentId)}
            analyzingPageId={analyzingPageId}
            reanalyzingDocumentId={reanalyzingDocumentId}
            reanalyzingFailedDocumentId={reanalyzingFailedDocumentId}
            deletingDocumentId={deletingDocumentId}
          />

          <AnalysisJobList jobs={jobs} />

          <JobList
            jobs={jobs}
            isLoading={isJobsLoading}
            isCreatingDemo={isCreatingDemo}
            errorMessage={jobsError}
            onCreateDemoJob={handleCreateDemoJob}
            onRefresh={() => void refreshJobs()}
          />

          <section className="panel panel-wide">
            <div className="panel-header">
              <div>
                <p className="eyebrow">页面分析</p>
                <h2>模型分析</h2>
                <p className="muted-copy">
                  配置模型后可批量分析新页面；待分析 {analysisStats.pendingPages} 页
                  {analysisStats.documentsWithPending > 0
                    ? `（${analysisStats.documentsWithPending} 个文档仍有待分析页面）`
                    : ""}
                  ，已分析 {analysisStats.analyzedPages} 页
                  {analysisStats.failedPages > 0
                    ? `，失败 ${analysisStats.failedPages} 页`
                    : ""}
                  。
                  {!analysisConfigured && modelStatus?.configured
                    ? " 请先确认隐私提示。"
                    : ""}
                </p>
              </div>
              <StatusBadge
                tone={
                  modelStatus?.configured
                    ? modelStatus.privacy_notice_accepted ||
                      !modelStatus.requires_privacy_notice
                      ? "success"
                      : "warning"
                    : "neutral"
                }
              >
                {isModelStatusLoading
                  ? "检查中"
                  : modelStatus?.configured
                    ? "已配置"
                    : "未配置"}
              </StatusBadge>
            </div>
            {!modelStatus?.configured ? (
              <p className="muted-copy">
                缺少：{formatMissingFields(modelStatus?.missing ?? [])}。
                <button type="button" className="link-button" onClick={onOpenSettings}>
                  前往设置
                </button>
              </p>
            ) : null}
            {analysisReadyMessage ? (
              <p className="muted-copy">{analysisReadyMessage}</p>
            ) : null}
            {analysisError ? (
              <ErrorMessage
                title="分析失败"
                message={analysisError.message}
                correlationId={analysisError.correlationId}
              />
            ) : null}
            <div className="action-row workbench-actions">
              <Button
                variant="primary"
                onClick={() => void handleAnalysisEntry()}
                disabled={
                  isModelStatusLoading ||
                  isBatchAnalyzing ||
                  !analysisConfigured
                }
              >
                {!modelStatus?.configured
                  ? "完成模型配置"
                  : isBatchAnalyzing
                    ? "批量分析中..."
                    : analysisStats.pendingPages > 0
                      ? `分析新页面（${analysisStats.pendingPages}）`
                      : "分析新页面"}
              </Button>
              {!modelStatus?.configured ? (
                <Button onClick={onOpenSettings}>打开设置</Button>
              ) : null}
            </div>
          </section>

          <section className="panel panel-wide">
            <div className="panel-header">
              <div>
                <p className="eyebrow">数据导出</p>
                <h2>一键导出</h2>
                <p className="muted-copy">
                  将所有已分析页面导出为 Markdown 文件和媒体资源包，可直接用于 Obsidian 等工具。
                </p>
              </div>
            </div>
            {exportError ? (
              <ErrorMessage
                title="导出失败"
                message={exportError.message}
                correlationId={exportError.correlationId}
              />
            ) : null}
            {exportResult ? (
              <p className="muted-copy">
                导出完成：{exportResult.document_count} 个文档，{exportResult.media_count} 个媒体文件。
                Markdown 文件：{exportResult.markdown_path}
              </p>
            ) : null}
            <div className="action-row workbench-actions">
              <Button
                variant="primary"
                onClick={() => void handleExport()}
                disabled={isExporting}
              >
                {isExporting ? "导出中..." : "一键导出"}
              </Button>
            </div>
          </section>

          <IndexStatusPanel workspaceReady={workspaceReady} isActive={isActive} />
        </>
      ) : null}

      <PrivacyNotice
        open={showPrivacyNotice}
        onConfirm={() => void handlePrivacyConfirm()}
        onCancel={() => {
          setShowPrivacyNotice(false);
          setPendingAnalysisPageId(null);
          setPendingBatchAction(null);
          setPendingReanalysisDocumentId(null);
          setPendingFailedReanalysisDocumentId(null);
        }}
        isSubmitting={isAcceptingPrivacy}
      />
    </div>
  );
}

function formatMissingFields(missing: string[]): string {
  const labels: Record<string, string> = {
    model_provider: "Provider",
    model_name: "Model Name",
    endpoint: "Base URL 或自定义 Endpoint",
    api_key: "API Key",
  };
  return missing.map((key) => labels[key] ?? key).join("、") || "模型配置";
}

function WorkbenchMetric({
  label,
  value,
  helper,
  tone = "neutral",
}: {
  label: string;
  value: number;
  helper: string;
  tone?: "neutral" | "danger";
}) {
  return (
    <div className="workbench-metric" data-tone={tone}>
      <span className="workbench-metric-label">{label}</span>
      <strong className="workbench-metric-value">{value}</strong>
      <span className="workbench-metric-helper">{helper}</span>
    </div>
  );
}

function computeAnalysisStats(
  documents: DocumentDto[],
  pagesByDocument: Record<string, PageWorkbenchDto[]>,
) {
  let pendingPages = 0;
  let analyzedPages = 0;
  let failedPages = 0;
  let documentsWithPending = 0;

  for (const doc of documents) {
    const pages = pagesByDocument[doc.document_id] ?? [];
    let docPending = 0;
    for (const page of pages) {
      if (page.status === "rendered") {
        pendingPages += 1;
        docPending += 1;
      } else if (page.status === "analyzed") {
        analyzedPages += 1;
      } else if (page.status === "failed") {
        failedPages += 1;
      }
    }
    if (docPending > 0) {
      documentsWithPending += 1;
    }
  }

  return {
    pendingPages,
    analyzedPages,
    failedPages,
    documentsWithPending,
  };
}

function computeWorkbenchStats(
  documents: DocumentDto[],
  pagesByDocument: Record<string, PageWorkbenchDto[]>,
  jobs: JobDto[],
) {
  let totalPages = 0;
  let generatedPages = 0;
  let failedPages = 0;

  for (const doc of documents) {
    const pages = pagesByDocument[doc.document_id] ?? [];
    totalPages += doc.page_count ?? pages.length;
    let documentGeneratedPages = 0;
    let documentFailedPages = 0;
    for (const page of pages) {
      if (page.image_path) {
        documentGeneratedPages += 1;
      }
      if (page.status === "failed") {
        documentFailedPages += 1;
      }
    }
    generatedPages += documentGeneratedPages;
    failedPages += Math.max(documentFailedPages, doc.analysis_failed_pages);
  }

  const failedDocuments = documents.filter((doc) => doc.status === "failed").length;
  const runningJobs = jobs.filter(
    (job) => job.status === "queued" || job.status === "running",
  ).length;

  return {
    documentCount: documents.length,
    totalPages,
    generatedPages,
    failureCount: failedDocuments + failedPages,
    runningJobs,
  };
}

function formatBatchMessage(prefix: string, result: AnalysisBatchResultDto) {
  return `${prefix}：共 ${result.total_pages} 页，成功 ${result.succeeded_pages} 页，失败 ${result.failed_pages} 页，跳过 ${result.skipped_pages} 页。`;
}

function resolveWorkspacePath(
  relativePath: string | null | undefined,
  workspacePath: string | null | undefined,
) {
  if (!relativePath || !workspacePath) {
    return null;
  }

  const normalized = relativePath.replace(/\\/g, "/");
  if (
    normalized.startsWith("/") ||
    /^[A-Za-z]:\//.test(normalized) ||
    normalized.split("/").includes("..")
  ) {
    return null;
  }

  const separator = workspacePath.includes("\\") ? "\\" : "/";
  const root = workspacePath.replace(/[\\/]+$/, "");
  return `${root}${separator}${normalized.replace(/\//g, separator)}`;
}

function normalizeImportPaths(filePaths: unknown[]) {
  const uniquePaths = new Set<string>();
  for (const filePath of filePaths) {
    if (typeof filePath !== "string") {
      continue;
    }
    const trimmedPath = filePath.trim();
    if (trimmedPath.length > 0) {
      uniquePaths.add(trimmedPath);
    }
  }
  return [...uniquePaths];
}

function getDroppedFilePath(file: File) {
  const fileWithPath = file as File & { path?: unknown };
  return typeof fileWithPath.path === "string" ? fileWithPath.path : null;
}

function extractError(error: unknown): { message: string; correlationId?: string | null } {
  if (typeof error === "object" && error !== null) {
    const e = error as Record<string, unknown>;
    const msg = typeof e.message === "string" ? e.message : null;
    const cid = typeof e.correlation_id === "string" ? e.correlation_id : null;
    if (msg) return { message: msg, correlationId: cid };
  }
  if (error instanceof Error) return { message: error.message };
  if (typeof error === "string") return { message: error };
  return { message: "任务命令调用失败，请稍后重试。" };
}
