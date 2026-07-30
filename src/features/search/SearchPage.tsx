import { useCallback, useEffect, useMemo, useState } from "react";
import { Button } from "../../components/common/Button";
import { EmptyState } from "../../components/common/EmptyState";
import { ErrorMessage } from "../../components/common/ErrorMessage";
import { StatusBadge } from "../../components/common/StatusBadge";
import { DocumentFormatViewer } from "../../components/document-viewer/DocumentFormatViewer";
import { tauriClient } from "../../lib/tauriClient";
import type {
  IndexStatusDto,
  SearchResponseDto,
  SearchResultItemDto,
} from "../../types/app";
import { SEARCH_PAGE_COPY as t } from "./searchPageCopy";

export interface SearchResultEntry {
  item: SearchResultItemDto;
  hitId: string;
}

interface SearchPageProps {
  workspaceReady: boolean;
  isActive: boolean;
}

export function SearchPage({ workspaceReady, isActive }: SearchPageProps) {
  const [indexStatus, setIndexStatus] = useState<IndexStatusDto | null>(null);
  const [isStatusLoading, setIsStatusLoading] = useState(false);
  const [query, setQuery] = useState("");
  const [submittedQuery, setSubmittedQuery] = useState("");
  const [results, setResults] = useState<SearchResponseDto | null>(null);
  const [selectedHitId, setSelectedHitId] = useState<string | null>(null);
  const [isSearching, setIsSearching] = useState(false);
  const [isRebuilding, setIsRebuilding] = useState(false);
  const [searchError, setSearchError] = useState<{
    message: string;
    correlationId?: string | null;
  } | null>(null);
  const [statusError, setStatusError] = useState<string | null>(null);

  const resultEntries = useMemo(
    () => buildSearchResultEntries(results?.items ?? []),
    [results],
  );
  const selected = useMemo(
    () => findSelectedSearchResult(resultEntries, selectedHitId),
    [resultEntries, selectedHitId],
  );

  const refreshIndexStatus = useCallback(async () => {
    if (!workspaceReady) {
      setIndexStatus(null);
      return;
    }
    setIsStatusLoading(true);
    setStatusError(null);
    try {
      setIndexStatus(await tauriClient.getIndexStatus());
    } catch (error) {
      setStatusError(extractError(error).message);
    } finally {
      setIsStatusLoading(false);
    }
  }, [workspaceReady]);

  useEffect(() => {
    if (workspaceReady && isActive) {
      void refreshIndexStatus();
    }
  }, [workspaceReady, isActive, refreshIndexStatus]);

  useEffect(() => {
    if (!workspaceReady || !isActive || indexStatus?.status !== "building") {
      return;
    }
    const timer = window.setInterval(() => void refreshIndexStatus(), 2000);
    return () => window.clearInterval(timer);
  }, [workspaceReady, isActive, indexStatus?.status, refreshIndexStatus]);

  async function handleSearch() {
    const trimmed = query.trim();
    if (!trimmed) {
      setResults({ items: [], query: "", limit: 20 });
      setSubmittedQuery("");
      setSelectedHitId(null);
      return;
    }
    if (!indexStatus?.can_search) {
      setSearchError({ message: t.searchUnavailable });
      return;
    }
    setIsSearching(true);
    setSearchError(null);
    setSubmittedQuery(trimmed);
    try {
      const response = await tauriClient.searchPages(trimmed, 20);
      setResults(response);
      setSelectedHitId(response.items[0] ? searchHitId(response.items[0], 0) : null);
    } catch (error) {
      setSearchError(extractError(error));
      setResults(null);
      setSelectedHitId(null);
    } finally {
      setIsSearching(false);
    }
  }

  async function handleRebuildIndex() {
    if (!workspaceReady || isRebuilding) {
      return;
    }
    setIsRebuilding(true);
    setStatusError(null);
    try {
      await tauriClient.startIndexRebuild();
      await refreshIndexStatus();
    } catch (error) {
      setStatusError(extractError(error).message);
    } finally {
      setIsRebuilding(false);
    }
  }

  const noIndexablePages = (indexStatus?.analyzable_page_count ?? 0) === 0;

  return (
    <div className="page-grid search-layout">
      <section className="panel panel-wide">
        <div className="panel-header">
          <div>
            <p className="eyebrow">{t.queryEyebrow}</p>
            <h2>{t.title}</h2>
            <p className="muted-copy">{indexStatusHint(indexStatus)}</p>
          </div>
          <StatusBadge tone={indexStatusTone(indexStatus?.status)}>
            {indexStatusLabel(indexStatus, isStatusLoading)}
          </StatusBadge>
        </div>
        {statusError ? <p className="job-error">{statusError}</p> : null}
        {indexStatus?.error_summary ? <p className="job-error">{indexStatus.error_summary}</p> : null}
        {indexStatus?.stale_reason ? <p className="muted-copy">{indexStatus.stale_reason}</p> : null}
        {indexStatus?.search_uses_stale_index ? <p className="muted-copy">{t.staleIndex}</p> : null}
        <div className="search-bar" aria-label={t.searchAria}>
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                void handleSearch();
              }
            }}
            placeholder={t.placeholder}
            disabled={!workspaceReady || isSearching || !indexStatus?.can_search}
          />
          <Button
            variant="primary"
            onClick={() => void handleSearch()}
            disabled={!workspaceReady || isSearching || !indexStatus?.can_search}
          >
            {isSearching ? t.searching : t.search}
          </Button>
          <Button
            onClick={() => void handleRebuildIndex()}
            disabled={
              !workspaceReady ||
              isRebuilding ||
              indexStatus?.status === "building" ||
              indexStatus?.can_rebuild === false ||
              noIndexablePages
            }
          >
            {isRebuilding || indexStatus?.status === "building"
              ? t.rebuilding
              : indexStatus?.status === "not_built"
                ? t.buildIndex
                : t.rebuildIndex}
          </Button>
        </div>
        {searchError ? (
          <ErrorMessage
            title={t.searchFailed}
            message={searchError.message}
            correlationId={searchError.correlationId}
          />
        ) : null}
      </section>

      <section className="panel search-results-panel">
        <div className="panel-header compact">
          <h3>{t.results}</h3>
          <StatusBadge>{results ? t.resultCount(results.items.length) : t.empty}</StatusBadge>
        </div>
        {!workspaceReady ? (
          <EmptyState title={t.waitWorkspace} description={t.waitWorkspaceDesc} />
        ) : !indexStatus?.can_search ? (
          <EmptyState
            title={t.indexUnavailable}
            description={
              noIndexablePages
                ? t.noIndexableHint
                : indexStatus?.status === "building"
                  ? t.indexBuilding
                  : t.buildIndexFirst
            }
          />
        ) : results && submittedQuery && results.items.length === 0 ? (
          <EmptyState title={t.noMatches} description={t.noMatchesDesc(submittedQuery)} />
        ) : results && results.items.length > 0 ? (
          <ul className="search-results-list">
            {resultEntries.map(({ item, hitId }) => (
              <li key={hitId}>
                <button
                  type="button"
                  className={selectedHitId === hitId ? "search-result-item selected" : "search-result-item"}
                  onClick={() => setSelectedHitId(hitId)}
                  aria-pressed={selectedHitId === hitId}
                >
                  <div className="search-result-heading">
                    <span className="search-result-title">
                      {item.title?.trim() || `第 ${item.page_number} 页`}
                    </span>
                    {isModuleHit(item) ? (
                      <span className="search-result-type">{moduleTypeLabel(item)}</span>
                    ) : null}
                  </div>
                  <p className="muted-copy search-result-meta">
                    {item.original_filename ?? t.unknownDoc} · 第 {item.page_number} 页 · 相关度 {item.score.toFixed(2)}
                  </p>
                  {searchSnippet(item) ? <p className="search-result-snippet">{searchSnippet(item)}</p> : null}
                </button>
              </li>
            ))}
          </ul>
        ) : (
          <EmptyState title={t.noResultsYet} description={t.noResultsYetDesc} />
        )}
      </section>

      <div className="search-viewer-column">
        <DocumentFormatViewer
          documentId={selected?.document_id ?? null}
          documentTitle={selected?.original_filename}
          pageNumber={selected?.page_number}
        />
      </div>
    </div>
  );
}

