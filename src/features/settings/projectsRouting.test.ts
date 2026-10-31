import { describe, expect, it } from "vitest";
import { paletteSection } from "./settingsRouting";
import { SETTINGS_SECTIONS } from "./settingsSections";

describe("the Projects section", () => {
  it("is listed after History, and the palette's edit-references opens it", () => {
    const ids = SETTINGS_SECTIONS.map((s) => s.id);
    expect(ids[ids.indexOf("history") + 1]).toBe("projects");
    expect(SETTINGS_SECTIONS.find((s) => s.id === "projects")?.label).toBe("Projects");
    expect(paletteSection("edit-references")).toBe("projects");
  });
});
