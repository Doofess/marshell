import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { ExportPreview } from "../../generated/ExportPreview";
import type { LoggingState } from "../../generated/LoggingState";
import { exportName, formatBytes } from "../../lib/api/diagnostics";
import { DiagnosticsDialogView, diagnosticsFocusTarget, exportReadyMessage } from "./DiagnosticsDialog";

const none = () => {};
const handlers = { onClose: none, onDetailed: none, onPrepare: none, onView: none, onBack: none, onSave: none };
const PREVIEW: ExportPreview = {
  id: "x",
  files: [
    { name: "about.json", bytes: 512 },
    { name: "logs/marshell.log", bytes: 2048 },
    { name: "logs/big.log", bytes: 3_250_000 },
  ],
  total_bytes: 3_252_560,
};
const view = (logging: LoggingState | "loading" | "failed", preview: Parameters<typeof DiagnosticsDialogView>[0]["preview"] = "idle", viewing: { name: string; text: string } | null = null) =>
  renderToStaticMarkup(<DiagnosticsDialogView logging={logging} preview={preview} viewing={viewing} {...handlers} />);
const ON: LoggingState = { available: true, detailed: false };

describe("diagnosticsFocusTarget", () => {
  it("recovers focus on Prepare when an export expires or preparation fails", () => {
    expect(diagnosticsFocusTarget(PREVIEW, "expired")).toBe("diagnostics-prepare");
    expect(diagnosticsFocusTarget("preparing", "failed")).toBe("diagnostics-prepare");
  });
  it("keeps focus inside while preparing again removes the file controls", () => {
    expect(diagnosticsFocusTarget(PREVIEW, "preparing")).toBe("diagnostics-prepare");
    expect(diagnosticsFocusTarget("expired", "preparing")).toBe("diagnostics-prepare");
    expect(diagnosticsFocusTarget("failed", "preparing")).toBe("diagnostics-prepare");
  });
  it("focuses the title after preparing an export, including an empty one", () => {
    expect(diagnosticsFocusTarget("preparing", PREVIEW)).toBe("diagnostics-title");
    expect(diagnosticsFocusTarget("preparing", { ...PREVIEW, files: [] })).toBe("diagnostics-title");
  });
  it("does not refocus on unchanged preview state or ordinary viewer navigation", () => {
    for (const state of ["idle", "preparing", "expired", "failed", PREVIEW] as const) {
      expect(diagnosticsFocusTarget(state, state)).toBeNull();
    }
  });
});

describe("formatBytes and exportName", () => {
  it("formats sizes", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(2048)).toBe("2.0 KB");
    expect(formatBytes(3_250_000)).toBe("3.1 MB");
  });
  it("names the zip with the local date", () => {
    expect(exportName(new Date(2026, 9, 20))).toBe("marshell-diagnostics-2026-10-20.zip");
  });
});

describe("exportReadyMessage", () => {
  it("announces one file with the formatted total", () => {
    expect(exportReadyMessage({ ...PREVIEW, files: [PREVIEW.files[0]!], total_bytes: 512 })).toBe("Export ready: 1 file, 512 B.");
  });
  it("announces seven files with the formatted total", () => {
    expect(exportReadyMessage({ ...PREVIEW, files: Array(7).fill(PREVIEW.files[0]), total_bytes: 3_250_000 })).toBe("Export ready: 7 files, 3.1 MB.");
  });
});

