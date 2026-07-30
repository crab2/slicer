import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Button } from "../../components/common/Button";
import { EmptyState } from "../../components/common/EmptyState";
import { ErrorMessage } from "../../components/common/ErrorMessage";
import { StatusBadge } from "../../components/common/StatusBadge";
import { tauriClient } from "../../lib/tauriClient";
import type {
  IndexStatusDto,
  NormalizedBoundingBoxDto,
  SearchResponseDto,
  SearchResultItemDto,
} from "../../types/app";
import { SEARCH_PAGE_COPY as t } from "./searchPageCopy";

const PREVIEW_CACHE_ENTRY_LIMIT = 24;
const PREVIEW_CACHE_BYTE_LIMIT = 32 * 1024 * 1024;

interface PreviewCacheEntry {
  dataUrl: string;
  byteSize: number;
}

export interface PreviewCacheState {
  entries: Map<string, PreviewCacheEntry>;
  totalBytes: number;
}

interface PreviewCacheLimits {
  maxEntries: number;
  maxBytes: number;
}

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
  const [previewSrc, setPreviewSrc] = useState<string | null>(null);
  const [isPreviewLoading, setIsPreviewLoading] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [isLargePreviewOpen, setIsLargePreviewOpen] = useState(false);
  const [isSearching, setIsSearching] = useState(false);
  const [isRebuilding, setIsRebuilding] = useState(false);
  const [searchError, setSearchError] = useState<{
    message: string;
    correlationId?: string | null;
  } | null>(null);
  const [statusError, setStatusError] = useState<string | null>(null);
  const previewCache = useRef(createPreviewCacheState());
  const previewRequests = useRef(new Map<string, Promise<string | null>>());
  const previewGeneration = useRef(0);

  const resultEntries = useMemo(
    () => buildSearchResultEntries(results?.items ?? []),
    [results],
  );
  const selected = useMemo(
    () => findSelectedSearchResult(resultEntries, selectedHitId),
    [resultEntries, selectedHitId],
  );
  const selectedBbox = validNormalizedBbox(selected?.bbox);

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
    if (!workspaceReady) {
      previewGeneration.current += 1;
      clearPreviewCache(previewCache.current);
      previewRequests.current.clear();
    }
  }, [workspaceReady]);

  useEffect(() => {
    const pageId = selected?.page_id;
    if (!workspaceReady || !pageId || !selected.image_available) {
      setPreviewSrc(null);
      setIsPreviewLoading(false);
      setPreviewError(null);
      return;
    }

    let cancelled = false;
    const cached = getCachedPagePreview(previewCache.current, pageId);
    if (cached) {
      setPreviewSrc(cached);
      setIsPreviewLoading(false);
      setPreviewError(null);
      return;
    }

    const generation = previewGeneration.current;
    const request = loadPagePreviewOnce(
      previewCache.current,
      previewRequests.current,
      pageId,
      async () => {
        const dataUrl = await tauriClient.getPageImagePreview(pageId);
        return generation === previewGeneration.current ? dataUrl : null;
      },
    );

    setPreviewSrc(null);
    setIsPreviewLoading(true);
    setPreviewError(null);

    request
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
  }, [workspaceReady, selected?.page_id, selected?.image_available]);

  useEffect(() => {
    setIsLargePreviewOpen(false);
  }, [selected?.page_id]);

  useEffect(() => {
    if (!isLargePreviewOpen) {
      return;
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setIsLargePreviewOpen(false);
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isLargePreviewOpen]);

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
      setSelectedHitId(
        response.items[0] ? searchHitId(response.items[0], 0) : null,
      );
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

  const statusTone = indexStatusTone(indexStatus?.status);
  const statusLabel = indexStatusLabel(indexStatus, isStatusLoading);
  const noIndexablePages = (indexStatus?.analyzable_page_count ?? 0) === 0;
  const selectedPreviewLabel = selected
    ? previewLabel(selected)
    : "";
  const selectedType = selected ? moduleTypeLabel(selected) : "";

  return (
    <>
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
              {resultEntries.map(({ item, hitId }) => (
                <li key={hitId}>
                  <button
                    type="button"
                    className={
                      selectedHitId === hitId
                        ? "search-result-item selected"
                        : "search-result-item"
                    }
                    onClick={() => setSelectedHitId(hitId)}
                    aria-pressed={selectedHitId === hitId}
                  >
                    <div className="search-result-heading">
                      <span className="search-result-title">
                        {item.title?.trim() || `第 ${item.page_number} 页`}
                      </span>
                      {isModuleHit(item) ? (
                        <span className="search-result-type">
                          {moduleTypeLabel(item)}
                        </span>
                      ) : null}
                    </div>
                    <p className="muted-copy search-result-meta">
                      {item.original_filename ?? t.unknownDoc} · 第 {item.page_number} 页 · 相关度{" "}
                      {item.score.toFixed(2)}
                    </p>
                    {searchSnippet(item) ? (
                      <p className="search-result-snippet">{searchSnippet(item)}</p>
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
            <StatusBadge
              tone={
                selected?.image_available
                  ? selectedBbox
                    ? "success"
                    : "warning"
                  : "neutral"
              }
            >
              {selected
                ? selected.image_available
                  ? selectedBbox
                    ? t.located
                    : t.notLocated
                  : t.missing
                : t.selectOne}
            </StatusBadge>
          </div>
          {selected?.image_available ? (
            isPreviewLoading ? (
              <p className="muted-copy">{t.previewLoading}</p>
            ) : previewSrc ? (
              <button
                type="button"
                className="search-preview-button"
                onClick={() => setIsLargePreviewOpen(true)}
                aria-label={`${t.openLargePreview}：${selectedPreviewLabel}`}
                title={t.openLargePreview}
              >
                <AnnotatedPageImage
                  src={previewSrc}
                  alt={selectedPreviewLabel}
                  bbox={selectedBbox}
                  imageClassName="search-preview-image"
                />
              </button>
            ) : (
              <p className="muted-copy">{previewError ?? t.imageMissing}</p>
            )
          ) : selected ? (
            <p className="muted-copy">{t.imageMissing}</p>
          ) : (
            <p className="muted-copy">{t.selectForPreview}</p>
          )}
          {selected ? (
            <p
              className="search-location-status"
              data-locatable={Boolean(selectedBbox)}
              role="status"
            >
              {selectedBbox
                ? t.locationAvailable(selectedType)
                : t.locationUnavailable}
            </p>
          ) : null}
        </section>

        <section className="panel">
          <div className="panel-header compact">
            <h3>{t.jsonView}</h3>
            <StatusBadge>
              {selected
                ? isModuleHit(selected)
                  ? moduleTypeLabel(selected)
                  : "page_analysis_v1"
                : t.selectOne}
            </StatusBadge>
          </div>
          <pre className="json-placeholder">
            {selected
              ? displayJson(selected.module_json ?? selected.page_json)
              : `{\n  "status": "${t.selectForJson}"\n}`}
          </pre>
        </section>
      </div>
      {isLargePreviewOpen && previewSrc ? (
        <div
          className="image-lightbox"
          role="dialog"
          aria-modal="true"
          aria-label={t.largePreviewTitle}
          onClick={() => setIsLargePreviewOpen(false)}
        >
          <div
            className="image-lightbox-frame"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="image-lightbox-header">
              <p className="image-lightbox-title">{selectedPreviewLabel}</p>
              <button
                type="button"
                className="image-lightbox-close"
                onClick={() => setIsLargePreviewOpen(false)}
                aria-label={t.closeLargePreview}
                title={t.closeLargePreview}
              >
                ×
              </button>
            </div>
            <div className="image-lightbox-body">
              <AnnotatedPageImage
                src={previewSrc}
                alt={selectedPreviewLabel}
                bbox={selectedBbox}
                imageClassName="image-lightbox-image"
              />
            </div>
          </div>
        </div>
      ) : null}
    </>
  );
}

export function createPreviewCacheState(): PreviewCacheState {
  return {
    entries: new Map(),
    totalBytes: 0,
  };
}

export function cachePagePreview(
  cache: PreviewCacheState,
  pageId: string,
  dataUrl: string,
  limits: PreviewCacheLimits = {
    maxEntries: PREVIEW_CACHE_ENTRY_LIMIT,
    maxBytes: PREVIEW_CACHE_BYTE_LIMIT,
  },
) {
  const existing = cache.entries.get(pageId);
  if (existing) {
    cache.entries.delete(pageId);
    cache.totalBytes -= existing.byteSize;
  }

  const byteSize = new TextEncoder().encode(dataUrl).byteLength;
  if (byteSize > limits.maxBytes || limits.maxEntries <= 0) {
    return;
  }

  cache.entries.set(pageId, { dataUrl, byteSize });
  cache.totalBytes += byteSize;
  while (
    cache.entries.size > limits.maxEntries ||
    cache.totalBytes > limits.maxBytes
  ) {
    const oldestPageId = cache.entries.keys().next().value as
      | string
      | undefined;
    if (oldestPageId === undefined) {
      break;
    }
    const oldest = cache.entries.get(oldestPageId);
    cache.entries.delete(oldestPageId);
    cache.totalBytes -= oldest?.byteSize ?? 0;
  }
}

export function loadPagePreviewOnce(
  cache: PreviewCacheState,
  requests: Map<string, Promise<string | null>>,
  pageId: string,
  loader: () => Promise<string | null>,
): Promise<string | null> {
  const cached = getCachedPagePreview(cache, pageId);
  if (cached) {
    return Promise.resolve(cached);
  }

  const existing = requests.get(pageId);
  if (existing) {
    return existing;
  }

  let request: Promise<string | null>;
  request = Promise.resolve()
    .then(loader)
    .then((dataUrl) => {
      if (dataUrl) {
        cachePagePreview(cache, pageId, dataUrl);
      }
      return dataUrl;
    })
    .finally(() => {
      if (requests.get(pageId) === request) {
        requests.delete(pageId);
      }
    });
  requests.set(pageId, request);
  return request;
}

function getCachedPagePreview(
  cache: PreviewCacheState,
  pageId: string,
): string | null {
  const cached = cache.entries.get(pageId);
  if (!cached) {
    return null;
  }
  cache.entries.delete(pageId);
  cache.entries.set(pageId, cached);
  return cached.dataUrl;
}

function clearPreviewCache(cache: PreviewCacheState) {
  cache.entries.clear();
  cache.totalBytes = 0;
}

export function AnnotatedPageImage({
  src,
  alt,
  bbox,
  imageClassName,
}: {
  src: string;
  alt: string;
  bbox: NormalizedBoundingBoxDto | null;
  imageClassName: string;
}) {
  return (
    <span className="annotated-page-image">
      <img className={imageClassName} src={src} alt={alt} />
      {bbox ? (
        <span
          className="search-bbox-overlay"
          style={{
            left: `${bbox.x * 100}%`,
            top: `${bbox.y * 100}%`,
            width: `${bbox.width * 100}%`,
            height: `${bbox.height * 100}%`,
          }}
          aria-hidden="true"
        />
      ) : null}
    </span>
  );
}

export function buildSearchResultEntries(
  items: SearchResultItemDto[],
): SearchResultEntry[] {
  return items.map((item, index) => ({
    item,
    hitId: searchHitId(item, index),
  }));
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

export function validNormalizedBbox(
  value: NormalizedBoundingBoxDto | null | undefined,
): NormalizedBoundingBoxDto | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  const { x, y, width, height } = value;
  if (![x, y, width, height].every(Number.isFinite)) {
    return null;
  }
  const tolerance = 0.000001;
  const rawRight = x + width;
  const rawBottom = y + height;
  if (
    width <= 0 ||
    height <= 0 ||
    x < -tolerance ||
    y < -tolerance ||
    rawRight > 1 + tolerance ||
    rawBottom > 1 + tolerance
  ) {
    return null;
  }
  const clampedX = Math.max(0, Math.min(1, x));
  const clampedY = Math.max(0, Math.min(1, y));
  const right = Math.max(0, Math.min(1, rawRight));
  const bottom = Math.max(0, Math.min(1, rawBottom));
  const clampedWidth = right - clampedX;
  const clampedHeight = bottom - clampedY;
  if (clampedWidth <= 0 || clampedHeight <= 0) {
    return null;
  }
  return {
    x: clampedX,
    y: clampedY,
    width: clampedWidth,
    height: clampedHeight,
  };
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
  if (typeof item.snippet === "string" && item.snippet.trim()) {
    return item.snippet.trim();
  }
  return item.summary?.trim() || null;
}

function previewLabel(item: SearchResultItemDto): string {
  const pageLabel = `${item.original_filename ?? t.unknownDoc}，第 ${item.page_number} 页`;
  return isModuleHit(item)
    ? `${pageLabel}，${moduleTypeLabel(item)}`
    : item.title?.trim() || pageLabel;
}

function displayJson(value: unknown): string {
  if (typeof value === "string") {
    try {
      const parsed = JSON.parse(value) as unknown;
      return JSON.stringify(parsed, null, 2);
    } catch {
      return value;
    }
  }
  try {
    return JSON.stringify(value, null, 2) ?? String(value);
  } catch {
    return String(value);
  }
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
    `已索引 ${status.indexed_page_count} 个内容项`,
    `可索引 ${status.analyzable_page_count} 个内容项`,
  ];
  if (status.pending_index_page_count > 0) {
    parts.push(`${status.pending_index_page_count} 个内容项待纳入`);
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
