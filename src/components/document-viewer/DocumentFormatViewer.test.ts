import { describe, expect, it } from "vitest";
import type { DocumentViewerPagePreviewDto, PdfPageGeometryDto } from "../../types/app";
import {
  cacheAnnotPreview,
  createViewerRequestGeneration,
  normalizePdfBoundingBox,
  parseOdlJson,
} from "./DocumentFormatViewer";

describe("document viewer request generation", () => {
  it("rejects responses from a previously selected document", () => {
    const generation = createViewerRequestGeneration();
    const firstDocument = generation.next();
    expect(generation.isCurrent(firstDocument)).toBe(true);

    const secondDocument = generation.next();
    expect(generation.isCurrent(firstDocument)).toBe(false);
    expect(generation.isCurrent(secondDocument)).toBe(true);
  });
});

describe("Annot and JSON block linking", () => {
  it("uses stable JSON paths for nested ODL blocks", () => {
    const parsed = parseOdlJson(JSON.stringify({
      "number of pages": 2,
      kids: [
        {
          type: "paragraph",
          id: 7,
          "page number": 1,
          "bounding box": [10, 150, 90, 180],
          content: "first",
          kids: [
            {
              type: "caption",
              id: 7,
              "page number": 1,
              "bounding box": [10, 130, 90, 145],
              content: "nested",
              slicer_visual_analysis: {
                schema_version: "visual_module_analysis_v1",
                block_id: "pdfmod-nested",
                description: "Model description",
                visible_text: "Visible model text",
                keywords: ["model", "image"],
                model: { provider: "local_mock", model_name: "mock-model" },
              },
            },
          ],
        },
        {
          type: "paragraph",
          id: 7,
          "page number": 2,
          "bounding box": [10, 20, 90, 40],
          content: "second page",
        },
      ],
    }));

    expect(parsed?.blocks.map((block) => block.key)).toEqual([
      '$["kids"][0]',
      '$["kids"][0]["kids"][0]',
      '$["kids"][1]',
    ]);
    expect(parsed?.blocks.map((block) => block.pageNumber)).toEqual([1, 1, 2]);
    const nestedKey = '$["kids"][0]["kids"][0]';
    const nestedRange = parsed?.ranges.get(nestedKey);
    expect(nestedRange).toBeDefined();
    expect(
      JSON.parse(parsed!.formatted.slice(nestedRange!.start, nestedRange!.end)),
    ).toMatchObject({
      type: "caption",
      content: "nested",
      slicer_visual_analysis: {
        description: "Model description",
        visible_text: "Visible model text",
        keywords: ["model", "image"],
      },
    });
  });

  it("keeps valid blocks when siblings have invalid bounding boxes", () => {
    const parsed = parseOdlJson(JSON.stringify({
      kids: [
        { "page number": 1, "bounding box": [0, 0, 10, 10] },
        { "page number": 1, "bounding box": [0, "bad", 10, 10] },
        { "page number": 1, content: "missing bbox" },
      ],
    }));

    expect(parsed?.blocks).toHaveLength(1);
    expect(parseOdlJson("{broken")).toBeNull();
  });

  it("normalizes PDF coordinates for unrotated and rotated pages", () => {
    const geometry: PdfPageGeometryDto = {
      width_points: 100,
      height_points: 200,
      crop_left_points: 0,
      crop_bottom_points: 0,
      crop_right_points: 100,
      crop_top_points: 200,
      rotation_degrees: 0,
    };

    expect(normalizePdfBoundingBox([10, 150, 90, 180], geometry)).toEqual({
      x: 0.1,
      y: 0.1,
      width: 0.8,
      height: 0.15,
    });
    expect(
      normalizePdfBoundingBox([10, 150, 90, 180], {
        ...geometry,
        rotation_degrees: 90,
      }),
    ).toEqual({
      x: 0.75,
      y: 0.1,
      width: 0.15,
      height: 0.8,
    });
    expect(normalizePdfBoundingBox([10, 10, 10, 10], geometry)).toBeNull();
  });

  it("evicts the oldest preview while refreshing replaced entries", () => {
    const cache = new Map<string, DocumentViewerPagePreviewDto>();
    cacheAnnotPreview(cache, "page-1", preview(1), 2);
    cacheAnnotPreview(cache, "page-2", preview(2), 2);
    cacheAnnotPreview(cache, "page-1", preview(1), 2);
    cacheAnnotPreview(cache, "page-3", preview(3), 2);

    expect([...cache.keys()]).toEqual(["page-1", "page-3"]);
  });
});

function preview(pageNumber: number): DocumentViewerPagePreviewDto {
  return {
    format: "annot",
    page_number: pageNumber,
    mime_type: "image/png",
    data_url: `data:image/png;base64,page-${pageNumber}`,
    geometry: {
      width_points: 100,
      height_points: 200,
      crop_left_points: 0,
      crop_bottom_points: 0,
      crop_right_points: 100,
      crop_top_points: 200,
      rotation_degrees: 0,
    },
  };
}
