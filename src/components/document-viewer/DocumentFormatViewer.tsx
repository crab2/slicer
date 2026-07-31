import {
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
  type ReactNode,
} from "react";
import { Button } from "../common/Button";
import { EmptyState } from "../common/EmptyState";
import { tauriClient } from "../../lib/tauriClient";
import type {
  DocumentViewerAssetDto,
  DocumentViewerContentDto,
  DocumentViewerFormat,
  DocumentViewerManifestDto,
  DocumentViewerPagePreviewDto,
  NormalizedBoundingBoxDto,
  PdfPageGeometryDto,
} from "../../types/app";

const ANNOT_PREVIEW_CACHE_LIMIT = 8;
const ANNOT_ZOOM_MIN = 0.6;
const ANNOT_ZOOM_MAX = 2;
const ANNOT_ZOOM_STEP = 0.2;

const VIEWER_FORMATS: DocumentViewerFormat[] = [
  "pdf",
  "annot",
  "preview",
  "html",
  "md",
  "json",
];

const FORMAT_LABELS: Record<DocumentViewerFormat, string> = {
  pdf: "PDF",
  annot: "Annot",
  preview: "Preview",
  html: "HTML",
  md: "MD",
  json: "JSON",
};

type ViewerPane = "left" | "right";

interface DocumentFormatViewerProps {
  documentId: string | null;
  documentTitle?: string | null;
  pageNumber?: number | null;
  refreshKey?: number;
}

interface ViewerRequestGeneration {
  current: () => number;
  next: () => number;
  isCurrent: (token: number) => boolean;
}

export interface OdlJsonBlock {
  key: string;
  pageNumber: number;
  value: Record<string, unknown>;
  rawBoundingBox: [number, number, number, number];
}

export interface ParsedOdlJson {
  value: unknown;
  blocks: OdlJsonBlock[];
  formatted: string;
  ranges: Map<string, { start: number; end: number }>;
}

