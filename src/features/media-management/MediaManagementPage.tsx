import { useEffect, useMemo, useRef, useState } from "react";
import { Button } from "../../components/common/Button";
import { EmptyState } from "../../components/common/EmptyState";
import { ErrorMessage } from "../../components/common/ErrorMessage";
import { StatusBadge } from "../../components/common/StatusBadge";
import { tauriClient } from "../../lib/tauriClient";
import type { DocumentDto, JobDto, PageWorkbenchDto, WorkspaceStatusDto } from "../../types/app";
import type { NavigationContext, ReanalysisNavigationContext, ViewId } from "../../app/navigation";
import {
  MediaAssetList,
  type MediaAssetSelection,
  type MediaStatusFilter,
} from "./components/MediaAssetList";

interface MediaManagementPageProps {
  workspaceStatus: WorkspaceStatusDto;
  isWorkspaceLoading: boolean;
  isActive: boolean;
  navigationContext: NavigationContext | null;
  onChooseWorkspace: () => void;
  onNavigateWithContext: (view: ViewId, context: NavigationContext) => void;
  onClearNavigationContext: () => void;
}

export function MediaManagementPage({
  workspaceStatus,
  isWorkspaceLoading,
  isActive,
  navigationContext,
  onChooseWorkspace,
  onNavigateWithContext,
  onClearNavigationContext,
}: MediaManagementPageProps) {
  const workspaceReady = workspaceStatus.status === "ready";
  const workspaceKey = workspaceStatus.workspace_path ?? "current";
  const docsGenRef = useRef(0);
  const jobsGenRef = useRef(0);
  const [jobs, setJobs] = useState<JobDto[]>([]);
  const [documents, setDocuments] = useState<DocumentDto[]>([]);
  const [pagesByDocument, setPagesByDocument] = useState<Record<string, PageWorkbenchDto[]>>({});
  const [isDocsLoading, setIsDocsLoading] = useState(false);
  const [isJobsLoading, setIsJobsLoading] = useState(false);
  const [error, setError] = useState<{ message: string; correlationId?: string | null } | null>(null);
  const [query, setQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<MediaStatusFilter>("all");
  const [selectedDocumentIds, setSelectedDocumentIds] = useState<string[]>([]);
  const [deletingDocumentId, setDeletingDocumentId] = useState<string | null>(null);
  const [returnNotice, setReturnNotice] = useState<string | null>(null);

  const validSelectedDocumentIds = useMemo(() => {
    const documentIds = new Set(documents.map((doc) => doc.document_id));
    return selectedDocumentIds.filter((id) => documentIds.has(id));
  }, [documents, selectedDocumentIds]);

  const managementStats = useMemo(
    () => computeMediaStats(documents, pagesByDocument, jobs),
    [documents, pagesByDocument, jobs],
  );

  async function refreshJobs() {
    if (!workspaceReady) {
      setJobs([]);
      return;
    }
    const gen = ++jobsGenRef.current;
    setIsJobsLoading(true);
    try {
      const result = await tauriClient.listJobs();
      if (gen === jobsGenRef.current) {
        setJobs(result);
      }
    } catch (err) {
      if (gen === jobsGenRef.current) {
        setError(extractError(err));
      }
    } finally {
      if (gen === jobsGenRef.current) {
        setIsJobsLoading(false);
      }
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
    setError(null);
    try {
      const docs = await tauriClient.listDocuments();
      if (gen !== docsGenRef.current) {
        return;
      }
      const pagesMap: Record<string, PageWorkbenchDto[]> = {};
      for (const doc of docs) {
        try {
          pagesMap[doc.document_id] = await tauriClient.listWorkbenchPages(doc.document_id);
        } catch {
          pagesMap[doc.document_id] = [];
        }
        if (gen !== docsGenRef.current) {
          return;
        }
      }
      setDocuments(docs);
      setPagesByDocument(pagesMap);
    } catch (err) {
      if (gen === docsGenRef.current) {
        setDocuments([]);
        setPagesByDocument({});
        setError(extractError(err));
      }
    } finally {
      if (gen === docsGenRef.current) {
        setIsDocsLoading(false);
      }
    }
  }

  async function refreshAll() {
    await Promise.all([refreshDocuments(), refreshJobs()]);
  }

  async function handleRetryImport(documentId: string) {
    setError(null);
    try {
      await tauriClient.retryImport(documentId);
      await refreshAll();
    } catch (err) {
      setError(extractError(err));
      await refreshAll();
    }
  }

  async function handleOpenSourceFile(path: string) {
    try {
      await tauriClient.revealDocumentInFolder(path);
    } catch (err) {
      setError(extractError(err));
    }
  }

  async function handleOpenDocumentImage(page: PageWorkbenchDto) {
    const imagePath = resolveWorkspacePath(page.image_path, workspaceStatus.workspace_path);
    if (!imagePath) {
      setError({ message: "页面图片不可用，可能尚未生成或路径无效。" });
      return;
    }

    try {
      await tauriClient.revealDocumentInFolder(imagePath);
    } catch (err) {
      setError(extractError(err));
    }
  }

  async function handleDeleteDocument(documentId: string) {
    setDeletingDocumentId(documentId);
    setError(null);
    try {
      await tauriClient.deleteDocument(documentId);
      setSelectedDocumentIds((ids) => ids.filter((id) => id !== documentId));
      await refreshAll();
    } catch (err) {
      setError(extractError(err));
      await refreshAll();
    } finally {
      setDeletingDocumentId(null);
    }
  }

  function handleReanalysisRequest(selection: MediaAssetSelection) {
    if (selection.disabledReason) {
      setError({ message: selection.disabledReason });
      return;
    }

    const context: ReanalysisNavigationContext = {
      action: "reanalyze",
      source_tab: "mediaManagement",
      return_to: "mediaManagement",
      selected_kind:
        selection.kind === "page"
          ? selection.ids.length > 1
            ? "page_batch"
            : "page"
          : selection.ids.length > 1
            ? "document_batch"
            : "document",
      selected_ids: selection.ids,
      filter: statusFilter,
      query,
      scroll_anchor: selection.ids[0] ? `media-${selection.ids[0]}` : null,
      selection_count: selection.ids.length,
    };
    onNavigateWithContext("analysis", context);
  }

  function handleBatchReanalysis() {
    const ids = validSelectedDocumentIds.filter((id) => {
      const doc = documents.find((item) => item.document_id === id);
      if (!doc) {
        return false;
      }
      const pages = pagesByDocument[id] ?? [];
      return doc.status !== "failed" && pages.some((page) => page.image_path);
    });
    if (ids.length === 0) {
      setError({ message: "当前选择中没有可重分析的媒体。" });
      return;
    }
    handleReanalysisRequest({
      kind: "document",
      ids,
      label: `${ids.length} 个媒体`,
    });
  }

  useEffect(() => {
    ++docsGenRef.current;
    ++jobsGenRef.current;
    setError(null);
    setSelectedDocumentIds([]);
    setDeletingDocumentId(null);
    if (!workspaceReady) {
      setJobs([]);
      setDocuments([]);
      setPagesByDocument({});
      return;
    }
    void refreshAll();
  }, [workspaceReady, workspaceKey]);

  useEffect(() => {
    if (!workspaceReady || !isActive) {
      return;
    }
    void refreshAll();
  }, [workspaceReady, isActive]);

  useEffect(() => {
    if (!isActive || !navigationContext || navigationContext.return_to !== "mediaManagement") {
      return;
    }
    setQuery(navigationContext.query);
    setStatusFilter((navigationContext.filter as MediaStatusFilter) || "all");
    setSelectedDocumentIds(
      navigationContext.selected_kind.includes("document")
        ? navigationContext.selected_ids
        : [],
    );
    setReturnNotice("已返回媒体管理，并重新从后端刷新当前媒体状态。");
    void refreshAll();
    onClearNavigationContext();
  }, [isActive, navigationContext, onClearNavigationContext]);

  if (!workspaceReady) {
    return (
      <div className="page-grid media-management-page">
        <section className="panel panel-wide">
          <div className="panel-header">
            <div>
              <p className="eyebrow">媒体管理</p>
              <h2>选择工作区后查看媒体资产</h2>
              <p className="muted-copy">
                媒体列表、页面缩略图和状态都来自本地 service 与 SQLite 账本。
              </p>
            </div>
            <StatusBadge tone="warning">尚未选择工作区</StatusBadge>
          </div>
          <Button
            variant="primary"
            onClick={onChooseWorkspace}
            disabled={isWorkspaceLoading}
          >
            {isWorkspaceLoading ? "检查中..." : "选择工作区"}
          </Button>
        </section>
      </div>
    );
  }

  return (
    <div className="page-grid media-management-page">
      <section className="panel panel-wide media-management-summary">
        <div className="panel-header">
          <div>
            <p className="eyebrow">媒体管理</p>
            <h2>媒体资产、页面与重分析选择</h2>
            <p className="muted-copy">
              管理操作通过 typed client 调用后端 service；重分析只构建路由上下文并交给模型分析页执行。
            </p>
          </div>
          <StatusBadge tone={isDocsLoading || isJobsLoading ? "warning" : "success"}>
            {isDocsLoading || isJobsLoading ? "刷新中" : "账本数据"}
          </StatusBadge>
        </div>

        <div className="workbench-summary-grid">
          <Metric label="媒体" value={managementStats.documentCount} helper="来自账本" />
          <Metric label="页面" value={managementStats.totalPages} helper={`${managementStats.generatedPages} 页有图片`} />
          <Metric label="待分析" value={managementStats.pendingPages} helper="可进入模型分析" />
          <Metric
            label="失败"
            value={managementStats.failureCount}
            helper="导入或页面失败"
            tone={managementStats.failureCount > 0 ? "danger" : "neutral"}
          />
          <Metric label="选中" value={validSelectedDocumentIds.length} helper="批量重分析上下文" />
        </div>

        <div className="media-management-actions">
          <Button onClick={() => void refreshAll()} disabled={isDocsLoading || isJobsLoading}>
            刷新
          </Button>
          <Button
            variant="primary"
            onClick={handleBatchReanalysis}
            disabled={validSelectedDocumentIds.length === 0}
          >
            重分析选中项
          </Button>
        </div>

        {returnNotice ? <p className="muted-copy">{returnNotice}</p> : null}
        {error ? (
          <ErrorMessage
            title="媒体管理"
            message={error.message}
            correlationId={error.correlationId}
          />
        ) : null}
      </section>

      {documents.length === 0 && !isDocsLoading ? (
        <EmptyState
          title="还没有媒体资产"
          description="请先在媒体导入中提交图片或文档。"
        />
      ) : null}

      <MediaAssetList
        documents={documents}
        pagesByDocument={pagesByDocument}
        jobs={jobs}
        isLoading={isDocsLoading}
        workspacePath={workspaceStatus.workspace_path}
        query={query}
        statusFilter={statusFilter}
        selectedDocumentIds={validSelectedDocumentIds}
        onQueryChange={setQuery}
        onStatusFilterChange={setStatusFilter}
        onSelectionChange={setSelectedDocumentIds}
        onRetry={(id) => void handleRetryImport(id)}
        onOpenSourceFile={(path) => void handleOpenSourceFile(path)}
        onOpenDocumentImage={(page) => void handleOpenDocumentImage(page)}
        onDeleteDocument={(documentId) => void handleDeleteDocument(documentId)}
        onReanalysisRequest={handleReanalysisRequest}
        deletingDocumentId={deletingDocumentId}
      />
    </div>
  );
}

function Metric({
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

function computeMediaStats(
  documents: DocumentDto[],
  pagesByDocument: Record<string, PageWorkbenchDto[]>,
  jobs: JobDto[],
) {
  let totalPages = 0;
  let generatedPages = 0;
  let pendingPages = 0;
  let failedPages = 0;

  for (const doc of documents) {
    const pages = pagesByDocument[doc.document_id] ?? [];
    totalPages += doc.page_count ?? pages.length;
    for (const page of pages) {
      if (page.image_path) {
        generatedPages += 1;
      }
      if (page.status === "rendered") {
        pendingPages += 1;
      }
      if (page.status === "failed") {
        failedPages += 1;
      }
    }
  }

  const failedDocuments = documents.filter((doc) => doc.status === "failed").length;
  const runningJobs = jobs.filter(
    (job) => job.status === "queued" || job.status === "running",
  ).length;

  return {
    documentCount: documents.length,
    totalPages,
    generatedPages,
    pendingPages,
    failureCount: failedDocuments + failedPages,
    runningJobs,
  };
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

function extractError(error: unknown): { message: string; correlationId?: string | null } {
  if (typeof error === "object" && error !== null) {
    const e = error as Record<string, unknown>;
    const msg = typeof e.message === "string" ? e.message : null;
    const cid = typeof e.correlation_id === "string" ? e.correlation_id : null;
    if (msg) return { message: msg, correlationId: cid };
  }
  if (error instanceof Error) return { message: error.message };
  if (typeof error === "string") return { message: error };
  return { message: "媒体管理命令调用失败，请稍后重试。" };
}
