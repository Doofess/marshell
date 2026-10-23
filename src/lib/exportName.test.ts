import { describe, expect, it } from "vitest";
import { exportFileName } from "./exportName";

const day = new Date(2026, 9, 23, 23, 59);

describe("exportFileName", () => {
  it("turns a title into lower-case words and a local date", () => {
    expect(exportFileName("Fix: the API!", "markdown", day)).toBe("fix-the-api-2026-10-23.md");
    expect(exportFileName("Retry limit work", "json", day)).toBe("retry-limit-work-2026-10-23.json");
  });
  it("falls back to 'conversation' when nothing ASCII is left", () => {
    expect(exportFileName("שלום עולם", "markdown", day)).toBe("conversation-2026-10-23.md");
    expect(exportFileName("  !!! ", "json", day)).toBe("conversation-2026-10-23.json");
    expect(exportFileName("", "markdown", day)).toBe("conversation-2026-10-23.md");
  });
  it("cuts the words to 40 characters without a trailing hyphen", () => {
    const name = exportFileName("a".repeat(39) + " b" + "c".repeat(30), "markdown", day);
    expect(name).toBe(`${"a".repeat(39)}-2026-10-23.md`);
    expect(exportFileName("x".repeat(100), "json", day)).toBe(`${"x".repeat(40)}-2026-10-23.json`);
  });
  it("never lets a path or an extension through", () => {
    expect(exportFileName("../../evil.exe", "markdown", day)).toBe("evil-exe-2026-10-23.md");
    expect(exportFileName("C:\\Users\\me\\a.md", "json", day)).toBe("c-users-me-a-md-2026-10-23.json");
  });
  it("pads the date and uses local time", () => {
    expect(exportFileName("Notes", "markdown", new Date(2026, 0, 5, 0, 1))).toBe("notes-2026-01-05.md");
  });
  it("always fits the shell's name check", () => {
    const shape = /^[a-z0-9]+(?:-[a-z0-9]+)*-\d{4}-\d{2}-\d{2}\.(md|json)$/;
    for (const title of ["", "Ünïcode ✓ title", "a-b--c", "---", "9".repeat(80), "x y\nz\t!"]) {
      for (const format of ["markdown", "json"] as const) {
        const name = exportFileName(title, format, day);
        expect(name).toMatch(shape);
        expect(name.length).toBeLessThanOrEqual(60);
      }
    }
  });
});