export function DocumentFormatViewer({
  documentId,
  documentTitle,
  pageNumber,
  refreshKey = 0,
}: DocumentFormatViewerProps) {
  const [manifest, setManifest] = useState<DocumentViewerManifestDto | null>(null);
  const [manifestLoading, setManifestLoading] = useState(false);
  const [manifestError, setManifestError] = useState<{
    documentId: string;
    message: string;
  } | null>(null);
  const [leftFormat, setLeftFormat] = useState<DocumentViewerFormat>("annot");
  const [rightFormat, setRightFormat] = useState<DocumentViewerFormat>("json");
  const [contentByFormat, setContentByFormat] = useState<
    Partial<Record<DocumentViewerFormat, DocumentViewerContentDto>>
  >({});
  const [loadingFormats, setLoadingFormats] = useState<Set<DocumentViewerFormat>>(new Set());
  const [errorsByFormat, setErrorsByFormat] = useState<
    Partial<Record<DocumentViewerFormat, string>>
  >({});
  const [manifestReloadVersion, setManifestReloadVersion] = useState(0);
  const [selectedJsonBlockKey, setSelectedJsonBlockKey] = useState<string | null>(null);
  const requestGeneration = useRef(createViewerRequestGeneration());
  const pendingRequests = useRef(
    new Map<DocumentViewerFormat, Promise<DocumentViewerContentDto>>(),
  );
  const activeManifest = manifest?.document_id === documentId ? manifest : null;
  const activeManifestError =
    manifestError?.documentId === documentId ? manifestError.message : null;

  useEffect(() => {
    const token = requestGeneration.current.next();
    pendingRequests.current.clear();
    setManifest(null);
    setManifestError(null);
    setContentByFormat({});
    setLoadingFormats(new Set());
    setErrorsByFormat({});
    setSelectedJsonBlockKey(null);
    setLeftFormat("annot");
    setRightFormat("json");
    if (!documentId) {
      setManifestLoading(false);
      return;
    }

    setManifestLoading(true);
    tauriClient
      .getDocumentViewerManifest(documentId)
      .then((nextManifest) => {
        if (requestGeneration.current.isCurrent(token)) {
          setManifest(nextManifest);
        }
      })
      .catch((error) => {
        if (requestGeneration.current.isCurrent(token)) {
          setManifestError({ documentId, message: extractErrorMessage(error) });
        }
      })
      .finally(() => {
        if (requestGeneration.current.isCurrent(token)) {
          setManifestLoading(false);
        }
      });
  }, [documentId, manifestReloadVersion, refreshKey]);

  useEffect(() => {
    if (!documentId || !activeManifest) {
      return;
    }
    const token = requestGeneration.current.current();
    const formats = new Set([leftFormat, rightFormat]);
    if (formats.has("annot")) {
      formats.add("json");
    }
    for (const format of formats) {
      if (format === "annot") {
        continue;
      }
      if (!isFormatAvailable(activeManifest, format) || contentByFormat[format]) {
        continue;
      }
      let request = pendingRequests.current.get(format);
      if (!request) {
        request = tauriClient.getDocumentViewerContent(documentId, format);
        pendingRequests.current.set(format, request);
      }
      const activeRequest = request;
      setLoadingFormats((current) => withSetValue(current, format, true));
      setErrorsByFormat((current) => ({ ...current, [format]: undefined }));
      activeRequest
        .then((content) => {
          if (requestGeneration.current.isCurrent(token)) {
            setContentByFormat((current) => ({ ...current, [format]: content }));
          }
        })
        .catch((error) => {
          if (requestGeneration.current.isCurrent(token)) {
            setErrorsByFormat((current) => ({
              ...current,
              [format]: extractErrorMessage(error),
            }));
          }
        })
        .finally(() => {
          if (pendingRequests.current.get(format) === activeRequest) {
            pendingRequests.current.delete(format);
          }
          if (requestGeneration.current.isCurrent(token)) {
            setLoadingFormats((current) => withSetValue(current, format, false));
          }
        });
    }
  }, [activeManifest, contentByFormat, documentId, leftFormat, rightFormat]);

  const viewerTitle = activeManifest?.original_filename || documentTitle || "文档查看";
  const page = normalizePageNumber(pageNumber);
  const parsedJson = useMemo(
    () => parseOdlJson(contentByFormat.json?.content),
    [contentByFormat.json?.content],
  );

  function handleBlockSelect(pane: ViewerPane, blockKey: string) {
    setSelectedJsonBlockKey(blockKey);
    if (pane === "left") {
      setRightFormat("json");
    } else {
      setLeftFormat("json");
    }
  }

  return (
    <section className="document-format-viewer" aria-label={`${viewerTitle} 格式查看器`}>
      <header className="document-viewer-header">
        <div>
          <p className="eyebrow">文档查看</p>
          <h3 title={viewerTitle}>{viewerTitle}</h3>
        </div>
        <p className="document-viewer-page">
          {page ? `定位第 ${page} 页` : activeManifest?.page_count ? `共 ${activeManifest.page_count} 页` : "完整文档"}
        </p>
      </header>

      <div className="document-viewer-grid">
        <ViewerPaneView
          pane="left"
          activeFormat={leftFormat}
          onFormatChange={setLeftFormat}
          manifest={activeManifest}
          manifestLoading={manifestLoading}
          manifestError={activeManifestError}
          documentId={documentId}
          pageNumber={page}
          content={contentByFormat[leftFormat]}
          parsedJson={parsedJson}
          loading={loadingFormats.has(leftFormat)}
          jsonLoading={loadingFormats.has("json")}
          error={errorsByFormat[leftFormat]}
          jsonError={errorsByFormat.json}
          selectedJsonBlockKey={selectedJsonBlockKey}
          onBlockSelect={(blockKey) => handleBlockSelect("left", blockKey)}
          onManifestRetry={() => setManifestReloadVersion((current) => current + 1)}
        />
        <ViewerPaneView
          pane="right"
          activeFormat={rightFormat}
          onFormatChange={setRightFormat}
          manifest={activeManifest}
          manifestLoading={manifestLoading}
          manifestError={activeManifestError}
          documentId={documentId}
          pageNumber={page}
          content={contentByFormat[rightFormat]}
          parsedJson={parsedJson}
          loading={loadingFormats.has(rightFormat)}
          jsonLoading={loadingFormats.has("json")}
          error={errorsByFormat[rightFormat]}
          jsonError={errorsByFormat.json}
          selectedJsonBlockKey={selectedJsonBlockKey}
          onBlockSelect={(blockKey) => handleBlockSelect("right", blockKey)}
          onManifestRetry={() => setManifestReloadVersion((current) => current + 1)}
        />
      </div>
    </section>
  );
}

