import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { exportFileName } from "../../lib/exportName";
import { ExportDialog, copyExport, saveExport } from "./ExportDialog";

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
