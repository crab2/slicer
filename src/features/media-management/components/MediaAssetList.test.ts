import { describe, expect, it } from "vitest";
import type { DocumentDto, PageWorkbenchDto } from "../../../types/app";
import {
  filterDocuments,
  getDocumentReanalysisValidation,
} from "./MediaAssetList";

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

function structuredPage(
  overrides: Partial<PageWorkbenchDto> = {},
): PageWorkbenchDto {
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

describe("structured media management", () => {
  it("allows document reanalysis without a whole-page image", () => {
    expect(
      getDocumentReanalysisValidation(document(), [structuredPage()]),
    ).toEqual({ disabledReason: null });
  });

  it("includes visual modules in pending and failed filters", () => {
    const doc = document();
    const pages = { [doc.document_id]: [structuredPage()] };

    expect(filterDocuments([doc], pages, "", "needs_analysis")).toEqual([doc]);
    expect(filterDocuments([doc], pages, "", "has_failed_pages")).toEqual([doc]);
  });
});
