import { useEffect, useMemo, useState } from "react";
import { Button } from "../../../components/common/Button";
import { StatusBadge } from "../../../components/common/StatusBadge";
import type { DocumentDto, JobDto, PageWorkbenchDto } from "../../../types/app";

const PAGE_SIZE = 8;

export type MediaStatusFilter =
  | "all"
  | "ready"
  | "failed"
  | "has_failed_pages"
  | "needs_analysis";

interface MediaAssetListProps {
  documents: DocumentDto[];
  pagesByDocument: Record<string, PageWorkbenchDto[]>;
  jobs: JobDto[];
  isLoading: boolean;
  query: string;
  statusFilter: MediaStatusFilter;
  onQueryChange: (query: string) => void;
  onStatusFilterChange: (filter: MediaStatusFilter) => void;
  onViewDocument: (document: DocumentDto) => void;
  onRetry?: (documentId: string) => void;
  onDeleteDocument?: (documentId: string) => void;
  deletingDocumentId?: string | null;
}

export function MediaAssetList({
  documents,
  pagesByDocument,
  jobs,
  isLoading,
  query,
  statusFilter,
  onQueryChange,
  onStatusFilterChange,
  onViewDocument,
  onRetry,
  onDeleteDocument,
  deletingDocumentId,
}: MediaAssetListProps) {
  const [page, setPage] = useState(1);
  const jobsById = useMemo(() => new Map(jobs.map((job) => [job.job_id, job])), [jobs]);
  const filteredDocuments = useMemo(
    () => filterDocuments(documents, pagesByDocument, query, statusFilter),
    [documents, pagesByDocument, query, statusFilter],
  );
  const pageCount = Math.max(1, Math.ceil(filteredDocuments.length / PAGE_SIZE));
  const currentPage = Math.min(page, pageCount);
  const visibleDocuments = filteredDocuments.slice(
    (currentPage - 1) * PAGE_SIZE,
    currentPage * PAGE_SIZE,
  );

  useEffect(() => setPage(1), [query, statusFilter]);
  useEffect(() => setPage((current) => Math.min(current, pageCount)), [pageCount]);

  if (isLoading) {
    return <p className="muted-copy">媒体资产加载中...</p>;
  }
  if (documents.length === 0) {
    return null;
  }

  return (
    <div className="media-asset-list">
      <div className="document-list-panel media-management-panel">
        <div className="document-list-toolbar media-management-toolbar">
          <label className="document-search-field">
            <span>搜索媒体</span>
            <input
              value={query}
              onChange={(event) => onQueryChange(event.target.value)}
              placeholder="按文件名、路径、类型或状态搜索"
            />
          </label>
          <label className="media-filter-field">
            <span>状态</span>
            <select
              value={statusFilter}
              onChange={(event) => onStatusFilterChange(event.target.value as MediaStatusFilter)}
            >
              <option value="all">全部</option>
              <option value="ready">已完成</option>
              <option value="needs_analysis">待分析</option>
              <option value="has_failed_pages">有失败项</option>
              <option value="failed">导入失败</option>
            </select>
          </label>
          <p className="document-list-count">
            {filteredDocuments.length} / {documents.length} 个媒体
          </p>
        </div>

        {filteredDocuments.length === 0 ? (
          <p className="document-empty-result">没有匹配的媒体。</p>
        ) : (
          <>
            <div className="document-asset-list" role="list" aria-label="媒体资产列表">
              {visibleDocuments.map((document) => (
                <MediaDocumentRow
                  key={document.document_id}
                  document={document}
                  pages={pagesByDocument[document.document_id] ?? []}
                  job={document.job_id ? jobsById.get(document.job_id) : null}
                  onViewDocument={onViewDocument}
                  onRetry={onRetry}
                  onDeleteDocument={onDeleteDocument}
                  deletingDocumentId={deletingDocumentId}
                />
              ))}
            </div>
            <DocumentPagination
              page={currentPage}
              pageCount={pageCount}
              totalItems={filteredDocuments.length}
              onPageChange={setPage}
            />
          </>
        )}
      </div>
    </div>
  );
}

