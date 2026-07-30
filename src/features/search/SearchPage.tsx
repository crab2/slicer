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
const PREVIEW_REQUEST_TIMEOUT_MS = 30_000;

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

export interface PagePreviewState {
  pageId: string;
  dataUrl: string | null;
  isLoading: boolean;
  error: string | null;
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
  const [previewState, setPreviewState] = useState<PagePreviewState | null>(null);
  const [previewRetryVersion, setPreviewRetryVersion] = useState(0);
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
  const previewTriggerRef = useRef<HTMLButtonElement>(null);
  const largePreviewCloseRef = useRef<HTMLButtonElement>(null);
  const wasLargePreviewOpen = useRef(false);

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

  useEffect(() => {
    if (!workspaceReady) {
      previewGeneration.current += 1;
      clearPreviewCache(previewCache.current);
      previewRequests.current.clear();
    }
  }, [workspaceReady]);

  useEffect(() => {
    const pageId = selected?.page_id;
    if (!workspaceReady || !pageId) {
      setPreviewState(null);
      return;
    }

    let cancelled = false;
    const cached = getCachedPagePreview(previewCache.current, pageId);
    if (cached) {
      setPreviewState({ pageId, dataUrl: cached, isLoading: false, error: null });
      return;
    }

    const generation = previewGeneration.current;
    const request = loadPagePreviewOnce(
      previewCache.current,
      previewRequests.current,
      pageId,
      async () => {
        const dataUrl = await withTimeout(
          tauriClient.getPageImagePreview(pageId),
          PREVIEW_REQUEST_TIMEOUT_MS,
          t.previewTimeout,
        );
        return generation === previewGeneration.current ? dataUrl : null;
      },
    );

    setPreviewState({ pageId, dataUrl: null, isLoading: true, error: null });
    request
      .then((dataUrl) => {
        if (cancelled) return;
        setPreviewState({
          pageId,
          dataUrl,
          isLoading: false,
          error: dataUrl ? null : t.imageMissing,
        });
      })
      .catch((error) => {
        if (!cancelled) {
          setPreviewState({
            pageId,
            dataUrl: null,
            isLoading: false,
            error: extractError(error).message,
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [workspaceReady, selected?.page_id, previewRetryVersion]);

  useEffect(() => {
    setIsLargePreviewOpen(false);
  }, [selectedHitId]);

  useEffect(() => {
    if (!isLargePreviewOpen) {
      if (wasLargePreviewOpen.current) {
        previewTriggerRef.current?.focus();
      }
      wasLargePreviewOpen.current = false;
      return;
    }
    wasLargePreviewOpen.current = true;
    largePreviewCloseRef.current?.focus();
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setIsLargePreviewOpen(false);
      } else if (event.key === "Tab") {
        event.preventDefault();
        largePreviewCloseRef.current?.focus();
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
  const selectedPreviewLabel = selected ? previewLabel(selected) : "";
  const selectedType = selected ? moduleTypeLabel(selected) : "";
  const selectedJson = selected ? searchResultJson(selected) : null;
  const selectedPreview = pagePreviewForSelection(previewState, selected?.page_id ?? null);
  const handlePreviewImageError = useCallback(() => {
    const pageId = selected?.page_id;
    if (!pageId) return;
    removeCachedPagePreview(previewCache.current, pageId);
    setPreviewState((current) =>
      current?.pageId === pageId
        ? { pageId, dataUrl: null, isLoading: false, error: t.imageInvalid }
        : current,
    );
    setIsLargePreviewOpen(false);
  }, [selected?.page_id]);

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

        <div className="search-viewer-column search-hit-viewer">
          <section className="search-hit-pane" aria-label="搜索命中预览">
            <header className="search-hit-pane-header">
              <div>
                <p className="eyebrow">命中预览</p>
                <h3>{selected ? `第 ${selected.page_number} 页` : "待选择"}</h3>
              </div>
              <StatusBadge tone={selectedBbox ? "success" : "neutral"}>
                {selected ? (selectedBbox ? t.located : t.notLocated) : t.selectOne}
              </StatusBadge>
            </header>
            <div className="search-hit-preview-content">
              {!selected ? (
                <EmptyState title="选择搜索结果" description="选择后显示命中页面。" />
              ) : !selectedPreview || selectedPreview.isLoading ? (
                <p className="muted-copy">{t.previewLoading}</p>
              ) : selectedPreview.dataUrl ? (
                <button
                  type="button"
                  className="search-preview-button"
                  ref={previewTriggerRef}
                  onClick={() => setIsLargePreviewOpen(true)}
                  aria-label={`${t.openLargePreview}：${selectedPreviewLabel}`}
                  title={t.openLargePreview}
                >
                  <AnnotatedPageImage
                    src={selectedPreview.dataUrl}
                    alt={selectedPreviewLabel}
                    bbox={selectedBbox}
                    imageClassName="search-preview-image"
                    onError={handlePreviewImageError}
                  />
                </button>
              ) : (
                <div className="search-preview-error">
                  <EmptyState
                    title="页面预览不可用"
                    description={selectedPreview.error ?? t.imageMissing}
                  />
                  <Button onClick={() => setPreviewRetryVersion((version) => version + 1)}>
                    {t.retryPreview}
                  </Button>
                </div>
              )}
            </div>
          </section>

          <section className="search-hit-pane" aria-label="召回数据">
            <header className="search-hit-pane-header">
              <div>
                <p className="eyebrow">召回数据</p>
                <h3 title={selectedType}>{selected ? selectedType : "待选择"}</h3>
              </div>
              <StatusBadge>{selected ? `第 ${selected.page_number} 页` : t.selectOne}</StatusBadge>
            </header>
            <div className="search-hit-json-content">
              {selected && selectedJson !== null ? (
                <pre className="document-viewer-source search-hit-json-source">
                  <code>{displayJson(selectedJson)}</code>
                </pre>
              ) : selected ? (
                <EmptyState title="暂无召回数据" description="当前结果没有可展示的数据。" />
              ) : (
                <EmptyState title="选择搜索结果" description="选择后显示对应数据块。" />
              )}
            </div>
          </section>
        </div>
      </div>

      {isLargePreviewOpen && selectedPreview?.dataUrl ? (
        <div
          className="image-lightbox"
          role="dialog"
          aria-modal="true"
          aria-label={t.largePreviewTitle}
          onClick={() => setIsLargePreviewOpen(false)}
        >
          <div className="image-lightbox-frame" onClick={(event) => event.stopPropagation()}>
            <div className="image-lightbox-header">
              <p className="image-lightbox-title">{selectedPreviewLabel}</p>
              <button
                type="button"
                className="image-lightbox-close"
                ref={largePreviewCloseRef}
                onClick={() => setIsLargePreviewOpen(false)}
                aria-label={t.closeLargePreview}
                title={t.closeLargePreview}
              >
                ×
              </button>
            </div>
            <div className="image-lightbox-body">
              <AnnotatedPageImage
                src={selectedPreview.dataUrl}
                alt={selectedPreviewLabel}
                bbox={selectedBbox}
                imageClassName="image-lightbox-image"
                onError={handlePreviewImageError}
              />
            </div>
          </div>
        </div>
      ) : null}
    </>
  );
}

export function createPreviewCacheState(): PreviewCacheState {
  return { entries: new Map(), totalBytes: 0 };
}

export function pagePreviewForSelection(
  preview: PagePreviewState | null,
  pageId: string | null,
): PagePreviewState | null {
  return preview?.pageId === pageId ? preview : null;
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
  const byteSize = new TextEncoder().encode(dataUrl).byteLength;
  if (byteSize > limits.maxBytes || limits.maxEntries <= 0) return;

  const existing = cache.entries.get(pageId);
  if (existing) {
    cache.entries.delete(pageId);
    cache.totalBytes -= existing.byteSize;
  }

  cache.entries.set(pageId, { dataUrl, byteSize });
  cache.totalBytes += byteSize;
  while (cache.entries.size > limits.maxEntries || cache.totalBytes > limits.maxBytes) {
    const oldestPageId = cache.entries.keys().next().value as string | undefined;
    if (oldestPageId === undefined) break;
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
  if (cached) return Promise.resolve(cached);

  const existing = requests.get(pageId);
  if (existing) return existing;

  let request: Promise<string | null>;
  request = Promise.resolve()
    .then(loader)
    .then((dataUrl) => {
      if (dataUrl) cachePagePreview(cache, pageId, dataUrl);
      return dataUrl;
    })
    .finally(() => {
      if (requests.get(pageId) === request) requests.delete(pageId);
    });
  requests.set(pageId, request);
  return request;
}

function getCachedPagePreview(cache: PreviewCacheState, pageId: string): string | null {
  const cached = cache.entries.get(pageId);
  if (!cached) return null;
  cache.entries.delete(pageId);
  cache.entries.set(pageId, cached);
  return cached.dataUrl;
}

function clearPreviewCache(cache: PreviewCacheState) {
  cache.entries.clear();
  cache.totalBytes = 0;
}

function removeCachedPagePreview(cache: PreviewCacheState, pageId: string) {
  const cached = cache.entries.get(pageId);
  if (!cached) return;
  cache.entries.delete(pageId);
  cache.totalBytes -= cached.byteSize;
}

export function AnnotatedPageImage({
  src,
  alt,
  bbox,
  imageClassName,
  onError,
}: {
  src: string;
  alt: string;
  bbox: NormalizedBoundingBoxDto | null;
  imageClassName: string;
  onError?: () => void;
}) {
  return (
    <span className="annotated-page-image">
      <img className={imageClassName} src={src} alt={alt} onError={onError} />
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

export function validNormalizedBbox(
  value: NormalizedBoundingBoxDto | null | undefined,
): NormalizedBoundingBoxDto | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const { x, y, width, height } = value;
  if (![x, y, width, height].every(Number.isFinite)) return null;
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
  if (right <= clampedX || bottom <= clampedY) return null;
  return {
    x: clampedX,
    y: clampedY,
    width: right - clampedX,
    height: bottom - clampedY,
  };
}

export function searchResultJson(item: SearchResultItemDto): unknown | null {
  if (
    item.module_json !== null &&
    item.module_json !== undefined &&
    (typeof item.module_json !== "string" || item.module_json.trim())
  ) {
    return item.module_json;
  }
  if (isModuleHit(item)) return null;
  return item.page_json?.trim() ? item.page_json : null;
}

export function withTimeout<T>(
  promise: Promise<T>,
  timeoutMs: number,
  message: string,
): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = globalThis.setTimeout(() => reject(new Error(message)), timeoutMs);
    promise.then(
      (value) => {
        globalThis.clearTimeout(timer);
        resolve(value);
      },
      (error: unknown) => {
        globalThis.clearTimeout(timer);
        reject(error);
      },
    );
  });
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

function previewLabel(item: SearchResultItemDto): string {
  const pageLabel = `${item.original_filename ?? t.unknownDoc}，第 ${item.page_number} 页`;
  return isModuleHit(item)
    ? `${pageLabel}，${moduleTypeLabel(item)}`
    : item.title?.trim() || pageLabel;
}

export function displayJson(value: unknown): string {
  if (typeof value === "string") {
    try {
      return JSON.stringify(JSON.parse(value) as unknown, null, 2);
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
