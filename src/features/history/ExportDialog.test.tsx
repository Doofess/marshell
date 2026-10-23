import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { exportFileName } from "../../lib/exportName";
import { ExportDialog, NO_RECORDING_COPY, copyExport, firstLines, previewFailure, saveExport } from "./ExportDialog";

const noop = () => {};
const dialog = () =>
  renderToStaticMarkup(
    <ExportDialog title="Fix the API" load={() => Promise.resolve("")} onSave={() => Promise.resolve(null)} onCopy={() => Promise.resolve()} onClose={noop} announce={noop} />,
  );

describe("ExportDialog", () => {
  it("starts on Markdown with secrets hidden", () => {
    const out = dialog();
    expect(out).toContain('role="dialog"');
    expect(out).toContain('aria-modal="true"');
    expect(out).toContain("Export conversation");
    expect(out).toMatch(/<input[^>]*type="radio"[^>]*checked=""[^>]*\/>\s*<span>Markdown<\/span>/);
    expect(out).toMatch(/<input[^>]*type="radio"(?![^>]*checked)[^>]*\/>\s*<span>JSON<\/span>/);
    expect(out).toMatch(/<input[^>]*type="checkbox"[^>]*checked=""[^>]*\/>\s*<span>Hide secrets and personal paths<\/span>/);
    expect(out).toMatch(/<pre[^>]*tabindex="0"/);
    expect(out).toContain("Preview");
    for (const b of ["Save…", "Copy", "Close"]) expect(out).toContain(`>${b}<`);
  });

  it("offers Asciicast (.cast) as a third format", () => {
    expect(dialog()).toMatch(/<input[^>]*type="radio"(?![^>]*checked)[^>]*\/>\s*<span>Asciicast \(\.cast\)<\/span>/);
  });

  it("with asciicast selected, hides Copy and says the preview is cut to 200 lines", () => {
    const out = renderToStaticMarkup(
      <ExportDialog
        title="Fix the API"
        initialFormat="asciicast"
        load={() => Promise.resolve("")}
        onSave={() => Promise.resolve(null)}
        onCopy={() => Promise.resolve()}
        onClose={noop}
        announce={noop}
      />,
    );
    expect(out).toMatch(/<input[^>]*type="radio"[^>]*checked=""[^>]*\/>\s*<span>Asciicast \(\.cast\)<\/span>/);
    expect(out).not.toContain(">Copy<");
    expect(out).toContain("The preview shows the first 200 lines.");
    expect(out).toContain(">Save…<");
    expect(out).toContain(">Close<");
  });

  it("Save names the file with exportFileName, and says where it went", async () => {
    const onSave = vi.fn((_text: string, _name: string) => Promise.resolve<string | null>("C:\\out\\x.md"));
    const announce = vi.fn();
    const now = new Date(2026, 9, 23);
    await saveExport({ text: "# t", title: "Fix the API", format: "markdown", onSave, announce, now });
    expect(onSave).toHaveBeenCalledWith("# t", exportFileName("Fix the API", "markdown", now));
    expect(onSave.mock.calls[0]![1]).toBe("fix-the-api-2026-10-23.md");
    expect(announce).toHaveBeenCalledWith("Saved to C:\\out\\x.md.");
  });

  it("Save says nothing when the person cancels the Save dialog", async () => {
    const announce = vi.fn();
    await saveExport({ text: "x", title: "t", format: "json", onSave: () => Promise.resolve(null), announce, now: new Date() });
    expect(announce).not.toHaveBeenCalled();
  });

  it("Copy says which format it copied", async () => {
    const announce = vi.fn();
    const onCopy = vi.fn((_text: string) => Promise.resolve());
    await copyExport({ text: "{}", format: "json", onCopy, announce });
    expect(onCopy).toHaveBeenCalledWith("{}");
    expect(announce).toHaveBeenCalledWith("Copied as JSON.");
    await copyExport({ text: "#", format: "markdown", onCopy, announce });
    expect(announce).toHaveBeenLastCalledWith("Copied as markdown.");
  });
});

describe("asciicast preview", () => {
  it("keeps the first 200 lines and leaves a shorter text whole", () => {
    const text = Array.from({ length: 500 }, (_, i) => `line ${i}`).join("\n");
    const cut = firstLines(text);
    expect(cut.split("\n")).toHaveLength(200);
    expect(cut.endsWith("line 199")).toBe(true);
    expect(firstLines("a\nb\n")).toBe("a\nb\n");
    expect(firstLines("")).toBe("");
    expect(firstLines("a\nb\nc", 2)).toBe("a\nb");
  });

  it("reads a 404 for an asciicast as 'no recording', and any other failure as a failure", () => {
    const notFound = Object.assign(new Error("Marshell has no recording of this conversation."), { status: 404 });
    expect(previewFailure("asciicast", notFound)).toEqual({ kind: "none" });
    expect(NO_RECORDING_COPY).toBe("Marshell has no recording of this conversation.");
    // A markdown 404 is still a failure; so is a 413.
    expect(previewFailure("markdown", notFound)).toMatchObject({ kind: "failed" });
    const tooLarge = Object.assign(new Error("This recording is over 64 MiB, too large to export."), { status: 413 });
    expect(previewFailure("asciicast", tooLarge)).toEqual({ kind: "failed", message: "This recording is over 64 MiB, too large to export." });
  });
});

describe("saveExport with hidden secrets", () => {
  it("names the file from the redacted title", async () => {
    const onSave = vi.fn((_text: string, _name: string) => Promise.resolve<string | null>(null));
    await saveExport({
      text: "# use <anthropic-key> here\n\nbody",
      title: "use sk-ant-api03-SECRET here",
      format: "markdown",
      redact: true,
      onSave,
      announce: vi.fn(),
      now: new Date(2026, 9, 23),
    });
    expect(onSave.mock.calls[0]![1]).toBe("use-anthropic-key-here-2026-10-23.md");
  });
});