describe("DiagnosticsDialogView", () => {
  it("keeps an empty polite status region mounted when idle, ready and viewing", () => {
    for (const h of [view(ON), view(ON, PREVIEW), view(ON, PREVIEW, { name: "about.json", text: "{}" })]) {
      expect(h).toMatch(/<p[^>]*role="status"[^>]*aria-live="polite"[^>]*><\/p>/);
      expect(h.match(/role="status"/g)).toHaveLength(1);
    }
  });
  it("puts preparing text into the persistent polite region", () => {
    const h = view(ON, "preparing");
    expect(h).toMatch(/<p[^>]*role="status"[^>]*aria-live="polite"[^>]*>Preparing the export…<\/p>/);
    expect(h.match(/role="status"/g)).toHaveLength(1);
  });
  it("announces expiration through just the polite region, without a second alert", () => {
    const h = view(ON, "expired");
    expect(h).toMatch(/<p[^>]*role="status"[^>]*aria-live="polite"[^>]*>This export has expired\. Prepare it again\.<\/p>/);
    expect(h.match(/This export has expired/g)).toHaveLength(1);
    expect(h).not.toContain('role="alert"');
  });
  it("shows the title, the logs switch and the export copy", () => {
    const h = view(ON);
    expect(h).toContain('aria-label="Diagnostics"');
    expect(h).toContain("Detailed logs");
    expect(h).toContain("Writes debug lines to Marshell&#x27;s log until Marshell restarts.");
    expect(h).toContain("Export diagnostics");
    expect(h).toContain("Logs and setup only: no transcripts, recordings, prompts or hook events. Keys, tokens, your home folder and user name are replaced.");
    expect(h).toContain(">Prepare export</button>");
  });
  it("disables the switch with its reason when logging is not running", () => {
    const h = view({ available: false, detailed: false });
    expect(h).toMatch(/<input[^>]*role="switch"[^>]*disabled/);
    expect(h).toContain("Logging isn&#x27;t running in this build.");
    expect(h).not.toContain("Writes debug lines");
  });
  it("lists the files with sizes, View buttons, the total and the two actions", () => {
    const h = view(ON, PREVIEW);
    expect(h).toContain("about.json");
    expect(h).toContain("512 B");
    expect(h).toContain("2.0 KB");
    expect(h).toContain("3.1 MB");
    expect((h.match(/>View</g) ?? []).length).toBe(3);
    expect(h).toContain("3 files, 3.1 MB");
    expect(h).toContain(">Save…</button>");
    expect(h).toContain(">Prepare again</button>");
    expect(h).not.toContain(">Prepare export</button>");
  });
  it("says one file in the singular", () => {
    expect(view(ON, { id: "x", files: [{ name: "a", bytes: 1 }], total_bytes: 1 })).toContain("1 file, 1 B");
  });
  it("shows the expired state with the Prepare button", () => {
    const h = view(ON, "expired");
    expect(h).toContain("This export has expired. Prepare it again.");
    expect(h).toContain(">Prepare export</button>");
  });
  it("renders the focus targets in the expired, failed, preparing and ready views", () => {
    for (const state of ["expired", "failed", "preparing"] as const) {
      expect(view(ON, state)).toMatch(/<button[^>]*id="diagnostics-prepare"[^>]*>Prepare export<\/button>/);
    }
    expect(view(ON, PREVIEW)).toMatch(/<h2[^>]*id="diagnostics-title"[^>]*tabindex="-1"/);
  });
  it("shows the text of the viewed file in a labelled read-only region", () => {
    const h = view(ON, PREVIEW, { name: "logs/marshell.log", text: "line one\nline two" });
    expect(h).toMatch(/<pre[^>]*tabindex="0"/);
    expect(h).toContain("line one\nline two");
    expect(h).toContain("logs/marshell.log");
    expect(h).toContain(">Back to the files</button>");
  });
  it("says so while preparing and when it failed", () => {
    expect(view(ON, "preparing")).toContain("Preparing the export…");
    expect(view(ON, "failed")).toContain("Couldn&#x27;t prepare the export.");
  });
  it("keeps focus on a button that is busy: aria-disabled, never disabled, so Escape still reaches the dialog", () => {
    const preparing = view(ON, "preparing");
    expect(preparing).toMatch(/<button[^>]*aria-disabled="true"[^>]*>Prepare export<\/button>/);
    expect(preparing).not.toMatch(/<button[^>]*\sdisabled[^>]*>Prepare export/);
    const saving = renderToStaticMarkup(<DiagnosticsDialogView logging={ON} preview={PREVIEW} viewing={null} saving {...handlers} />);
    expect(saving).toMatch(/<button[^>]*aria-disabled="true"[^>]*>Save…<\/button>/);
    expect(saving).not.toMatch(/<button[^>]*\sdisabled[^>]*>Save…/);
    expect(view(ON, PREVIEW)).not.toContain("aria-disabled");
  });
});
