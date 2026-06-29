import { useEffect, useMemo, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Button } from "../../../components/common/Button";
import { StatusBadge } from "../../../components/common/StatusBadge";
import { tauriClient } from "../../../lib/tauriClient";
import type {
  DocumentDto,
  JobDto,
  PageAnalysisSummaryDto,
  PageWorkbenchDto,
} from "../../../types/app";

const PAGE_SIZE = 8;

export type MediaStatusFilter = "all" | "ready" | "failed" | "has_failed_pages" | "needs_analysis";

export interface MediaAssetSelection {
  kind: "document" | "page";
  ids: string[];
  label: string;
  disabledReason?: string | null;
}

interface MediaAssetListProps {
  documents: DocumentDto[];
  pagesByDocument: Record<string, PageWorkbenchDto[]>;
  jobs: JobDto[];
  isLoading: boolean;
  workspacePath?: string | null;
  query: string;
  statusFilter: MediaStatusFilter;
  selectedDocumentIds: string[];
  onQueryChange: (query: string) => void;
  onStatusFilterChange: (filter: MediaStatusFilter) => void;
  onSelectionChange: (documentIds: string[]) => void;
  onRetry?: (documentId: string) => void;
  onOpenSourceFile?: (path: string) => void;
  onOpenDocumentImage?: (page: PageWorkbenchDto) => void;
  onDeleteDocument?: (documentId: string) => void;
  onReanalysisRequest?: (selection: MediaAssetSelection) => void;
  deletingDocumentId?: string | null;
}

