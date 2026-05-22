import { useCallback, useEffect, useState } from "react";
import { Button } from "../../components/common/Button";
import { EmptyState } from "../../components/common/EmptyState";
import { ErrorMessage } from "../../components/common/ErrorMessage";
import { StatusBadge } from "../../components/common/StatusBadge";
import { tauriClient } from "../../lib/tauriClient";
import type {
  IndexStatusDto,
  SearchResponseDto,
  SearchResultItemDto,
} from "../../types/app";
import { SEARCH_PAGE_COPY as t } from "./searchPageCopy";

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
  const [selected, setSelected] = useState<SearchResultItemDto | null>(null);
  const [previewSrc, setPreviewSrc] = useState<string | null>(null);
  const [isPreviewLoading, setIsPreviewLoading] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [isSearching, setIsSearching] = useState(false);
  const [isRebuilding, setIsRebuilding] = useState(false);
  const [searchError, setSearchError] = useState<{
    message: string;
    correlationId?: string | null;
  } | null>(null);
  const [statusError, setStatusError] = useState<string | null>(null);

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
    if (!workspaceReady || !isActive) {
      return;
    }
    void refreshIndexStatus();
  }, [workspaceReady, isActive, refreshIndexStatus]);

  useEffect(() => {
    if (!workspaceReady || !isActive) {
      return;
    }
    if (indexStatus?.status !== "building") {
      return;
    }
    const timer = window.setInterval(() => {
      void refreshIndexStatus();
    }, 2000);
    return () => window.clearInterval(timer);
  }, [workspaceReady, isActive, indexStatus?.status, refreshIndexStatus]);

  useEffect(() => {
    if (!selected?.image_available) {
      setPreviewSrc(null);
      setIsPreviewLoading(false);
      setPreviewError(null);
      return;
    }

    let cancelled = false;
    setPreviewSrc(null);
    setIsPreviewLoading(true);
    setPreviewError(null);

    tauriClient
      .getPageImagePreview(selected.page_id)
      .then((dataUrl) => {
        if (cancelled) {
          return;
        }
        if (dataUrl) {
          setPreviewSrc(dataUrl);
        } else {
          setPreviewError(t.imageMissing);
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setPreviewError(extractError(error).message);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setIsPreviewLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [selected]);

  async function handleSearch() {
    const trimmed = query.trim();
    if (!trimmed) {
      setResults({ items: [], query: "", limit: 20 });
      setSubmittedQuery("");
      setSelected(null);
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
      setSelected(response.items[0] ?? null);
    } catch (error) {
      setSearchError(extractError(error));
      setResults(null);
      setSelected(null);
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

  const statusTone = indexStatusTone(indexStatus?.status);
  const statusLabel = indexStatusLabel(indexStatus, isStatusLoading);
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
          <StatusBadge tone={statusTone}>{statusLabel}</StatusBadge>
        </div>
        {statusError ? <p className="job-error">{statusError}</p> : null}
        {indexStatus?.error_summary ? (
          <p className="job-error">{indexStatus.error_summary}</p>
        ) : null}
        {indexStatus?.stale_reason ? (
          <p className="muted-copy">{indexStatus.stale_reason}</p>
        ) : null}
        {indexStatus?.search_uses_stale_index ? (
          <p className="muted-copy">{t.staleIndex}</p>
        ) : null}
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
            variant="secondary"
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

      <section className="panel">
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
          <EmptyState
            title={t.noMatches}
            description={t.noMatchesDesc(submittedQuery)}
          />
        ) : results && results.items.length > 0 ? (
          <ul className="search-results-list">
            {results.items.map((item) => (
              <li key={item.page_id}>
                <button
                  type="button"
                  className={
                    selected?.page_id === item.page_id
                      ? "search-result-item selected"
                      : "search-result-item"
                  }
                  onClick={() => setSelected(item)}
                >
                  <div className="search-result-title">
                    {item.title?.trim() || `第 ${item.page_number} 页`}
                  </div>
                  <p className="muted-copy search-result-meta">
                    {item.original_filename ?? t.unknownDoc} · 第 {item.page_number} 页 · 相关度{" "}
                    {item.score.toFixed(2)}
                  </p>
                  {item.summary ? (
                    <p className="search-result-snippet">{item.summary}</p>
                  ) : null}
                </button>
              </li>
            ))}
          </ul>
        ) : (
          <EmptyState title={t.noResultsYet} description={t.noResultsYetDesc} />
        )}
      </section>

      <section className="panel">
        <div className="panel-header compact">
          <h3>{t.preview}</h3>
          <StatusBadge tone={selected?.image_available ? "success" : "neutral"}>
            {selected ? (selected.image_available ? t.available : t.missing) : t.selectOne}
          </StatusBadge>
        </div>
        {selected?.image_available ? (
          isPreviewLoading ? (
            <p className="muted-copy">{t.previewLoading}</p>
          ) : previewSrc ? (
            <img
              className="search-preview-image"
              src={previewSrc}
              alt={selected.title ?? `第 ${selected.page_number} 页`}
            />
          ) : (
            <p className="muted-copy">{previewError ?? t.imageMissing}</p>
          )
        ) : selected ? (
          <p className="muted-copy">{t.imageMissing}</p>
        ) : (
          <p className="muted-copy">{t.selectForPreview}</p>
        )}
      </section>

      <section className="panel">
        <div className="panel-header compact">
          <h3>{t.jsonView}</h3>
          <StatusBadge>{selected ? "page_analysis_v1" : t.selectOne}</StatusBadge>
        </div>
        <pre className="json-placeholder">
          {selected?.page_json ??
            `{\n  "status": "${t.selectForJson}"\n}`}
        </pre>
      </section>
    </div>
  );
}

function indexStatusLabel(status: IndexStatusDto | null, loading: boolean): string {
  if (loading) {
    return t.checking;
  }
  switch (status?.status) {
    case "ready":
      return t.indexReady;
    case "building":
      return t.building;
    case "failed":
      return t.indexFailed;
    case "needs_rebuild":
      return t.needsRebuild;
    case "not_built":
    default:
      return t.notBuilt;
  }
}

function indexStatusTone(
  status: string | undefined,
): "success" | "warning" | "neutral" | "danger" {
  switch (status) {
    case "ready":
      return "success";
    case "building":
      return "warning";
    case "failed":
      return "danger";
    default:
      return "neutral";
  }
}

function indexStatusHint(status: IndexStatusDto | null): string {
  if (!status) {
    return t.loadingStatus;
  }
  const parts = [
    `已索引 ${status.indexed_page_count} 页`,
    `可索引 ${status.analyzable_page_count} 页`,
  ];
  if (status.pending_index_page_count > 0) {
    parts.push(`${status.pending_index_page_count} 页待纳入`);
  }
  return `${parts.join("，")}。`;
}

function extractError(error: unknown): { message: string; correlationId?: string | null } {
  if (typeof error === "object" && error !== null) {
    const e = error as Record<string, unknown>;
    return {
      message: typeof e.message === "string" ? e.message : t.opFailed,
      correlationId:
        typeof e.correlation_id === "string" ? e.correlation_id : null,
    };
  }
  return { message: typeof error === "string" ? error : t.opFailed };
}
