import { describe, expect, it } from "vitest";
import type { SearchResultItemDto } from "../../types/app";
import {
  buildSearchResultEntries,
  findSelectedSearchResult,
} from "./SearchPage";

function searchResult(overrides: Partial<SearchResultItemDto> = {}): SearchResultItemDto {
  return {
    hit_id: "hit-1",
    module_id: "module-1",
    type: "paragraph",
    snippet: "matched text",
    page: { page_id: "page-1", document_id: "document-1", page_number: 1 },
    bbox: null,
    module_json: null,
    page_id: "page-1",
    document_id: "document-1",
    page_number: 1,
    original_filename: "example.pdf",
    score: 1,
    title: null,
    summary: null,
    image_path: null,
    image_available: false,
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
