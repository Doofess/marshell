import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { nameContainsLabel } from "../../lib/labelInName";
import { DoctorPage } from "./DoctorPage";
import { showProgress } from "./doctorUi";
import { DOCTOR_CLEAR, DOCTOR_PROBLEMS, DOCTOR_REPORT, UNDO_FIXED } from "./fixtures";

describe("DoctorPage", () => {
  it("ready: counts the issues and offers the fix and a re-run", () => {
    const h = renderToStaticMarkup(<DoctorPage report={DOCTOR_REPORT} phase="ready" />);
    expect(h).toContain("4 issues");
    expect(h).toContain("Fix 4 issues");
    expect(h).toContain("Run again");
  });
  it("tells every severity in words as well as a glyph, and says why a problem cannot be fixed", () => {
    const h = renderToStaticMarkup(<DoctorPage report={DOCTOR_PROBLEMS} phase="ready" />);
    expect(h).toMatch(/Error/);
    expect(h).toMatch(/Warning/);
    expect(h).toContain("Marshell never edits a file it can&#x27;t read.");
    expect(h).not.toContain("Fix 0");
    expect(h).toContain("Nothing to fix");
  });
  it("lists information (managed settings) with no fix control", () => {
    const h = renderToStaticMarkup(<DoctorPage report={DOCTOR_REPORT} phase="ready" />);
    expect(h).toContain("managed-settings.json");
    expect(h).toMatch(/Information/);
  });
  it("running: says what is being checked and is busy, with no live region", () => {
    const h = renderToStaticMarkup(<DoctorPage report={DOCTOR_REPORT} phase="running" elapsedMs={400} />);
    expect(h).toContain("Checking Claude Code, Codex and Gemini");
    expect(h).toContain('aria-busy="true"');
    expect(h).not.toMatch(/aria-live|role="status"|role="alert"/);
  });
  it("shows nothing for the first 150 ms", () => {
    expect(showProgress(150)).toBe(false);
    expect(showProgress(151)).toBe(true);
    const h = renderToStaticMarkup(<DoctorPage report={DOCTOR_REPORT} phase="running" elapsedMs={100} />);
    expect(h).not.toContain("Checking Claude Code");
  });
  it("all clear: says so and offers only Run again", () => {
    const h = renderToStaticMarkup(<DoctorPage report={DOCTOR_CLEAR} phase="ready" />);
    expect(h).toContain("All clear");
    expect(h).not.toContain("Fix ");
  });
  it("failed: gives the reason and a way to run it again", () => {
    const h = renderToStaticMarkup(<DoctorPage report={DOCTOR_REPORT} phase="failed" failure="The core is not running." />);
    expect(h).toContain("The doctor could not run: The core is not running.");
    expect(h).toContain("Run again");
  });
  it("fixed: ticks each row, shows the receipt toast with Restart them and Undo, and keeps Undo last fix in the header afterwards", () => {
    const toast = renderToStaticMarkup(<DoctorPage report={DOCTOR_REPORT} phase="fixed" undo={UNDO_FIXED} nowMs={UNDO_FIXED.fixedAtMs + 2_000} />);
    expect(toast).toContain("Fixed 4 issues. 2 sessions pick this up after a restart.");
    expect(toast).toContain("Restart them");
    expect((toast.match(/data-state="fixed"/g) ?? []).length).toBe(4);
    const later = renderToStaticMarkup(<DoctorPage report={DOCTOR_REPORT} phase="fixed" undo={UNDO_FIXED} nowMs={UNDO_FIXED.fixedAtMs + 60_000} />);
    expect(later).not.toContain("Restart them");
    expect(later).toContain("Undo last fix");
  });
  it("a stale undo says why and shows no undo button", () => {
    const h = renderToStaticMarkup(<DoctorPage report={DOCTOR_REPORT} phase="fixed" undo={{ ...UNDO_FIXED, filesMatch: false }} nowMs={UNDO_FIXED.fixedAtMs + 60_000} />);
    expect(h).toContain("A file changed since the fix");
    expect(h).not.toContain("Undo last fix");
  });
  it("names its buttons from the text they show", () => {
    const h = renderToStaticMarkup(<DoctorPage report={DOCTOR_REPORT} phase="ready" />);
    for (const b of h.match(/<button[\s\S]*?<\/button>/g) ?? []) expect(nameContainsLabel(b)).toBe(true);
  });
  it("names the sessions that need a restart by number", () => {
    const h = renderToStaticMarkup(<DoctorPage report={DOCTOR_REPORT} phase="ready" />);
    expect(h).toContain("Sessions 3 and 5 pick this up after a restart");
  });
});