function ViewerPaneView({
  pane,
  activeFormat,
  onFormatChange,
  manifest,
  manifestLoading,
  manifestError,
  documentId,
  pageNumber,
  content,
  parsedJson,
  loading,
  jsonLoading,
  error,
  jsonError,
  selectedJsonBlockKey,
  onBlockSelect,
  onManifestRetry,
}: {
  pane: ViewerPane;
  activeFormat: DocumentViewerFormat;
  onFormatChange: (format: DocumentViewerFormat) => void;
  manifest: DocumentViewerManifestDto | null;
  manifestLoading: boolean;
  manifestError: string | null;
  documentId: string | null;
  pageNumber: number | null;
  content?: DocumentViewerContentDto;
  parsedJson: ParsedOdlJson | null;
  loading: boolean;
  jsonLoading: boolean;
  error?: string;
  jsonError?: string;
  selectedJsonBlockKey: string | null;
  onBlockSelect: (blockKey: string) => void;
  onManifestRetry: () => void;
}) {
  const available = manifest ? isFormatAvailable(manifest, activeFormat) : false;
  const paneId = useId();
  const panelId = `${paneId}-panel`;

  return (
    <div className="document-viewer-pane" data-pane={pane}>
      <div className="document-viewer-tabs" role="tablist" aria-label={`${pane === "left" ? "左" : "右"}栏格式`}>
        {VIEWER_FORMATS.map((format) => {
          const formatAvailable = manifest ? isFormatAvailable(manifest, format) : false;
          return (
            <button
              key={format}
              type="button"
              role="tab"
              className="document-viewer-tab"
              data-active={activeFormat === format}
              data-available={formatAvailable}
              data-format={format}
              id={`${paneId}-${format}`}
              tabIndex={activeFormat === format ? 0 : -1}
              aria-selected={activeFormat === format}
              aria-controls={panelId}
              onClick={() => onFormatChange(format)}
              onKeyDown={(event) => handleTabKeyDown(event, format, onFormatChange)}
              title={formatAvailable ? `查看 ${FORMAT_LABELS[format]}` : `${FORMAT_LABELS[format]} 制品不可用`}
            >
              {FORMAT_LABELS[format]}
            </button>
          );
        })}
      </div>
      <div
        className="document-viewer-content"
        role="tabpanel"
        id={panelId}
        aria-labelledby={`${paneId}-${activeFormat}`}
      >
        {!documentId ? (
          <EmptyState title="选择文档" description="选择搜索结果或媒体后可查看六种文档格式。" />
        ) : manifestLoading ? (
          <ViewerNotice>正在读取文档格式...</ViewerNotice>
        ) : manifestError ? (
          <ViewerError message={manifestError} onRetry={onManifestRetry} />
        ) : !manifest ? (
          <ViewerNotice>文档格式信息不可用。</ViewerNotice>
        ) : !available ? (
          <EmptyState
            title={`${FORMAT_LABELS[activeFormat]} 不可用`}
            description="此文档没有登记该格式的制品，不会在查看时重新解析。"
          />
        ) : activeFormat === "annot" ? (
          <InteractiveAnnotContent
            documentId={documentId}
            pageCount={manifest.page_count}
            initialPageNumber={pageNumber}
            parsedJson={parsedJson}
            jsonLoading={jsonLoading}
            jsonError={jsonError}
            selectedJsonBlockKey={selectedJsonBlockKey}
            onBlockSelect={onBlockSelect}
          />
        ) : error ? (
          <ViewerError message={error} />
        ) : loading || !content ? (
          <ViewerNotice>正在加载 {FORMAT_LABELS[activeFormat]}...</ViewerNotice>
        ) : (
          <FormatContent
            content={content}
            pageNumber={pageNumber}
            parsedJson={parsedJson}
            selectedJsonBlockKey={selectedJsonBlockKey}
          />
        )}
      </div>
    </div>
  );
}

function FormatContent({
  content,
  pageNumber,
  parsedJson,
  selectedJsonBlockKey,
}: {
  content: DocumentViewerContentDto;
  pageNumber: number | null;
  parsedJson: ParsedOdlJson | null;
  selectedJsonBlockKey: string | null;
}) {
  if (content.format === "pdf") {
    return <PdfContent content={content} pageNumber={pageNumber} />;
  }
  if (content.format === "preview") {
    return (
      <iframe
        className="document-viewer-frame document-viewer-preview"
        title="文档 Preview"
        sandbox=""
        srcDoc={createSandboxedPreviewHtml(content.content, content.assets)}
      />
    );
  }
  if (content.format === "json") {
    return (
      <JsonSourceContent
        content={content.content}
        parsedJson={parsedJson}
        selectedJsonBlockKey={selectedJsonBlockKey}
      />
    );
  }
  return (
    <pre className="document-viewer-source">
      <code>{formatSourceContent(content)}</code>
    </pre>
  );
}

function PdfContent({
  content,
  pageNumber,
}: {
  content: DocumentViewerContentDto;
  pageNumber: number | null;
}) {
  const objectUrl = useMemo(() => createPdfObjectUrl(content), [content]);
  useEffect(
    () => () => {
      if (objectUrl.startsWith("blob:")) {
        URL.revokeObjectURL(objectUrl);
      }
    },
    [objectUrl],
  );
  const pageFragment = pageNumber ? `#page=${pageNumber}` : "";
  return (
    <iframe
      className="document-viewer-frame document-viewer-pdf"
      title={`${FORMAT_LABELS[content.format]} 文档`}
      src={`${objectUrl}${pageFragment}`}
    />
  );
}

interface AnnotPreviewState {
  pageNumber: number;
  preview: DocumentViewerPagePreviewDto | null;
  loading: boolean;
  error: { message: string; retryable: boolean } | null;
}

interface InteractiveOdlBlock extends OdlJsonBlock {
  bbox: NormalizedBoundingBoxDto;
}

interface HoveredOdlBlock {
  block: InteractiveOdlBlock;
  left: number;
  top: number;
}

