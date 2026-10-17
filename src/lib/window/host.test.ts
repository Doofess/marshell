import { describe, expect, it } from "vitest";
import { markDesktopHost } from "./appWindow";

describe("markDesktopHost", () => {
  it("does nothing outside Tauri", () => {
    const doc = { documentElement: { dataset: {} as Record<string, string> } };
    markDesktopHost(doc as unknown as Document);
    expect(doc.documentElement.dataset.host).toBeUndefined();
  });
});