export function MediaAssetList({
  documents,
  pagesByDocument,
  jobs,
  isLoading,
  workspacePath,
  query,
  statusFilter,
  selectedDocumentIds,
  onQueryChange,
  onStatusFilterChange,
  onSelectionChange,
  onRetry,
  onOpenSourceFile,
  onOpenDocumentImage,
  onDeleteDocument,
  onReanalysisRequest,
  deletingDocumentId,
}: MediaAssetListProps) {
  const [page, setPage] = useState(1);
  const selectedSet = useMemo(() => new Set(selectedDocumentIds), [selectedDocumentIds]);
  const jobsById = useMemo(() => new Map(jobs.map((job) => [job.job_id, job])), [jobs]);
  const totalPages = documents.reduce((sum, doc) => sum + (doc.page_count ?? 0), 0);
  const failedCount = documents.filter((doc) => doc.status === "failed").length;
  const failedPageCount = documents.reduce(
    (sum, doc) => sum + doc.analysis_failed_pages,
    0,
  );

  const filteredDocuments = useMemo(
    () => filterDocuments(documents, pagesByDocument, query, statusFilter),
    [documents, pagesByDocument, query, statusFilter],
  );
  const pageCount = Math.max(1, Math.ceil(filteredDocuments.length / PAGE_SIZE));
  const currentPage = Math.min(page, pageCount);
  const visibleDocuments = useMemo(
    () =>
      filteredDocuments.slice(
        (currentPage - 1) * PAGE_SIZE,
        currentPage * PAGE_SIZE,
      ),
    [filteredDocuments, currentPage],
  );

  useEffect(() => {
    setPage(1);
  }, [query, statusFilter]);

  useEffect(() => {
    setPage((value) => Math.min(value, pageCount));
  }, [pageCount]);

  function updateDocumentSelection(documentId: string, checked: boolean) {
    const next = new Set(selectedDocumentIds);
    if (checked) {
      next.add(documentId);
    } else {
      next.delete(documentId);
    }
    onSelectionChange([...next]);
  }

  function selectVisibleDocuments() {
    const next = new Set(selectedDocumentIds);
    for (const doc of visibleDocuments) {
      const validation = getDocumentReanalysisValidation(doc, pagesByDocument[doc.document_id] ?? []);
      if (!validation.disabledReason) {
        next.add(doc.document_id);
      }
    }
    onSelectionChange([...next]);
  }

  if (isLoading) {
    return <p className="muted-copy">媒体资产加载中...</p>;
  }

  if (documents.length === 0) {
    return null;
  }

  return (
    <div className="media-asset-list">
      <div className="doc-summary">
        <span>{documents.length} 个媒体</span>
        <span className="doc-summary-sep">·</span>
        <span>{totalPages} 页</span>
        {failedCount > 0 ? (
          <>
            <span className="doc-summary-sep">·</span>
            <span className="doc-summary-failed">{failedCount} 个失败</span>
          </>
        ) : null}
        {failedPageCount > 0 ? (
          <>
            <span className="doc-summary-sep">·</span>
            <span className="doc-summary-failed">{failedPageCount} 页分析失败</span>
          </>
        ) : null}
      </div>

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
              onChange={(event) =>
                onStatusFilterChange(event.target.value as MediaStatusFilter)
              }
            >
              <option value="all">全部</option>
              <option value="ready">已完成</option>
              <option value="needs_analysis">待分析</option>
              <option value="has_failed_pages">有失败页</option>
              <option value="failed">导入失败</option>
            </select>
          </label>
          <div className="media-selection-actions">
            <p className="document-list-count">
              {filteredDocuments.length} / {documents.length} 个媒体
            </p>
            <Button onClick={selectVisibleDocuments} disabled={visibleDocuments.length === 0}>
              选择当前页可重分析项
            </Button>
            <Button onClick={() => onSelectionChange([])} disabled={selectedDocumentIds.length === 0}>
              清空选择
            </Button>
          </div>
        </div>

        {filteredDocuments.length === 0 ? (
          <p className="document-empty-result">没有匹配的媒体。</p>
        ) : (
          <>
            <div className="document-asset-list" role="list" aria-label="媒体资产列表">
              {visibleDocuments.map((doc) => (
                <MediaDocumentRow
                  key={doc.document_id}
                  doc={doc}
                  pages={pagesByDocument[doc.document_id] ?? []}
                  job={doc.job_id ? jobsById.get(doc.job_id) : null}
                  workspacePath={workspacePath}
                  selected={selectedSet.has(doc.document_id)}
                  onSelectedChange={(checked) =>
                    updateDocumentSelection(doc.document_id, checked)
                  }
                  onRetry={onRetry}
                  onOpenSourceFile={onOpenSourceFile}
                  onOpenDocumentImage={onOpenDocumentImage}
                  onDeleteDocument={onDeleteDocument}
                  onReanalysisRequest={onReanalysisRequest}
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

interface MediaDocumentRowProps {
  doc: DocumentDto;
  pages: PageWorkbenchDto[];
  job?: JobDto | null;
  workspacePath?: string | null;
  selected: boolean;
  onSelectedChange: (checked: boolean) => void;
  onRetry?: (documentId: string) => void;
  onOpenSourceFile?: (path: string) => void;
  onOpenDocumentImage?: (page: PageWorkbenchDto) => void;
  onDeleteDocument?: (documentId: string) => void;
  onReanalysisRequest?: (selection: MediaAssetSelection) => void;
  deletingDocumentId?: string | null;
}

function MediaDocumentRow({
  doc,
  pages,
  job,
  workspacePath,
  selected,
  onSelectedChange,
  onRetry,
  onOpenSourceFile,
  onOpenDocumentImage,
  onDeleteDocument,
  onReanalysisRequest,
  deletingDocumentId,
}: MediaDocumentRowProps) {
  const isImporting = doc.status === "importing" && job;
  const isFailed = doc.status === "failed";
  const failedPages = pages.filter((page) => page.status === "failed");
  const failedPageCount = Math.max(failedPages.length, doc.analysis_failed_pages);
  const firstFailedPage = failedPages.find((page) => page.error_summary);
  const failedPageSummary = firstFailedPage?.error_summary
    ? `第 ${firstFailedPage.page_number} 页 ${firstFailedPage.error_summary}`
    : failedPageCount > 0
      ? `有 ${failedPageCount} 页处理失败，可展开页面详情查看。`
      : null;
  const isDeleting = deletingDocumentId === doc.document_id;
  const firstImagePage = pages.find((page) => Boolean(page.image_path));
  const generatedPageCount = pages.filter((page) => Boolean(page.image_path)).length;
  const analyzablePageCount = pages.filter((page) => page.status === "rendered").length;
  const pageTotal = doc.page_count ?? pages.length;
  const fallbackThumbnailSrc = useMemo(
    () => resolvePageImageSrc(firstImagePage?.image_path, workspacePath),
    [firstImagePage?.image_path, workspacePath],
  );
  const [thumbnailSrc, setThumbnailSrc] = useState<string | null>(null);
  const [isThumbnailLoading, setIsThumbnailLoading] = useState(false);
  const [thumbnailFailed, setThumbnailFailed] = useState(false);
  const validation = getDocumentReanalysisValidation(doc, pages);

  useEffect(() => {
    let cancelled = false;
    setThumbnailFailed(false);
    setThumbnailSrc(null);

    if (!firstImagePage?.page_id) {
      setIsThumbnailLoading(false);
      return () => {
        cancelled = true;
      };
    }

    setIsThumbnailLoading(true);
    tauriClient
      .getPageImagePreview(firstImagePage.page_id)
      .then((dataUrl) => {
        if (!cancelled) {
          setThumbnailSrc(dataUrl ?? fallbackThumbnailSrc);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setThumbnailSrc(fallbackThumbnailSrc);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setIsThumbnailLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [firstImagePage?.page_id, fallbackThumbnailSrc]);

  return (
    <article
      className="document-asset-row media-asset-row"
      role="listitem"
      aria-label={doc.original_filename}
      id={`media-${doc.document_id}`}
    >
      <label className="media-select-box">
        <input
          type="checkbox"
          checked={selected}
          onChange={(event) => onSelectedChange(event.target.checked)}
          disabled={Boolean(validation.disabledReason)}
          aria-label={`选择 ${doc.original_filename} 用于重分析`}
        />
        <span>选择</span>
      </label>

      <button
        type="button"
        className="document-thumb-button"
        onClick={() => {
          if (firstImagePage && onOpenDocumentImage) {
            onOpenDocumentImage(firstImagePage);
          }
        }}
        disabled={!firstImagePage || !onOpenDocumentImage}
        title={
          firstImagePage
            ? "查看此文档第一页图片"
            : "页面图片生成后会出现在这里"
        }
      >
        {firstImagePage ? (
          isThumbnailLoading ? (
            <span className="document-thumb-loading">加载预览</span>
          ) : thumbnailSrc && !thumbnailFailed ? (
            <img
              className="document-thumb-image"
              src={thumbnailSrc}
              alt={`${doc.original_filename} 第 ${firstImagePage.page_number} 页`}
              onError={() => setThumbnailFailed(true)}
            />
          ) : (
            <span className="document-thumb-real" aria-hidden="true" data-failed="true">
              <span className="document-thumb-page" />
              <span className="document-thumb-lines" />
            </span>
          )
        ) : (
          <span className="document-thumb-placeholder">
            <strong>{doc.file_type.toUpperCase()}</strong>
            <small>暂无页面图片</small>
          </span>
        )}
      </button>

      <div className="document-asset-main">
        <div className="document-asset-header">
          <div className="document-name-cell">
            <span className="document-type-label">{doc.file_type.toUpperCase()}</span>
            <span className="document-name-text" title={doc.original_filename}>
              {doc.original_filename}
            </span>
            <span className="document-path-text" title={doc.original_path}>
              {doc.original_path}
            </span>
          </div>
          <div className="document-status-stack">
            <StatusBadge tone={statusTone(doc.status)}>
              {statusLabel(doc.status)}
            </StatusBadge>
            <span className="document-date-cell">
              更新于 {formatDateTime(doc.updated_at)}
            </span>
          </div>
        </div>

        <div className="document-asset-stats" aria-label="页面资产状态">
          <span>{pageTotal} 页</span>
          <span>{generatedPageCount} 页已生成图片</span>
          <span>{doc.analysis_succeeded_pages} 页已分析</span>
          {analyzablePageCount > 0 ? <span>{analyzablePageCount} 页可分析</span> : null}
          {failedPageCount > 0 ? (
            <span className="doc-summary-failed">{failedPageCount} 页失败</span>
          ) : null}
        </div>

        {validation.disabledReason ? (
          <p className="media-disabled-reason">不可重分析：{validation.disabledReason}</p>
        ) : null}

        {isImporting ? (
          <div className="document-inline-progress" aria-label={`导入进度 ${boundedProgress(job.progress)}%`}>
            <span
              className="progress-fill"
              style={{ width: `${boundedProgress(job.progress)}%` }}
            />
          </div>
        ) : null}

        {failedPageSummary ? (
          <p className="job-error">失败原因：{failedPageSummary}</p>
        ) : null}

        {job?.last_event_message || doc.error_summary || pages.length > 0 ? (
          <div className="document-row-detail">
            {job?.last_event_message ? (
              <p className="job-event">{job.last_event_message}</p>
            ) : null}
            {doc.error_summary ? (
              <p className="job-error">失败原因：{doc.error_summary}</p>
            ) : null}
            {pages.length > 0 ? (
              <details className="document-pages-detail">
                <summary>页面详情（{pages.length}）</summary>
                <div className="page-list">
                  {pages.map((page) => (
                    <div key={page.page_id} className="page-item">
                      <div className="page-item-main">
                        <span>第 {page.page_number} 页</span>
                        {page.error_summary ? (
                          <span className="page-error-summary">
                            {page.error_summary}
                          </span>
                        ) : null}
                      </div>
                      <StatusBadge tone={pageStatusTone(page.status)}>
                        {pageStatusLabel(page.status)}
                      </StatusBadge>
                      {onOpenDocumentImage ? (
                        <Button
                          variant="secondary"
                          className="document-row-button"
                          onClick={() => onOpenDocumentImage(page)}
                          disabled={!page.image_path}
                          title={
                            page.image_path
                              ? "打开此页在 pages 目录中的图片"
                              : "此页面图片不可用"
                          }
                        >
                          查看页面
                        </Button>
                      ) : null}
                      {onReanalysisRequest ? (
                        <Button
                          variant="secondary"
                          className="document-row-button"
                          onClick={() =>
                            onReanalysisRequest({
                              kind: "page",
                              ids: [page.page_id],
                              label: `${doc.original_filename} 第 ${page.page_number} 页`,
                              disabledReason: getPageReanalysisReason(page),
                            })
                          }
                          disabled={Boolean(getPageReanalysisReason(page))}
                          title={getPageReanalysisReason(page) ?? "进入模型分析处理此页"}
                        >
                          重分析此页
                        </Button>
                      ) : null}
                      {page.status === "analyzed" && page.analysis_summary ? (
                        <PageAnalysisSummaryBlock summary={page.analysis_summary} />
                      ) : null}
                    </div>
                  ))}
                </div>
              </details>
            ) : null}
          </div>
        ) : null}
      </div>

      <div className="document-row-actions">
        {onOpenDocumentImage ? (
          <Button
            variant="primary"
            className="document-row-button"
            onClick={() => {
              if (firstImagePage) {
                onOpenDocumentImage(firstImagePage);
              }
            }}
            disabled={!firstImagePage}
            title={
              firstImagePage
                ? "打开此文档 pages 目录中的第一张页面图片"
                : "此文档还没有可查看的页面图片"
            }
          >
            查看页面
          </Button>
        ) : null}
        {onReanalysisRequest ? (
          <Button
            variant="secondary"
            className="document-row-button"
            onClick={() =>
              onReanalysisRequest({
                kind: "document",
                ids: [doc.document_id],
                label: doc.original_filename,
                disabledReason: validation.disabledReason,
              })
            }
            disabled={Boolean(validation.disabledReason)}
            title={validation.disabledReason ?? "进入模型分析处理此媒体"}
          >
            重分析
          </Button>
        ) : null}
        {isFailed && onRetry ? (
          <Button
            variant="secondary"
            className="document-row-button"
            onClick={() => onRetry(doc.document_id)}
          >
            重试导入
          </Button>
        ) : null}
        {onOpenSourceFile ? (
          <Button
            variant="secondary"
            className="document-row-button"
            onClick={() => onOpenSourceFile(doc.original_path)}
            title="使用系统默认应用打开导入的源文件"
          >
            源文件
          </Button>
        ) : null}
        {onDeleteDocument ? (
          <Button
            variant="secondary"
            className="document-row-button document-row-button-danger"
            onClick={() => {
              if (window.confirm(`确定删除文档“${doc.original_filename}”吗？`)) {
                onDeleteDocument(doc.document_id);
              }
            }}
            disabled={isDeleting}
            title="删除此文档及其工作区文件"
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
  if (totalItems <= PAGE_SIZE) {
    return null;
  }

  const first = (page - 1) * PAGE_SIZE + 1;
  const last = Math.min(totalItems, page * PAGE_SIZE);

  return (
    <div className="document-pagination">
      <p className="document-list-count">
        {first}-{last} / {totalItems}
      </p>
      <div className="document-pagination-actions">
        <Button onClick={() => onPageChange(page - 1)} disabled={page <= 1}>
          上一页
        </Button>
        <span className="document-page-indicator">
          {page} / {pageCount}
        </span>
        <Button onClick={() => onPageChange(page + 1)} disabled={page >= pageCount}>
          下一页
        </Button>
      </div>
    </div>
  );
}

function PageAnalysisSummaryBlock({
  summary,
}: {
  summary: PageAnalysisSummaryDto;
}) {
  return (
    <div className="page-analysis-summary">
      {summary.title ? (
        <p className="page-analysis-title">{summary.title}</p>
      ) : null}
      {summary.summary ? (
        <p className="muted-copy page-analysis-snippet">
          {truncateText(summary.summary, 120)}
        </p>
      ) : null}
      <p className="page-analysis-meta">
        {summary.keywords.length > 0
          ? `关键词 ${summary.keywords.length} 个`
          : "无关键词"}
        {" · "}
        {summary.topic_count > 0 ? `主题 ${summary.topic_count} 个` : "无主题"}
        {" · "}
        可见文本约 {summary.visible_text_char_count} 字
      </p>
    </div>
  );
}

function getDocumentReanalysisValidation(doc: DocumentDto, pages: PageWorkbenchDto[]) {
  if (doc.status === "failed") {
    return { disabledReason: "导入失败的媒体需先重试导入。" };
  }
  if (pages.length === 0) {
    return { disabledReason: "没有可查询的页面记录。" };
  }
  if (!pages.some((page) => page.image_path)) {
    return { disabledReason: "还没有生成页面图片。" };
  }
  if (!pages.some((page) => canReanalyzePage(page))) {
    return { disabledReason: "没有可分析或可重分析的页面。" };
  }
  return { disabledReason: null };
}

function getPageReanalysisReason(page: PageWorkbenchDto) {
  if (!page.image_path) {
    return "此页还没有页面图片。";
  }
  if (!canReanalyzePage(page)) {
    return "此页状态暂不可重分析。";
  }
  return null;
}

function canReanalyzePage(page: PageWorkbenchDto) {
  return page.status === "rendered" || page.status === "failed" || page.status === "analyzed";
}

function resolvePageImageSrc(
  imagePath: string | null | undefined,
  workspacePath: string | null | undefined,
) {
  const absolutePath = resolveWorkspacePath(imagePath, workspacePath);
  if (!absolutePath) {
    return null;
  }
  try {
    return convertFileSrc(absolutePath);
  } catch {
    return null;
  }
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

function filterDocuments(
  documents: DocumentDto[],
  pagesByDocument: Record<string, PageWorkbenchDto[]>,
  query: string,
  statusFilter: MediaStatusFilter,
) {
  const normalized = query.trim().toLocaleLowerCase("zh-CN");
  return documents.filter((doc) => {
    const pages = pagesByDocument[doc.document_id] ?? [];
    const matchesQuery =
      !normalized ||
      [
        doc.original_filename,
        doc.original_path,
        doc.file_type,
        statusLabel(doc.status),
        doc.status,
      ]
        .join(" ")
        .toLocaleLowerCase("zh-CN")
        .includes(normalized);
    if (!matchesQuery) {
      return false;
    }

    switch (statusFilter) {
      case "ready":
        return doc.status === "ready";
      case "failed":
        return doc.status === "failed";
      case "has_failed_pages":
        return doc.analysis_failed_pages > 0 || pages.some((page) => page.status === "failed");
      case "needs_analysis":
        return pages.some((page) => page.status === "rendered");
      case "all":
      default:
        return true;
    }
  });
}

function truncateText(text: string, maxLen: number) {
  if (text.length <= maxLen) {
    return text;
  }
  return `${text.slice(0, maxLen)}...`;
}

function boundedProgress(progress: number) {
  if (!Number.isFinite(progress)) {
    return 0;
  }
  return Math.min(100, Math.max(0, Math.round(progress)));
}

function pageStatusLabel(status: string) {
  switch (status) {
    case "rendered":
      return "已渲染";
    case "analysis_pending":
      return "分析中";
    case "analyzed":
      return "已分析";
    case "failed":
      return "已失败";
    default:
      return status;
  }
}

function pageStatusTone(status: string) {
  switch (status) {
    case "rendered":
    case "analyzed":
      return "success";
    case "analysis_pending":
      return "warning";
    case "failed":
      return "danger";
    default:
      return "neutral";
  }
}

function statusLabel(status: string) {
  switch (status) {
    case "ready":
      return "已完成";
    case "importing":
      return "导入中";
    case "failed":
      return "已失败";
    case "pending":
      return "等待中";
    default:
      return status;
  }
}

function statusTone(status: string) {
  switch (status) {
    case "ready":
      return "success";
    case "failed":
      return "danger";
    case "importing":
      return "warning";
    default:
      return "neutral";
  }
}

function formatDateTime(value: string) {
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return value;
  }
  return new Intl.DateTimeFormat("zh-CN", {
    dateStyle: "short",
    timeStyle: "medium",
  }).format(parsed);
}
