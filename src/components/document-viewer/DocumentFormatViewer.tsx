import {
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
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
} from "../../types/app";

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

export function DocumentFormatViewer({
  documentId,
  documentTitle,
  pageNumber,
  refreshKey = 0,
}: DocumentFormatViewerProps) {
  const [manifest, setManifest] = useState<DocumentViewerManifestDto | null>(null);
  const [manifestLoading, setManifestLoading] = useState(false);
  const [manifestError, setManifestError] = useState<string | null>(null);
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
  const requestGeneration = useRef(createViewerRequestGeneration());
  const pendingRequests = useRef(
    new Map<DocumentViewerFormat, Promise<DocumentViewerContentDto>>(),
  );

  useEffect(() => {
    const token = requestGeneration.current.next();
    pendingRequests.current.clear();
    setManifest(null);
    setManifestError(null);
    setContentByFormat({});
    setLoadingFormats(new Set());
    setErrorsByFormat({});
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
          setManifestError(extractErrorMessage(error));
        }
      })
      .finally(() => {
        if (requestGeneration.current.isCurrent(token)) {
          setManifestLoading(false);
        }
      });
  }, [documentId, manifestReloadVersion, refreshKey]);

  useEffect(() => {
    if (!documentId || !manifest) {
      return;
    }
    const token = requestGeneration.current.current();
    for (const format of new Set([leftFormat, rightFormat])) {
      if (!isFormatAvailable(manifest, format) || contentByFormat[format]) {
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
  }, [contentByFormat, documentId, leftFormat, manifest, rightFormat]);

  const viewerTitle = manifest?.original_filename || documentTitle || "文档查看";
  const page = normalizePageNumber(pageNumber);

  return (
    <section className="document-format-viewer" aria-label={`${viewerTitle} 格式查看器`}>
      <header className="document-viewer-header">
        <div>
          <p className="eyebrow">文档查看</p>
          <h3 title={viewerTitle}>{viewerTitle}</h3>
        </div>
        <p className="document-viewer-page">
          {page ? `定位第 ${page} 页` : manifest?.page_count ? `共 ${manifest.page_count} 页` : "完整文档"}
        </p>
      </header>

      <div className="document-viewer-grid">
        <ViewerPaneView
          pane="left"
          activeFormat={leftFormat}
          onFormatChange={setLeftFormat}
          manifest={manifest}
          manifestLoading={manifestLoading}
          manifestError={manifestError}
          documentId={documentId}
          pageNumber={page}
          content={contentByFormat[leftFormat]}
          loading={loadingFormats.has(leftFormat)}
          error={errorsByFormat[leftFormat]}
          onManifestRetry={() => setManifestReloadVersion((current) => current + 1)}
        />
        <ViewerPaneView
          pane="right"
          activeFormat={rightFormat}
          onFormatChange={setRightFormat}
          manifest={manifest}
          manifestLoading={manifestLoading}
          manifestError={manifestError}
          documentId={documentId}
          pageNumber={page}
          content={contentByFormat[rightFormat]}
          loading={loadingFormats.has(rightFormat)}
          error={errorsByFormat[rightFormat]}
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
  loading,
  error,
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
  loading: boolean;
  error?: string;
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
        ) : error ? (
          <ViewerError message={error} />
        ) : loading || !content ? (
          <ViewerNotice>正在加载 {FORMAT_LABELS[activeFormat]}...</ViewerNotice>
        ) : (
          <FormatContent content={content} pageNumber={pageNumber} />
        )}
      </div>
    </div>
  );
}

function FormatContent({
  content,
  pageNumber,
}: {
  content: DocumentViewerContentDto;
  pageNumber: number | null;
}) {
  if (content.format === "pdf" || content.format === "annot") {
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
