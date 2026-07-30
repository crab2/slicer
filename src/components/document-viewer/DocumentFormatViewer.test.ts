import { describe, expect, it } from "vitest";
import { createViewerRequestGeneration } from "./DocumentFormatViewer";

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