export function buildSearchResultEntries(items: SearchResultItemDto[]): SearchResultEntry[] {
  return items.map((item, index) => ({ item, hitId: searchHitId(item, index) }));
}

export function findSelectedSearchResult(
  entries: SearchResultEntry[],
  selectedHitId: string | null,
): SearchResultItemDto | null {
  return entries.find(({ hitId }) => hitId === selectedHitId)?.item ?? null;
}

export function searchHitId(item: SearchResultItemDto, index: number): string {
  if (typeof item.hit_id === "string" && item.hit_id.trim()) {
    return item.hit_id;
  }
  if (typeof item.module_id === "string" && item.module_id.trim()) {
    return `module:${item.module_id}`;
  }
  return `legacy-page:${item.page_id}:${index}`;
}

function isModuleHit(item: SearchResultItemDto): boolean {
  return Boolean(item.module_id) || moduleType(item) !== "page";
}

function moduleType(item: SearchResultItemDto): string {
  const value = item.module_type ?? item.type;
  return typeof value === "string" && value.trim() ? value.trim() : "page";
}

function moduleTypeLabel(item: SearchResultItemDto): string {
  const value = moduleType(item);
  const labels: Record<string, string> = {
    page: "页面",
    title: "标题",
    heading: "标题",
    paragraph: "段落",
    text: "文本",
    list: "列表",
    table: "表格",
    image: "图片",
    figure: "图片",
    caption: "图注",
  };
  return labels[value.toLowerCase()] ?? value;
}

function searchSnippet(item: SearchResultItemDto): string | null {
  return item.snippet?.trim() || item.summary?.trim() || null;
}

function indexStatusLabel(status: IndexStatusDto | null, loading: boolean): string {
  if (loading) return t.checking;
  switch (status?.status) {
    case "ready": return t.indexReady;
    case "building": return t.building;
    case "failed": return t.indexFailed;
    case "needs_rebuild": return t.needsRebuild;
    default: return t.notBuilt;
  }
}

function indexStatusTone(status: string | undefined): "success" | "warning" | "neutral" | "danger" {
  if (status === "ready") return "success";
  if (status === "building") return "warning";
  if (status === "failed") return "danger";
  return "neutral";
}

function indexStatusHint(status: IndexStatusDto | null): string {
  if (!status) return t.loadingStatus;
  const parts = [`已索引 ${status.indexed_page_count} 个内容项`, `可索引 ${status.analyzable_page_count} 个内容项`];
  if (status.pending_index_page_count > 0) {
    parts.push(`${status.pending_index_page_count} 个内容项待纳入`);
  }
  return `${parts.join("，")}。`;
}

function extractError(error: unknown): { message: string; correlationId?: string | null } {
  if (typeof error === "object" && error !== null) {
    const value = error as Record<string, unknown>;
    return {
      message: typeof value.message === "string" ? value.message : t.opFailed,
      correlationId: typeof value.correlation_id === "string" ? value.correlation_id : null,
    };
  }
  return { message: typeof error === "string" ? error : t.opFailed };
}