function InteractiveAnnotContent({
  documentId,
  pageCount,
  initialPageNumber,
  parsedJson,
  jsonLoading,
  jsonError,
  selectedJsonBlockKey,
  onBlockSelect,
}: {
  documentId: string;
  pageCount: number | null;
  initialPageNumber: number | null;
  parsedJson: ParsedOdlJson | null;
  jsonLoading: boolean;
  jsonError?: string;
  selectedJsonBlockKey: string | null;
  onBlockSelect: (blockKey: string) => void;
}) {
  const normalizedPageCount = normalizePageCount(pageCount);
  const initialPage = clampPageNumber(initialPageNumber ?? 1, normalizedPageCount);
  const [currentPage, setCurrentPage] = useState(initialPage);
  const [zoom, setZoom] = useState(1);
  const [retryVersion, setRetryVersion] = useState(0);
  const [previewState, setPreviewState] = useState<AnnotPreviewState | null>(null);
  const [hoveredBlock, setHoveredBlock] = useState<HoveredOdlBlock | null>(null);
  const previewCache = useRef(new Map<string, DocumentViewerPagePreviewDto>());
  const requestGeneration = useRef(0);
  const tooltipHideTimer = useRef<number | null>(null);
  const tooltipPreRef = useRef<HTMLPreElement>(null);
  const tooltipId = useId();
  const pageRequestKey = `${tooltipId}-page`;

  useEffect(() => {
    requestGeneration.current += 1;
    previewCache.current.clear();
    setCurrentPage(initialPage);
    setZoom(1);
    setPreviewState(null);
    setHoveredBlock(null);
  }, [documentId, initialPage]);

  useEffect(
    () => () => {
      if (tooltipHideTimer.current !== null) {
        window.clearTimeout(tooltipHideTimer.current);
      }
    },
    [],
  );

  useEffect(() => {
    const cacheKey = `${documentId}:${currentPage}`;
    const cached = getCachedAnnotPreview(previewCache.current, cacheKey);
    if (cached) {
      setPreviewState({ pageNumber: currentPage, preview: cached, loading: false, error: null });
      return;
    }

    let cancelled = false;
    const generation = ++requestGeneration.current;
    setPreviewState({ pageNumber: currentPage, preview: null, loading: true, error: null });
    const requestTimer = window.setTimeout(() => {
      tauriClient
        .getDocumentViewerPagePreview(documentId, currentPage, pageRequestKey)
        .then((preview) => {
          if (cancelled || generation !== requestGeneration.current) return;
          cacheAnnotPreview(previewCache.current, cacheKey, preview);
          setPreviewState({ pageNumber: currentPage, preview, loading: false, error: null });
        })
        .catch((error) => {
          if (cancelled || generation !== requestGeneration.current) return;
          setPreviewState({
            pageNumber: currentPage,
            preview: null,
            loading: false,
            error: extractViewerError(error),
          });
        });
    }, 100);

    return () => {
      cancelled = true;
      window.clearTimeout(requestTimer);
    };
  }, [currentPage, documentId, pageRequestKey, retryVersion]);

  const currentPreview =
    previewState?.pageNumber === currentPage ? previewState : null;
  const interactiveBlocks = useMemo(() => {
    const geometry = currentPreview?.preview?.geometry;
    if (!geometry || !parsedJson) return [];
    return parsedJson.blocks
      .filter((block) => block.pageNumber === currentPage)
      .flatMap<InteractiveOdlBlock>((block) => {
        const bbox = normalizePdfBoundingBox(block.rawBoundingBox, geometry);
        return bbox ? [{ ...block, bbox }] : [];
      });
  }, [currentPage, currentPreview?.preview?.geometry, parsedJson]);
  const hoveredBlockJson = useMemo(
    () => (hoveredBlock ? JSON.stringify(hoveredBlock.block.value, null, 2) : ""),
    [hoveredBlock?.block.key],
  );

  function changePage(nextPage: number) {
    cancelTooltipHide();
    setHoveredBlock(null);
    setCurrentPage(clampPageNumber(nextPage, normalizedPageCount));
  }

  function showPointerTooltip(block: InteractiveOdlBlock, event: MouseEvent<HTMLButtonElement>) {
    cancelTooltipHide();
    const position = tooltipPosition(event.clientX, event.clientY);
    setHoveredBlock({ block, ...position });
  }

  function hideTooltip(blockKey: string) {
    cancelTooltipHide();
    tooltipHideTimer.current = window.setTimeout(() => {
      setHoveredBlock((current) => (current?.block.key === blockKey ? null : current));
      tooltipHideTimer.current = null;
    }, 120);
  }

  function cancelTooltipHide() {
    if (tooltipHideTimer.current !== null) {
      window.clearTimeout(tooltipHideTimer.current);
      tooltipHideTimer.current = null;
    }
  }

  return (
    <div className="document-viewer-annot">
      <div className="document-viewer-annot-toolbar" role="toolbar" aria-label="Annot 页面工具栏">
        <button
          type="button"
          className="document-viewer-icon-button"
          onClick={() => changePage(currentPage - 1)}
          disabled={currentPage <= 1}
          aria-label="上一页"
          title="上一页"
        >
          ‹
        </button>
        <input
          className="document-viewer-page-input"
          type="number"
          min={1}
          max={normalizedPageCount ?? undefined}
          value={currentPage}
          onChange={(event) => {
            const value = Number(event.currentTarget.value);
            if (Number.isFinite(value) && value >= 1) changePage(value);
          }}
          aria-label="当前页码"
        />
        <span className="document-viewer-page-total">/ {normalizedPageCount ?? "?"}</span>
        <button
          type="button"
          className="document-viewer-icon-button"
          onClick={() => changePage(currentPage + 1)}
          disabled={normalizedPageCount !== null && currentPage >= normalizedPageCount}
          aria-label="下一页"
          title="下一页"
        >
          ›
        </button>
        <span className="document-viewer-toolbar-spacer" />
        <button
          type="button"
          className="document-viewer-icon-button"
          onClick={() => setZoom((value) => clampZoom(value - ANNOT_ZOOM_STEP))}
          disabled={zoom <= ANNOT_ZOOM_MIN}
          aria-label="缩小"
          title="缩小"
        >
          −
        </button>
        <output className="document-viewer-zoom-value" aria-label="当前缩放比例">
          {Math.round(zoom * 100)}%
        </output>
        <button
          type="button"
          className="document-viewer-icon-button"
          onClick={() => setZoom((value) => clampZoom(value + ANNOT_ZOOM_STEP))}
          disabled={zoom >= ANNOT_ZOOM_MAX}
          aria-label="放大"
          title="放大"
        >
          +
        </button>
        {jsonLoading ? (
          <span className="document-viewer-annot-status">JSON 加载中</span>
        ) : jsonError ? (
          <span className="document-viewer-annot-status" title={jsonError}>JSON 不可用</span>
        ) : null}
      </div>

      <div className="document-viewer-annot-stage">
        {!currentPreview || currentPreview.loading ? (
          <ViewerNotice>正在渲染第 {currentPage} 页...</ViewerNotice>
        ) : currentPreview.error ? (
          <ViewerError
            message={currentPreview.error.message}
            onRetry={
              currentPreview.error.retryable
                ? () => setRetryVersion((version) => version + 1)
                : undefined
            }
          />
        ) : currentPreview.preview ? (
          <div className="document-viewer-annot-page" style={{ width: `${zoom * 100}%` }}>
            <img
              src={currentPreview.preview.data_url}
              alt={`Annot 第 ${currentPage} 页`}
              draggable="false"
            />
            <div className="document-viewer-annot-hotspots">
              {interactiveBlocks.map((block) => (
                <button
                  key={block.key}
                  type="button"
                  className="document-viewer-annot-hotspot"
                  data-selected={selectedJsonBlockKey === block.key}
                  style={{
                    left: `${block.bbox.x * 100}%`,
                    top: `${block.bbox.y * 100}%`,
                    width: `${block.bbox.width * 100}%`,
                    height: `${block.bbox.height * 100}%`,
                  }}
                  aria-label={blockAriaLabel(block)}
                  aria-describedby={hoveredBlock?.block.key === block.key ? tooltipId : undefined}
                  onMouseEnter={(event) => showPointerTooltip(block, event)}
                  onMouseLeave={() => hideTooltip(block.key)}
                  onFocus={(event) => {
                    const rect = event.currentTarget.getBoundingClientRect();
                    setHoveredBlock({
                      block,
                      ...tooltipPosition(rect.right, rect.top + rect.height / 2),
                    });
                  }}
                  onBlur={() => hideTooltip(block.key)}
                  onKeyDown={(event) => handleTooltipScrollKey(event, tooltipPreRef.current)}
                  onClick={() => onBlockSelect(block.key)}
                />
              ))}
            </div>
          </div>
        ) : null}
      </div>

      {hoveredBlock ? (
        <div
          className="document-viewer-json-tooltip"
          id={tooltipId}
          role="tooltip"
          style={{ left: hoveredBlock.left, top: hoveredBlock.top }}
          onMouseEnter={cancelTooltipHide}
          onMouseLeave={() => hideTooltip(hoveredBlock.block.key)}
        >
          <div className="document-viewer-json-tooltip-header">
            <strong>{blockTypeLabel(hoveredBlock.block)}</strong>
            <span>第 {hoveredBlock.block.pageNumber} 页</span>
          </div>
          <pre ref={tooltipPreRef}>{hoveredBlockJson}</pre>
        </div>
      ) : null}
    </div>
  );
}

