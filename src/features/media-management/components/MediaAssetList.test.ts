import { describe, expect, it } from "vitest";
import type { DocumentDto, PageWorkbenchDto } from "../../../types/app";
import { countDocumentAnalysisFailures, filterDocuments } from "./MediaAssetList";

function document(): DocumentDto {
  return {
    document_id: "structured-document",
    original_filename: "structured.pdf",
    file_type: "pdf",
    file_hash: "document-hash",
    original_path: "structured.pdf",
    page_count: 1,
    status: "ready",
    error_summary: null,
    job_id: null,
    analysis_succeeded_pages: 0,
    analysis_failed_pages: 0,
    last_analyzed_at: null,
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
  };
}

function structuredPage(overrides: Partial<PageWorkbenchDto> = {}): PageWorkbenchDto {
  return {
    page_id: "structured-page",
    document_id: "structured-document",
    page_number: 1,
    image_hash: null,
    image_path: null,
    status: "structured",
    error_summary: null,
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
    analysis_summary: null,
    visual_module_count: 2,
    pending_visual_module_count: 1,
    succeeded_visual_module_count: 0,
    failed_visual_module_count: 1,
    ...overrides,
  };
}

describe("media management filters", () => {
  it("includes visual modules in pending and failed filters", () => {
    const item = document();
    const pages = { [item.document_id]: [structuredPage()] };
    expect(filterDocuments([item], pages, "", "needs_analysis")).toEqual([item]);
    expect(filterDocuments([item], pages, "", "has_failed_pages")).toEqual([item]);
  });

  it("does not double-count failed pages reported by both document and page rows", () => {
    const item = { ...document(), analysis_failed_pages: 1 };
    const pages = [structuredPage({ status: "failed", failed_visual_module_count: 2 })];
    expect(countDocumentAnalysisFailures(item, pages)).toBe(3);
  });
});
