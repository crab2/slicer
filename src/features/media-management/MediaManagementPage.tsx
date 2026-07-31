import { useEffect, useMemo, useRef, useState } from "react";
import { Button } from "../../components/common/Button";
import { EmptyState } from "../../components/common/EmptyState";
import { ErrorMessage } from "../../components/common/ErrorMessage";
import { StatusBadge } from "../../components/common/StatusBadge";
import { DocumentFormatViewer } from "../../components/document-viewer/DocumentFormatViewer";
import { tauriClient } from "../../lib/tauriClient";
import type { DocumentDto, JobDto, PageWorkbenchDto, WorkspaceStatusDto } from "../../types/app";
import {
  countDocumentAnalysisFailures,
  MediaAssetList,
  type MediaStatusFilter,
} from "./components/MediaAssetList";

const PAGE_REFRESH_CONCURRENCY = 16;

interface MediaManagementPageProps {
  workspaceStatus: WorkspaceStatusDto;
  isWorkspaceLoading: boolean;
  isActive: boolean;
  onChooseWorkspace: () => void;
}

export function MediaManagementPage({
  workspaceStatus,
  isWorkspaceLoading,
  isActive,
  onChooseWorkspace,
}: MediaManagementPageProps) {
  const workspaceReady = workspaceStatus.status === "ready";
  const workspaceKey = workspaceStatus.workspace_path ?? "current";
  const docsGeneration = useRef(0);
  const jobsGeneration = useRef(0);
  const viewerRef = useRef<HTMLDivElement>(null);
  const [jobs, setJobs] = useState<JobDto[]>([]);
  const [documents, setDocuments] = useState<DocumentDto[]>([]);
  const [pagesByDocument, setPagesByDocument] = useState<Record<string, PageWorkbenchDto[]>>({});
  const [isDocsLoading, setIsDocsLoading] = useState(false);
  const [isJobsLoading, setIsJobsLoading] = useState(false);
  const [error, setError] = useState<{ message: string; correlationId?: string | null } | null>(null);
  const [query, setQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<MediaStatusFilter>("all");
  const [selectedDocumentId, setSelectedDocumentId] = useState<string | null>(null);
  const [deletingDocumentId, setDeletingDocumentId] = useState<string | null>(null);
  const [viewerRefreshKey, setViewerRefreshKey] = useState(0);

  const selectedDocument = useMemo(
    () => documents.find((document) => document.document_id === selectedDocumentId) ?? null,
    [documents, selectedDocumentId],
  );
  const stats = useMemo(
    () => computeMediaStats(documents, pagesByDocument),
    [documents, pagesByDocument],
  );

  async function refreshJobs() {
    if (!workspaceReady) {
      setJobs([]);
      return;
    }
    const generation = ++jobsGeneration.current;
    setIsJobsLoading(true);
    try {
      const result = await tauriClient.listJobs();
      if (generation === jobsGeneration.current) setJobs(result);
    } catch (nextError) {
      if (generation === jobsGeneration.current) setError(extractError(nextError));
    } finally {
      if (generation === jobsGeneration.current) setIsJobsLoading(false);
    }
  }

  async function refreshDocuments() {
    if (!workspaceReady) {
      setDocuments([]);
      setPagesByDocument({});
      return;
    }
    const generation = ++docsGeneration.current;
    setIsDocsLoading(true);
    setError(null);
    try {
      const nextDocuments = await tauriClient.listDocuments();
      const pageEntries: Array<readonly [string, PageWorkbenchDto[]]> = [];
      for (let offset = 0; offset < nextDocuments.length; offset += PAGE_REFRESH_CONCURRENCY) {
        const batch = await Promise.all(
          nextDocuments.slice(offset, offset + PAGE_REFRESH_CONCURRENCY).map(async (document) => {
          try {
            return [document.document_id, await tauriClient.listWorkbenchPages(document.document_id)] as const;
          } catch {
            return [document.document_id, [] as PageWorkbenchDto[]] as const;
          }
          }),
        );
        if (generation !== docsGeneration.current) return;
        pageEntries.push(...batch);
      }
      setDocuments(nextDocuments);
      setPagesByDocument(Object.fromEntries(pageEntries));
    } catch (nextError) {
      if (generation === docsGeneration.current) {
        setDocuments([]);
        setPagesByDocument({});
        setError(extractError(nextError));
      }
    } finally {
      if (generation === docsGeneration.current) setIsDocsLoading(false);
    }
  }

  async function refreshAll() {
    await Promise.all([refreshDocuments(), refreshJobs()]);
    setViewerRefreshKey((current) => current + 1);
  }

  async function handleRetryImport(documentId: string) {
    setError(null);
    try {
      await tauriClient.retryImport(documentId);
    } catch (nextError) {
      setError(extractError(nextError));
    } finally {
      await refreshAll();
    }
  }

  async function handleDeleteDocument(documentId: string) {
    setDeletingDocumentId(documentId);
    setError(null);
    try {
      await tauriClient.deleteDocument(documentId);
      setSelectedDocumentId((current) => (current === documentId ? null : current));
    } catch (nextError) {
      setError(extractError(nextError));
    } finally {
      setDeletingDocumentId(null);
      await refreshAll();
    }
  }

  function handleViewDocument(document: DocumentDto) {
    setSelectedDocumentId(document.document_id);
    window.requestAnimationFrame(() => {
      viewerRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
    });
  }

  useEffect(() => {
    ++docsGeneration.current;
    ++jobsGeneration.current;
    setError(null);
    setSelectedDocumentId(null);
    setDeletingDocumentId(null);
    if (!workspaceReady) {
      setDocuments([]);
      setPagesByDocument({});
      setJobs([]);
      return;
    }
    void refreshAll();
  }, [workspaceReady, workspaceKey]);

  useEffect(() => {
    if (workspaceReady && isActive) void refreshAll();
  }, [workspaceReady, isActive]);

  if (!workspaceReady) {
    return (
      <div className="page-grid media-management-page">
        <section className="panel panel-wide">
          <div className="panel-header">
            <div>
              <p className="eyebrow">媒体管理</p>
              <h2>选择工作区后查看媒体资产</h2>
            </div>
            <StatusBadge tone="warning">尚未选择工作区</StatusBadge>
          </div>
          <Button variant="primary" onClick={onChooseWorkspace} disabled={isWorkspaceLoading}>
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
            <h2>媒体资产</h2>
          </div>
          <StatusBadge tone={isDocsLoading || isJobsLoading ? "warning" : "success"}>
            {isDocsLoading || isJobsLoading ? "刷新中" : "已同步"}
          </StatusBadge>
        </div>
        <div className="workbench-summary-grid media-management-metrics">
          <Metric label="媒体" value={stats.documentCount} helper="本地账本" />
          <Metric label="页面" value={stats.totalPages} helper="已导入" />
          <Metric label="已分析" value={stats.analyzedPages} helper="页面" />
          <Metric label="失败" value={stats.failureCount} helper="导入或分析" tone={stats.failureCount > 0 ? "danger" : "neutral"} />
        </div>
        <div className="media-management-actions">
          <Button onClick={() => void refreshAll()} disabled={isDocsLoading || isJobsLoading}>刷新</Button>
        </div>
        {error ? <ErrorMessage title="媒体管理" message={error.message} correlationId={error.correlationId} /> : null}
      </section>

      {documents.length === 0 && !isDocsLoading ? (
        <EmptyState title="还没有媒体资产" description="请先在媒体导入中添加文档。" />
      ) : null}

      <div className="panel-wide">
        <MediaAssetList
          documents={documents}
          pagesByDocument={pagesByDocument}
          jobs={jobs}
          isLoading={isDocsLoading}
          query={query}
          statusFilter={statusFilter}
          onQueryChange={setQuery}
          onStatusFilterChange={setStatusFilter}
          onViewDocument={handleViewDocument}
          onRetry={(documentId) => void handleRetryImport(documentId)}
          onDeleteDocument={(documentId) => void handleDeleteDocument(documentId)}
          deletingDocumentId={deletingDocumentId}
        />
      </div>

      <div className="panel-wide media-document-viewer" ref={viewerRef}>
        <DocumentFormatViewer
          documentId={selectedDocument?.document_id ?? null}
          documentTitle={selectedDocument?.original_filename}
          refreshKey={viewerRefreshKey}
        />
      </div>
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
) {
  let totalPages = 0;
  let failedAnalysisItems = 0;
  for (const document of documents) {
    const pages = pagesByDocument[document.document_id] ?? [];
    totalPages += document.page_count ?? pages.length;
    failedAnalysisItems += countDocumentAnalysisFailures(document, pages);
  }
  return {
    documentCount: documents.length,
    totalPages,
    analyzedPages: documents.reduce((sum, document) => sum + document.analysis_succeeded_pages, 0),
    failureCount:
      documents.filter((document) => document.status === "failed").length + failedAnalysisItems,
  };
}

function extractError(error: unknown): { message: string; correlationId?: string | null } {
  if (typeof error === "object" && error !== null) {
    const value = error as Record<string, unknown>;
    const message = typeof value.message === "string" ? value.message : null;
    if (message) {
      return {
        message,
        correlationId: typeof value.correlation_id === "string" ? value.correlation_id : null,
      };
    }
  }
  if (error instanceof Error) return { message: error.message };
  return { message: typeof error === "string" ? error : "媒体管理命令调用失败。" };
}