function JsonSourceContent({
  content,
  parsedJson,
  selectedJsonBlockKey,
}: {
  content: string;
  parsedJson: ParsedOdlJson | null;
  selectedJsonBlockKey: string | null;
}) {
  const sourceRef = useRef<HTMLPreElement>(null);
  const selectedBlockRef = useRef<HTMLSpanElement>(null);
  const selectedRange = selectedJsonBlockKey
    ? parsedJson?.ranges.get(selectedJsonBlockKey) ?? null
    : null;
  const selectionMissing = Boolean(selectedJsonBlockKey && parsedJson && !selectedRange);

  useEffect(() => {
    if (!selectedRange) return;
    const frame = window.requestAnimationFrame(() => {
      const source = sourceRef.current;
      const target = selectedBlockRef.current;
      if (!source || !target) return;
      const sourceRect = source.getBoundingClientRect();
      const targetRect = target.getBoundingClientRect();
      source.scrollTo({
        top: Math.max(0, source.scrollTop + targetRect.top - sourceRect.top - source.clientHeight / 3),
        behavior: "smooth",
      });
    });
    return () => window.cancelAnimationFrame(frame);
  }, [selectedJsonBlockKey, selectedRange]);

  if (!parsedJson) {
    return (
      <pre className="document-viewer-source">
        <code>{content}</code>
      </pre>
    );
  }

  return (
    <div className="document-viewer-json-source-wrap">
      {selectionMissing ? (
        <p className="document-viewer-json-link-notice" role="status">
          未找到对应 JSON 对象
        </p>
      ) : null}
      <pre className="document-viewer-source" ref={sourceRef}>
        <code>
          {selectedRange ? parsedJson.formatted.slice(0, selectedRange.start) : parsedJson.formatted}
          {selectedRange ? (
            <span
              ref={selectedBlockRef}
              className="document-viewer-json-block"
              data-selected="true"
              data-json-block-key={selectedJsonBlockKey ?? undefined}
            >
              {parsedJson.formatted.slice(selectedRange.start, selectedRange.end)}
            </span>
          ) : null}
          {selectedRange ? parsedJson.formatted.slice(selectedRange.end) : null}
        </code>
      </pre>
    </div>
  );
}

