import { describe, expect, it } from "vitest";
import { PREVIEW_RATIO, clampPreviewRatio, previewLayout, previewTracks } from "./previewLayout";

describe("previewLayout", () => {
  it("is side by side from 720 px of main width", () => {
    expect(previewLayout(720)).toBe("side-by-side");
    expect(previewLayout(1400)).toBe("side-by-side");
  });
  it("takes the whole main area below 720 px", () => {
    expect(previewLayout(719)).toBe("full");
    expect(previewLayout(300)).toBe("full");
  });
});

describe("the divider", () => {
  it("starts at 50/50 and is held between 30% and 70%", () => {
    expect(PREVIEW_RATIO.default).toBe(0.5);
    expect(clampPreviewRatio(0.1)).toBe(0.3);
    expect(clampPreviewRatio(0.9)).toBe(0.7);
    expect(clampPreviewRatio(0.42)).toBe(0.42);
  });
  it("gives the session its share, a hairline and the preview the rest", () => {
    expect(previewTracks(0.5)).toBe("minmax(0, 0.5fr) 1px minmax(0, 0.5fr)");
    expect(previewTracks(0.3)).toBe("minmax(0, 0.3fr) 1px minmax(0, 0.7fr)");
  });
});
