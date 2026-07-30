import { describe, expect, it } from "vitest";
import type {
  AnalysisBatchResultDto,
  DocumentDto,
  PageWorkbenchDto,
} from "../../types/app";
import {
  computeAnalysisStats,
  formatAnalysisOverview,
  formatBatchMessage,
  formatCombinedBatchMessage,
  getBatchResultError,
} from "./AnalysisPage";

function document(documentId: string): DocumentDto {
  return {
    document_id: documentId,
    original_filename: `${documentId}.pdf`,
    file_type: "pdf",
    file_hash: `hash-${documentId}`,
    original_path: `${documentId}.pdf`,
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

function page(
  pageId: string,
  documentId: string,
  overrides: Partial<PageWorkbenchDto> = {},
): PageWorkbenchDto {
  return {
    page_id: pageId,
    document_id: documentId,
    page_number: 1,
    image_hash: `hash-${pageId}`,
    image_path: `pages/${pageId}.png`,
    status: "rendered",
    error_summary: null,
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
    analysis_summary: null,
    ...overrides,
  };
}

describe("analysis overview", () => {
  it("shows legacy page and structured visual-module statistics together", () => {
    const documents = [document("legacy"), document("structured")];
    const pagesByDocument = {
      legacy: [page("legacy-page", "legacy")],
      structured: [
        page("structured-page", "structured", {
          visual_module_count: 4,
          pending_visual_module_count: 2,
          succeeded_visual_module_count: 1,
          failed_visual_module_count: 1,
        }),
      ],
    };

    const stats = computeAnalysisStats(documents, pagesByDocument);
    const overview = formatAnalysisOverview(stats);

    expect(stats.hasLegacyPageStats).toBe(true);
    expect(stats.hasVisualModuleStats).toBe(true);
    expect(stats.pendingPages).toBe(1);
    expect(stats.pendingVisualModules).toBe(2);
    expect(overview).toContain("待分析 1 页");
    expect(overview).toContain("视觉模块共 4 个");
    expect(overview).toContain("待分析 2 个");
  });

  it("keeps page and visual-module batch counts semantically separate", () => {
    const result: AnalysisBatchResultDto = {
      job_id: "job-1",
      total_pages: 2,
      succeeded_pages: 1,
      failed_pages: 1,
      skipped_pages: 0,
      total_visual_modules: 3,
      succeeded_visual_modules: 2,
      failed_visual_modules: 0,
      skipped_visual_modules: 1,
      status: "succeeded_with_failures",
      error: null,
      updated_at: "2026-01-01T00:00:00Z",
    };

    const message = formatBatchMessage("完成", result);

    expect(message).toContain("页面共 2 页");
    expect(message).toContain("失败 1 页");
    expect(message).toContain("视觉模块共 3 个");
    expect(message).toContain("跳过 1 个");
  });

  it("combines visual-module counts across document reanalysis results", () => {
    const first: AnalysisBatchResultDto = {
      job_id: "job-1",
      total_pages: 0,
      succeeded_pages: 0,
      failed_pages: 0,
      skipped_pages: 0,
      total_visual_modules: 2,
      succeeded_visual_modules: 1,
      failed_visual_modules: 1,
      skipped_visual_modules: 0,
      status: "succeeded_with_failures",
      error: null,
      updated_at: "2026-01-01T00:00:00Z",
    };
    const second = {
      ...first,
      job_id: "job-2",
      total_visual_modules: 3,
      succeeded_visual_modules: 3,
      failed_visual_modules: 0,
    };

    const message = formatCombinedBatchMessage("完成", [first, second]);

    expect(message).toContain("视觉模块共 5 个");
    expect(message).toContain("成功 4 个");
    expect(message).toContain("失败 1 个");
  });

  it("maps the representative batch error to visible diagnostics", () => {
    const result: AnalysisBatchResultDto = {
      job_id: "job-failed",
      total_pages: 0,
      succeeded_pages: 0,
      failed_pages: 0,
      skipped_pages: 0,
      total_visual_modules: 2,
      succeeded_visual_modules: 0,
      failed_visual_modules: 2,
      skipped_visual_modules: 0,
      status: "failed",
      error: {
        code: "model_http_status_failed",
        message: "模型分析失败：当前 API Key 所属账号或分组没有可用订阅。",
        stage: "analysis_provider",
        retryable: true,
        details: "status=403; endpoint_kind=openai",
        correlation_id: "diagnostic-403",
      },
      updated_at: "2026-01-01T00:00:00Z",
    };

    expect(getBatchResultError(result)).toEqual({
      title: "分析失败",
      message: "模型分析失败：当前 API Key 所属账号或分组没有可用订阅。",
      details: "status=403; endpoint_kind=openai",
      correlationId: "diagnostic-403",
    });

    expect(
      getBatchResultError({ ...result, status: "succeeded_with_failures" })?.title,
    ).toBe("分析部分完成");
  });
});