export function parseOdlJson(content: string | null | undefined): ParsedOdlJson | null {
  if (!content?.trim()) return null;
  try {
    const value: unknown = JSON.parse(content);
    const blocks: OdlJsonBlock[] = [];
    collectOdlBlocks(value, "$", blocks);
    const { formatted, ranges } = formatJsonWithRanges(value);
    return { value, blocks, formatted, ranges };
  } catch {
    return null;
  }
}

function formatJsonWithRanges(value: unknown): {
  formatted: string;
  ranges: Map<string, { start: number; end: number }>;
} {
  const state = {
    chunks: [] as string[],
    length: 0,
    ranges: new Map<string, { start: number; end: number }>(),
  };
  appendFormattedJson(value, 0, "$", state);
  return { formatted: state.chunks.join(""), ranges: state.ranges };
}

function appendFormattedJson(
  value: unknown,
  depth: number,
  path: string,
  state: {
    chunks: string[];
    length: number;
    ranges: Map<string, { start: number; end: number }>;
  },
) {
  if (Array.isArray(value)) {
    appendFormattedText(state, "[");
    value.forEach((item, index) => {
      appendFormattedText(state, `\n${jsonIndent(depth + 1)}`);
      appendFormattedJson(item, depth + 1, `${path}[${index}]`, state);
      if (index < value.length - 1) appendFormattedText(state, ",");
    });
    if (value.length > 0) appendFormattedText(state, `\n${jsonIndent(depth)}`);
    appendFormattedText(state, "]");
    return;
  }

  if (isJsonObject(value)) {
    const rangeStart = state.length;
    const entries = Object.entries(value);
    appendFormattedText(state, "{");
    entries.forEach(([key, child], index) => {
      appendFormattedText(state, `\n${jsonIndent(depth + 1)}${JSON.stringify(key)}: `);
      appendFormattedJson(child, depth + 1, jsonPropertyPath(path, key), state);
      if (index < entries.length - 1) appendFormattedText(state, ",");
    });
    if (entries.length > 0) appendFormattedText(state, `\n${jsonIndent(depth)}`);
    appendFormattedText(state, "}");
    const block = odlBlockFromObject(value, path);
    if (block) state.ranges.set(block.key, { start: rangeStart, end: state.length });
    return;
  }

  appendFormattedText(state, JSON.stringify(value) ?? "null");
}

function appendFormattedText(
  state: { chunks: string[]; length: number },
  text: string,
) {
  state.chunks.push(text);
  state.length += text.length;
}

function collectOdlBlocks(value: unknown, path: string, blocks: OdlJsonBlock[]) {
  if (Array.isArray(value)) {
    value.forEach((item, index) => collectOdlBlocks(item, `${path}[${index}]`, blocks));
    return;
  }
  if (!isJsonObject(value)) return;

  const block = odlBlockFromObject(value, path);
  if (block) blocks.push(block);
  Object.entries(value).forEach(([key, child]) => {
    collectOdlBlocks(child, jsonPropertyPath(path, key), blocks);
  });
}

function odlBlockFromObject(value: Record<string, unknown>, path: string): OdlJsonBlock | null {
  const pageNumber = value["page number"];
  const boundingBox = value["bounding box"];
  if (
    typeof pageNumber !== "number" ||
    !Number.isFinite(pageNumber) ||
    pageNumber < 1 ||
    !Array.isArray(boundingBox) ||
    boundingBox.length !== 4 ||
    !boundingBox.every((coordinate) => typeof coordinate === "number" && Number.isFinite(coordinate))
  ) {
    return null;
  }
  return {
    key: path,
    pageNumber: Math.floor(pageNumber),
    value,
    rawBoundingBox: boundingBox as [number, number, number, number],
  };
}