function MediaDocumentRow({
  document,
  pages,
  job,
  onViewDocument,
  onRetry,
  onDeleteDocument,
  deletingDocumentId,
}: {
  document: DocumentDto;
  pages: PageWorkbenchDto[];
  job?: JobDto | null;
  onViewDocument: (document: DocumentDto) => void;
  onRetry?: (documentId: string) => void;
  onDeleteDocument?: (documentId: string) => void;
  deletingDocumentId?: string | null;
}) {
  const isFailed = document.status === "failed";
  const isDeleting = deletingDocumentId === document.document_id;
  const failedItems = countDocumentAnalysisFailures(document, pages);
  const importingJob = document.status === "importing" ? job : null;

  return (
    <article className="media-asset-row-simple" role="listitem" aria-label={document.original_filename}>
      <div className="document-asset-main">
        <div className="document-asset-header">
          <div className="document-name-cell">
            <span className="document-type-label">{document.file_type.toUpperCase()}</span>
            <span className="document-name-text" title={document.original_filename}>
              {document.original_filename}
            </span>
            <span className="document-path-text" title={document.original_path}>
              {document.original_path}
            </span>
          </div>
          <div className="document-status-stack">
            <StatusBadge tone={statusTone(document.status)}>{statusLabel(document.status)}</StatusBadge>
            <span className="document-date-cell">更新于 {formatDateTime(document.updated_at)}</span>
          </div>
        </div>
        <div className="document-asset-stats">
          <span>{document.page_count ?? pages.length} 页</span>
          <span>{document.analysis_succeeded_pages} 页已分析</span>
          {failedItems > 0 ? <span className="doc-summary-failed">{failedItems} 项失败</span> : null}
        </div>
        {importingJob ? (
          <div className="document-inline-progress" aria-label={`导入进度 ${boundedProgress(importingJob.progress)}%`}>
            <span className="progress-fill" style={{ width: `${boundedProgress(importingJob.progress)}%` }} />
          </div>
        ) : null}
        {document.error_summary ? <p className="job-error">{document.error_summary}</p> : null}
      </div>

      <div className="document-row-actions media-row-primary-actions">
        <Button variant="primary" onClick={() => onViewDocument(document)}>
          查看文档
        </Button>
        {isFailed && onRetry ? (
          <Button onClick={() => onRetry(document.document_id)}>重试导入</Button>
        ) : null}
        {onDeleteDocument ? (
          <Button
            className="document-row-button-danger"
            onClick={() => {
              if (window.confirm(`确定删除文档“${document.original_filename}”吗？`)) {
                onDeleteDocument(document.document_id);
              }
            }}
            disabled={isDeleting}
          >
            {isDeleting ? "删除中" : "删除"}
          </Button>
        ) : null}
      </div>
    </article>
  );
}

function DocumentPagination({
  page,
  pageCount,
  totalItems,
  onPageChange,
}: {
  page: number;
  pageCount: number;
  totalItems: number;
  onPageChange: (page: number) => void;
}) {
  if (totalItems <= PAGE_SIZE) return null;
  const first = (page - 1) * PAGE_SIZE + 1;
  const last = Math.min(totalItems, page * PAGE_SIZE);
  return (
    <div className="document-pagination">
      <p className="document-list-count">{first}-{last} / {totalItems}</p>
      <div className="document-pagination-actions">
        <Button onClick={() => onPageChange(page - 1)} disabled={page <= 1}>上一页</Button>
        <span className="document-page-indicator">{page} / {pageCount}</span>
        <Button onClick={() => onPageChange(page + 1)} disabled={page >= pageCount}>下一页</Button>
      </div>
    </div>
  );
}

export function filterDocuments(
  documents: DocumentDto[],
  pagesByDocument: Record<string, PageWorkbenchDto[]>,
  query: string,
  statusFilter: MediaStatusFilter,
) {
  const normalized = query.trim().toLocaleLowerCase("zh-CN");
  return documents.filter((document) => {
    const pages = pagesByDocument[document.document_id] ?? [];
    const matchesQuery =
      !normalized ||
      [
        document.original_filename,
        document.original_path,
        document.file_type,
        statusLabel(document.status),
        document.status,
      ]
        .join(" ")
        .toLocaleLowerCase("zh-CN")
        .includes(normalized);
    if (!matchesQuery) return false;
    switch (statusFilter) {
      case "ready":
        return document.status === "ready";
      case "failed":
        return document.status === "failed";
      case "has_failed_pages":
        return (
          document.analysis_failed_pages > 0 ||
          pages.some(
            (page) =>
              page.status === "failed" || countValue(page.failed_visual_module_count) > 0,
          )
        );
      case "needs_analysis":
        return pages.some(
          (page) =>
            page.status === "rendered" || countValue(page.pending_visual_module_count) > 0,
        );
      default:
        return true;
    }
  });
}

export function countDocumentAnalysisFailures(
  document: DocumentDto,
  pages: PageWorkbenchDto[],
) {
  const failedPageCount = Math.max(
    document.analysis_failed_pages,
    pages.filter((page) => page.status === "failed").length,
  );
  const failedVisualModuleCount = pages.reduce(
    (sum, page) => sum + countValue(page.failed_visual_module_count),
    0,
  );
  return failedPageCount + failedVisualModuleCount;
}

function boundedProgress(progress: number) {
  return Number.isFinite(progress) ? Math.min(100, Math.max(0, Math.round(progress))) : 0;
}

function countValue(value: number | null | undefined) {
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

function statusLabel(status: string) {
  const labels: Record<string, string> = {
    ready: "已完成",
    importing: "导入中",
    failed: "已失败",
    pending: "等待中",
  };
  return labels[status] ?? status;
}

function statusTone(status: string): "success" | "warning" | "neutral" | "danger" {
  if (status === "ready") return "success";
  if (status === "failed") return "danger";
  if (status === "importing") return "warning";
  return "neutral";
}

function formatDateTime(value: string) {
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return value;
  return new Intl.DateTimeFormat("zh-CN", { dateStyle: "short", timeStyle: "medium" }).format(parsed);
}
