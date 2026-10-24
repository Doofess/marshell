import { describe, expect, it } from "vitest";
import { exportFileName, exportTitle } from "./exportName";

const day = new Date(2026, 9, 23, 23, 59);

describe("exportFileName", () => {
  it("turns a title into lower-case words and a local date", () => {
    expect(exportFileName("Fix: the API!", "markdown", day)).toBe("fix-the-api-2026-10-23.md");
    expect(exportFileName("Retry limit work", "json", day)).toBe("retry-limit-work-2026-10-23.json");
  });
  it("gives a recording the .cast extension", () => {
    expect(exportFileName("Fix: the API!", "asciicast", day)).toBe("fix-the-api-2026-10-23.cast");
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
    const shape = /^[a-z0-9]+(?:-[a-z0-9]+)*-\d{4}-\d{2}-\d{2}\.(md|json|cast)$/;
    for (const title of ["", "Ünïcode ✓ title", "a-b--c", "---", "9".repeat(80), "x y\nz\t!"]) {
      for (const format of ["markdown", "json", "asciicast"] as const) {
        const name = exportFileName(title, format, day);
        expect(name).toMatch(shape);
        expect(name.length).toBeLessThanOrEqual(60);
      }
    }
  });
});

describe("exportTitle", () => {
  const raw = "use sk-ant-api03-SECRET here";
  it("uses the raw title when nothing is hidden", () => {
    expect(exportTitle("# other\n\nbody", "markdown", raw, false)).toBe(raw);
  });
  it("reads the redacted title from the first line of a markdown export", () => {
    const name = exportFileName(exportTitle("# use <anthropic-key> here\n\nbody", "markdown", raw, true), "markdown", day);
    expect(name).toContain("anthropic-key");
    expect(name).not.toContain("sk-ant");
  });
  it("reads the redacted title from a JSON export", () => {
    const text = JSON.stringify({ title: "fix <home>/app", turns: [] });
    expect(exportFileName(exportTitle(text, "json", "fix C:\Users\rotem\app", true), "json", day)).toContain("home");
  });
  it("reads the redacted title from the header line of an asciicast", () => {
    const text = `${JSON.stringify({ version: 2, width: 80, height: 24, title: "use <anthropic-key> here" })}\n[0,"o","x"]\n`;
    const name = exportFileName(exportTitle(text, "asciicast", raw, true), "asciicast", day);
    expect(name).toBe("use-anthropic-key-here-2026-10-23.cast");
    expect(exportTitle(text, "asciicast", raw, false)).toBe(raw);
    expect(exportTitle("not a header\n", "asciicast", raw, true)).toBe("");
  });
  it("never falls back to the raw title when the redacted text has none", () => {
    expect(exportTitle("not json", "json", raw, true)).toBe("");
    expect(exportTitle("", "markdown", raw, true)).toBe("");
  });
});