export function normalizePdfBoundingBox(
  raw: [number, number, number, number],
  geometry: PdfPageGeometryDto,
): NormalizedBoundingBoxDto | null {
  const geometryValues = [
    geometry.width_points,
    geometry.height_points,
    geometry.crop_left_points,
    geometry.crop_bottom_points,
    geometry.crop_right_points,
    geometry.crop_top_points,
  ];
  const pageWidth = geometry.crop_right_points - geometry.crop_left_points;
  const pageHeight = geometry.crop_top_points - geometry.crop_bottom_points;
  if (
    !geometryValues.every(Number.isFinite) ||
    !raw.every(Number.isFinite) ||
    pageWidth <= 0 ||
    pageHeight <= 0 ||
    ![0, 90, 180, 270].includes(geometry.rotation_degrees)
  ) {
    return null;
  }

  const left =
    clamp(Math.min(raw[0], raw[2]), geometry.crop_left_points, geometry.crop_right_points) -
    geometry.crop_left_points;
  const right =
    clamp(Math.max(raw[0], raw[2]), geometry.crop_left_points, geometry.crop_right_points) -
    geometry.crop_left_points;
  const bottom =
    clamp(Math.min(raw[1], raw[3]), geometry.crop_bottom_points, geometry.crop_top_points) -
    geometry.crop_bottom_points;
  const top =
    clamp(Math.max(raw[1], raw[3]), geometry.crop_bottom_points, geometry.crop_top_points) -
    geometry.crop_bottom_points;
  if (right <= left || top <= bottom) return null;

  const displayWidth = geometry.rotation_degrees === 90 || geometry.rotation_degrees === 270
    ? pageHeight
    : pageWidth;
  const displayHeight = geometry.rotation_degrees === 90 || geometry.rotation_degrees === 270
    ? pageWidth
    : pageHeight;
  const displayPoints = [
    pdfPointToDisplay(left, bottom, pageWidth, pageHeight, geometry.rotation_degrees),
    pdfPointToDisplay(left, top, pageWidth, pageHeight, geometry.rotation_degrees),
    pdfPointToDisplay(right, bottom, pageWidth, pageHeight, geometry.rotation_degrees),
    pdfPointToDisplay(right, top, pageWidth, pageHeight, geometry.rotation_degrees),
  ];
  const xs = displayPoints.map(([x]) => x);
  const ys = displayPoints.map(([, y]) => y);
  const minX = Math.min(...xs);
  const maxX = Math.max(...xs);
  const minY = Math.min(...ys);
  const maxY = Math.max(...ys);
  const bbox = {
    x: clamp(minX / displayWidth, 0, 1),
    y: clamp(minY / displayHeight, 0, 1),
    width: clamp((maxX - minX) / displayWidth, 0, 1),
    height: clamp((maxY - minY) / displayHeight, 0, 1),
  };
  return bbox.width > 0 && bbox.height > 0 ? bbox : null;
}

function pdfPointToDisplay(
  x: number,
  y: number,
  pageWidth: number,
  pageHeight: number,
  rotation: PdfPageGeometryDto["rotation_degrees"],
): [number, number] {
  switch (rotation) {
    case 90:
      return [y, x];
    case 180:
      return [pageWidth - x, y];
    case 270:
      return [pageHeight - y, pageWidth - x];
    default:
      return [x, pageHeight - y];
  }
}

export function cacheAnnotPreview(
  cache: Map<string, DocumentViewerPagePreviewDto>,
  key: string,
  preview: DocumentViewerPagePreviewDto,
  limit = ANNOT_PREVIEW_CACHE_LIMIT,
) {
  if (limit <= 0) return;
  cache.delete(key);
  cache.set(key, preview);
  while (cache.size > limit) {
    const oldestKey = cache.keys().next().value as string | undefined;
    if (oldestKey === undefined) break;
    cache.delete(oldestKey);
  }
}

function getCachedAnnotPreview(
  cache: Map<string, DocumentViewerPagePreviewDto>,
  key: string,
): DocumentViewerPagePreviewDto | null {
  const preview = cache.get(key);
  if (!preview) return null;
  cache.delete(key);
  cache.set(key, preview);
  return preview;
}

function jsonIndent(depth: number): string {
  return "  ".repeat(depth);
}

function jsonPropertyPath(path: string, key: string): string {
  return `${path}[${JSON.stringify(key)}]`;
}

function isJsonObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function normalizePageCount(value: number | null): number | null {
  return typeof value === "number" && Number.isFinite(value) && value >= 1
    ? Math.floor(value)
    : null;
}

function clampPageNumber(value: number, pageCount: number | null): number {
  const page = Math.max(1, Math.floor(value));
  return pageCount === null ? page : Math.min(page, pageCount);
}

function clampZoom(value: number): number {
  return clamp(Number(value.toFixed(2)), ANNOT_ZOOM_MIN, ANNOT_ZOOM_MAX);
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(maximum, value));
}

function tooltipPosition(clientX: number, clientY: number): { left: number; top: number } {
  const viewportWidth = typeof window === "undefined" ? 1200 : window.innerWidth;
  const viewportHeight = typeof window === "undefined" ? 800 : window.innerHeight;
  const tooltipWidth = Math.min(380, Math.max(240, viewportWidth - 24));
  const tooltipHeight = Math.min(280, Math.max(120, viewportHeight - 24));
  const left = clamp(clientX + 12, 12, Math.max(12, viewportWidth - tooltipWidth - 12));
  const top = clientY + 12 + tooltipHeight <= viewportHeight - 12
    ? clientY + 12
    : Math.max(12, clientY - tooltipHeight - 12);
  return { left, top };
}

function handleTooltipScrollKey(
  event: KeyboardEvent<HTMLButtonElement>,
  tooltip: HTMLPreElement | null,
) {
  if (!tooltip || !["PageDown", "PageUp", "Home", "End"].includes(event.key)) return;
  event.preventDefault();
  if (event.key === "Home") tooltip.scrollTo({ top: 0 });
  else if (event.key === "End") tooltip.scrollTo({ top: tooltip.scrollHeight });
  else {
    tooltip.scrollBy({
      top: (event.key === "PageDown" ? 1 : -1) * Math.max(80, tooltip.clientHeight * 0.8),
    });
  }
}

function blockTypeLabel(block: OdlJsonBlock): string {
  const type = block.value.type;
  return typeof type === "string" && type.trim() ? type : "block";
}

