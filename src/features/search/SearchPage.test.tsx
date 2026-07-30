import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { SearchResultItemDto } from "../../types/app";
import {
  AnnotatedPageImage,
  buildSearchResultEntries,
  cachePagePreview,
  createPreviewCacheState,
  findSelectedSearchResult,
  loadPagePreviewOnce,
  validNormalizedBbox,
} from "./SearchPage";

function searchResult(
  overrides: Partial<SearchResultItemDto> = {},
): SearchResultItemDto {
  return {
    hit_id: "hit-1",
    module_id: "module-1",
    type: "paragraph",
    snippet: "matched text",
    page: {
      page_id: "page-1",
      document_id: "document-1",
      page_number: 1,
    },
    bbox: null,
    module_json: null,
    page_id: "page-1",
    document_id: "document-1",
    page_number: 1,
    original_filename: "example.pdf",
    score: 1,
    title: null,
    summary: null,
    image_path: "pages/page-1.png",
    image_available: true,
    page_json: "{}",
    ...overrides,
  };
}

describe("search results", () => {
  it("keeps multiple hits from one page and selects by unique hit_id", () => {
    const first = searchResult({ hit_id: "hit-1", module_id: "module-1" });
    const second = searchResult({ hit_id: "hit-2", module_id: "module-2" });

    const entries = buildSearchResultEntries([first, second]);

    expect(entries.map(({ hitId }) => hitId)).toEqual(["hit-1", "hit-2"]);
    expect(findSelectedSearchResult(entries, "hit-2")).toBe(second);
  });
});

describe("page preview cache", () => {
  it("loads one preview for concurrent and later hits on the same page", async () => {
    const cache = createPreviewCacheState();
    const requests = new Map<string, Promise<string | null>>();
    let resolvePreview: (value: string) => void = () => undefined;
    const preview = new Promise<string>((resolve) => {
      resolvePreview = resolve;
    });
    const loader = vi.fn(() => preview);

    const first = loadPagePreviewOnce(cache, requests, "page-1", loader);
    const second = loadPagePreviewOnce(cache, requests, "page-1", loader);
    expect(first).toBe(second);

    resolvePreview("data:image/png;base64,cHJldmlldw==");
    await expect(first).resolves.toContain("data:image/png");
    await expect(second).resolves.toContain("data:image/png");
    await expect(
      loadPagePreviewOnce(cache, requests, "page-1", loader),
    ).resolves.toContain("data:image/png");
    expect(loader).toHaveBeenCalledTimes(1);
  });

  it("evicts least-recent entries to satisfy entry and byte caps", () => {
    const cache = createPreviewCacheState();
    const limits = { maxEntries: 2, maxBytes: 6 };

    cachePagePreview(cache, "page-1", "111", limits);
    cachePagePreview(cache, "page-2", "222", limits);
    cachePagePreview(cache, "page-3", "333", limits);

    expect([...cache.entries.keys()]).toEqual(["page-2", "page-3"]);
    expect(cache.entries.size).toBeLessThanOrEqual(limits.maxEntries);
    expect(cache.totalBytes).toBeLessThanOrEqual(limits.maxBytes);
  });
});

describe("preview bounding boxes", () => {
  it("clamps tolerated floating-point overflow to normalized bounds", () => {
    const bbox = validNormalizedBbox({
      x: -0.0000005,
      y: 0.8,
      width: 0.4,
      height: 0.2000005,
    });

    expect(bbox).not.toBeNull();
    expect(bbox?.x).toBe(0);
    expect(bbox?.y).toBeGreaterThanOrEqual(0);
    expect((bbox?.x ?? 0) + (bbox?.width ?? 0)).toBeLessThanOrEqual(1);
    expect((bbox?.y ?? 0) + (bbox?.height ?? 0)).toBeLessThanOrEqual(1);
  });

  it("renders no overlay for bbox null", () => {
    const markup = renderToStaticMarkup(
      <AnnotatedPageImage
        src="data:image/png;base64,cHJldmlldw=="
        alt="preview"
        bbox={null}
        imageClassName="preview-image"
      />,
    );

    expect(markup).not.toContain("search-bbox-overlay");
  });
});
