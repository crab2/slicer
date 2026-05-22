import { useEffect, useMemo, useState } from "react";
import { Button } from "../../../components/common/Button";
import { StatusBadge } from "../../../components/common/StatusBadge";
import type {
  DocumentDto,
  JobDto,
  PageAnalysisSummaryDto,
  PageWorkbenchDto,
} from "../../../types/app";

const PAGE_SIZE = 8;

interface DocumentListProps {
  documents: DocumentDto[];
  pagesByDocument: Record<string, PageWorkbenchDto[]>;
  jobs: JobDto[];
  isLoading: boolean;
  onRetry?: (documentId: string) => void;
  onAnalyzePage?: (pageId: string) => void;
  onReanalyzeDocument?: (documentId: string) => void;
  onReanalyzeFailedPages?: (documentId: string) => void;
  onOpenSourceFile?: (path: string) => void;
  onOpenDocumentImage?: (page: PageWorkbenchDto) => void;
  onDeleteDocument?: (documentId: string) => void;
  analyzingPageId?: string | null;
  reanalyzingDocumentId?: string | null;
  reanalyzingFailedDocumentId?: string | null;
  deletingDocumentId?: string | null;
}

export function DocumentList({
  documents,
  pagesByDocument,
  jobs,
  isLoading,
  onRetry,
  onAnalyzePage,
  onReanalyzeDocument,
  onReanalyzeFailedPages,
  onOpenSourceFile,
  onOpenDocumentImage,
  onDeleteDocument,
  analyzingPageId,
  reanalyzingDocumentId,
  reanalyzingFailedDocumentId,
  deletingDocumentId,
}: DocumentListProps) {
  const [query, setQuery] = useState("");
  const [page, setPage] = useState(1);

  const jobsById = useMemo(() => new Map(jobs.map((j) => [j.job_id, j])), [jobs]);
  const totalPages = documents.reduce((sum, doc) => sum + (doc.page_count ?? 0), 0);
  const failedCount = documents.filter((d) => d.status === "failed").length;
  const failedPageCount = documents.reduce(
    (sum, doc) => sum + doc.analysis_failed_pages,
    0,
  );
  const filteredDocuments = useMemo(
    () => filterDocuments(documents, query),
    [documents, query],
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
  }, [query]);

  useEffect(() => {
    setPage((value) => Math.min(value, pageCount));
  }, [pageCount]);

  if (isLoading) {
    return <p className="muted-copy">文档加载中...</p>;
  }

  if (documents.length === 0) {
    return null;
  }

  return (
    <div className="document-list">
      <div className="doc-summary">
        <span>{documents.length} 个文档</span>
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

      <div className="document-list-panel">
        <div className="document-list-toolbar">
          <label className="document-search-field">
            <span>搜索文档</span>
            <input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="按文件名、路径、类型或状态搜索"
            />
          </label>
          <p className="document-list-count">
            {filteredDocuments.length} / {documents.length} 个文档
          </p>
        </div>

        {filteredDocuments.length === 0 ? (
          <p className="document-empty-result">没有匹配的文档。</p>
        ) : (
          <>
            <div className="document-table" role="table" aria-label="文档列表">
              <div className="document-table-row document-table-head" role="row">
                <span role="columnheader">文档</span>
                <span role="columnheader">状态</span>
                <span role="columnheader">页数</span>
                <span role="columnheader">分析</span>
                <span role="columnheader">更新于</span>
                <span role="columnheader">操作</span>
              </div>

              {visibleDocuments.map((doc) => (
                <DocumentRow
                  key={doc.document_id}
                  doc={doc}
                  pages={pagesByDocument[doc.document_id] ?? []}
                  job={doc.job_id ? jobsById.get(doc.job_id) : null}
                  onRetry={onRetry}
                  onAnalyzePage={onAnalyzePage}
                  onReanalyzeDocument={onReanalyzeDocument}
                  onReanalyzeFailedPages={onReanalyzeFailedPages}
                  onOpenSourceFile={onOpenSourceFile}
                  onOpenDocumentImage={onOpenDocumentImage}
                  onDeleteDocument={onDeleteDocument}
                  analyzingPageId={analyzingPageId}
                  reanalyzingDocumentId={reanalyzingDocumentId}
                  reanalyzingFailedDocumentId={reanalyzingFailedDocumentId}
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

interface DocumentRowProps {
  doc: DocumentDto;
  pages: PageWorkbenchDto[];
  job?: JobDto | null;
  onRetry?: (documentId: string) => void;
  onAnalyzePage?: (pageId: string) => void;
  onReanalyzeDocument?: (documentId: string) => void;
  onReanalyzeFailedPages?: (documentId: string) => void;
  onOpenSourceFile?: (path: string) => void;
  onOpenDocumentImage?: (page: PageWorkbenchDto) => void;
  onDeleteDocument?: (documentId: string) => void;
  analyzingPageId?: string | null;
  reanalyzingDocumentId?: string | null;
  reanalyzingFailedDocumentId?: string | null;
  deletingDocumentId?: string | null;
}

function DocumentRow({
  doc,
  pages,
  job,
  onRetry,
  onAnalyzePage,
  onReanalyzeDocument,
  onReanalyzeFailedPages,
  onOpenSourceFile,
  onOpenDocumentImage,
  onDeleteDocument,
  analyzingPageId,
  reanalyzingDocumentId,
  reanalyzingFailedDocumentId,
  deletingDocumentId,
}: DocumentRowProps) {
  const isImporting = doc.status === "importing" && job;
  const isFailed = doc.status === "failed";
  const failedPages = pages.filter((page) => page.status === "failed");
  const failedPageCount = Math.max(failedPages.length, doc.analysis_failed_pages);
  const isReanalyzing = reanalyzingDocumentId === doc.document_id;
  const isReanalyzingFailed = reanalyzingFailedDocumentId === doc.document_id;
  const isDeleting = deletingDocumentId === doc.document_id;
  const firstImagePage = pages.find((page) => Boolean(page.image_path));

  return (
    <div className="document-table-row" role="row">
      <div className="document-name-cell" role="cell">
        <span className="document-type-label">{doc.file_type.toUpperCase()}</span>
        <span className="document-name-text" title={doc.original_filename}>
          {doc.original_filename}
        </span>
        <span className="document-path-text" title={doc.original_path}>
          {doc.original_path}
        </span>
      </div>

      <div className="document-status-cell" role="cell">
        <StatusBadge tone={statusTone(doc.status)}>
          {statusLabel(doc.status)}
        </StatusBadge>
        {isImporting ? (
          <div className="document-inline-progress">
            <span
              className="progress-fill"
              style={{ width: `${boundedProgress(job.progress)}%` }}
            />
          </div>
        ) : null}
      </div>

      <span className="document-table-number" role="cell">
        {doc.page_count ?? 0} 页
      </span>

      <div className="document-analysis-cell" role="cell">
        <span>{doc.analysis_succeeded_pages} 页已分析</span>
        {failedPageCount > 0 ? (
          <span className="doc-summary-failed">{failedPageCount} 页失败</span>
        ) : null}
      </div>

      <span className="document-date-cell" role="cell">
        {formatDateTime(doc.updated_at)}
      </span>

      <div className="document-row-actions" role="cell">
        {onOpenSourceFile ? (
          <Button
            variant="secondary"
            className="document-row-button"
            onClick={() => onOpenSourceFile(doc.original_path)}
            title="使用系统默认应用打开导入的源文件"
          >
            查看源文件
          </Button>
        ) : null}
        {onOpenDocumentImage ? (
          <Button
            variant="secondary"
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
            查看图片
          </Button>
        ) : null}
        {onReanalyzeFailedPages && failedPageCount > 0 ? (
          <Button
            variant="secondary"
            className="document-row-button"
            onClick={() => onReanalyzeFailedPages(doc.document_id)}
            disabled={isReanalyzingFailed}
            title="重新分析此文档中的失败页面"
          >
            {isReanalyzingFailed ? "重分析中" : "重试失败页"}
          </Button>
        ) : null}
        {onReanalyzeDocument && doc.status === "ready" ? (
          <Button
            variant="secondary"
            className="document-row-button"
            onClick={() => onReanalyzeDocument(doc.document_id)}
            disabled={isReanalyzing}
            title="重新分析此文档"
          >
            {isReanalyzing ? "重分析中" : "重分析"}
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
                        查看图片
                      </Button>
                    ) : null}
                    {page.status === "analyzed" && page.analysis_summary ? (
                      <PageAnalysisSummaryBlock summary={page.analysis_summary} />
                    ) : null}
                    {onAnalyzePage && canAnalyzePage(page.status) ? (
                      <Button
                        variant="secondary"
                        onClick={() => onAnalyzePage(page.page_id)}
                        disabled={analyzingPageId === page.page_id}
                      >
                        {pageActionLabel(page.status, analyzingPageId === page.page_id)}
                      </Button>
                    ) : null}
                  </div>
                ))}
              </div>
            </details>
          ) : null}
        </div>
      ) : null}
    </div>
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

function filterDocuments(documents: DocumentDto[], query: string) {
  const normalized = query.trim().toLocaleLowerCase("zh-CN");
  if (!normalized) {
    return documents;
  }

  return documents.filter((doc) =>
    [
      doc.original_filename,
      doc.original_path,
      doc.file_type,
      statusLabel(doc.status),
      doc.status,
    ]
      .join(" ")
      .toLocaleLowerCase("zh-CN")
      .includes(normalized),
  );
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

function canAnalyzePage(status: string) {
  return status === "rendered" || status === "failed" || status === "analyzed";
}

function pageActionLabel(status: string, isAnalyzing: boolean) {
  if (isAnalyzing) {
    return "分析中...";
  }
  if (status === "failed") {
    return "重试此页";
  }
  if (status === "analyzed") {
    return "重新分析此页";
  }
  return "分析此页";
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