function blockAriaLabel(block: OdlJsonBlock): string {
  const id = block.value.id;
  const idLabel = typeof id === "string" || typeof id === "number" ? ` ${id}` : "";
  return `${blockTypeLabel(block)}${idLabel}，第 ${block.pageNumber} 页，定位 JSON`;
}

function ViewerNotice({ children }: { children: ReactNode }) {
  return <p className="document-viewer-notice">{children}</p>;
}

function ViewerError({ message, onRetry }: { message: string; onRetry?: () => void }) {
  return (
    <div className="document-viewer-error" role="alert">
      <p>格式加载失败</p>
      <span>{message}</span>
      {onRetry ? <Button onClick={onRetry}>重试</Button> : null}
    </div>
  );
}

export function createViewerRequestGeneration(): ViewerRequestGeneration {
  let generation = 0;
  return {
    current: () => generation,
    next: () => {
      generation += 1;
      return generation;
    },
    isCurrent: (token) => token === generation,
  };
}

export function createSandboxedPreviewHtml(
  html: string,
  assets: DocumentViewerAssetDto[],
): string {
  const parser = new DOMParser();
  const document = parser.parseFromString(html, "text/html");
  const assetMap = new Map(
    assets.map((asset) => [normalizePreviewSource(asset.source), asset.data_url]),
  );

  document
    .querySelectorAll("script, iframe, object, embed, link, base, meta[http-equiv]")
    .forEach((node) => node.remove());
  document.querySelectorAll("a, area").forEach((link) => {
    link.removeAttribute("href");
    link.removeAttribute("xlink:href");
  });
  document.querySelectorAll("img").forEach((image) => {
    const source = normalizePreviewSource(image.getAttribute("src") ?? "");
    const dataUrl = assetMap.get(source);
    image.removeAttribute("srcset");
    if (dataUrl) {
      image.setAttribute("src", dataUrl);
    } else {
      image.removeAttribute("src");
    }
  });
  const csp = document.createElement("meta");
  csp.setAttribute("http-equiv", "Content-Security-Policy");
  csp.setAttribute(
    "content",
    "default-src 'none'; img-src data:; style-src 'unsafe-inline'; font-src data:; form-action 'none'; base-uri 'none';",
  );
  document.head.prepend(csp);
  return `<!doctype html>\n${document.documentElement.outerHTML}`;
}

function handleTabKeyDown(
  event: KeyboardEvent<HTMLButtonElement>,
  currentFormat: DocumentViewerFormat,
  onFormatChange: (format: DocumentViewerFormat) => void,
) {
  const currentIndex = VIEWER_FORMATS.indexOf(currentFormat);
  let nextIndex: number | null = null;
  if (event.key === "ArrowRight") nextIndex = (currentIndex + 1) % VIEWER_FORMATS.length;
  if (event.key === "ArrowLeft") {
    nextIndex = (currentIndex - 1 + VIEWER_FORMATS.length) % VIEWER_FORMATS.length;
  }
  if (event.key === "Home") nextIndex = 0;
  if (event.key === "End") nextIndex = VIEWER_FORMATS.length - 1;
  if (nextIndex === null) return;

  event.preventDefault();
  const nextFormat = VIEWER_FORMATS[nextIndex];
  const tabList = event.currentTarget.parentElement;
  onFormatChange(nextFormat);
  window.requestAnimationFrame(() => {
    tabList?.querySelector<HTMLButtonElement>(`[data-format="${nextFormat}"]`)?.focus();
  });
}

function normalizePreviewSource(source: string): string {
  const withoutFragment = source.split(/[?#]/, 1)[0].replace(/\\/g, "/");
  try {
    return decodeURIComponent(withoutFragment).replace(/^\.\//, "");
  } catch {
    return withoutFragment.replace(/^\.\//, "");
  }
}

function createPdfObjectUrl(content: DocumentViewerContentDto): string {
  if (typeof URL.createObjectURL !== "function") {
    return `data:${content.mime_type};base64,${content.content}`;
  }
  const binary = atob(content.content);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return URL.createObjectURL(new Blob([bytes], { type: content.mime_type }));
}

function formatSourceContent(content: DocumentViewerContentDto): string {
  if (content.format !== "json") {
    return content.content;
  }
  try {
    return JSON.stringify(JSON.parse(content.content) as unknown, null, 2);
  } catch {
    return content.content;
  }
}

function isFormatAvailable(
  manifest: DocumentViewerManifestDto,
  format: DocumentViewerFormat,
): boolean {
  return manifest.formats.some((item) => item.format === format && item.available);
}

function withSetValue<T>(current: Set<T>, value: T, present: boolean): Set<T> {
  const next = new Set(current);
  if (present) {
    next.add(value);
  } else {
    next.delete(value);
  }
  return next;
}

function normalizePageNumber(value: number | null | undefined): number | null {
  return typeof value === "number" && Number.isFinite(value) && value >= 1
    ? Math.floor(value)
    : null;
}

function extractErrorMessage(error: unknown): string {
  if (error && typeof error === "object" && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return typeof error === "string" ? error : "无法读取文档格式。";
}

function extractViewerError(error: unknown): { message: string; retryable: boolean } {
  return {
    message: extractErrorMessage(error),
    retryable:
      !error ||
      typeof error !== "object" ||
      !("retryable" in error) ||
      (error as { retryable: unknown }).retryable === true,
  };
}
