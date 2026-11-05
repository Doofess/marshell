import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { nameContainsLabel } from "../../lib/labelInName";
import { DoctorPage } from "./DoctorPage";
import { showProgress } from "./doctorUi";
import { DOCTOR_CLEAR, DOCTOR_CLI, DOCTOR_PROBLEMS, DOCTOR_REPORT, UNDO_FIXED } from "./fixtures";
import { initialSelection } from "./report";
import type { DoctorReport } from "./types";

const noop = () => {};
type Props = Parameters<typeof DoctorPage>[0];
const page = (report: DoctorReport | null, phase: Props["phase"], extra: Partial<Props> = {}) =>
  renderToStaticMarkup(
    <DoctorPage report={report} phase={phase} selected={report ? initialSelection(report) : new Set()} nowMs={(report?.checked_at_ms ?? 0) + 12_000} onToggle={noop} onFix={noop} onRunAgain={noop} onUndo={noop} onShowDifferences={noop} onOpenAgents={noop} cliRunning={false} onRunCliDoctors={noop} {...extra} />,
  );
const at = (ms: number) => UNDO_FIXED.fixed_at_ms + ms;

describe("DoctorPage", () => {
  it("groups under Broken, Waste and Info headings with counts, in that order, leaving out an empty one", () => {
    const h = page(DOCTOR_REPORT, "ready");
    const heads = [...h.matchAll(/<h2[^>]*class="doctor__group-title"[^>]*>(.*?)<\/h2>/g)].map((m) => m[1]!.replace(/<[^>]+>/g, ""));
    expect(heads).toEqual(["Broken 3", "Info 2"]);
  });
  it("drops empty groups", () => {
    const h = page(DOCTOR_PROBLEMS, "ready");
    expect(h).toContain("Broken");
    expect(h).not.toContain("Waste");
    expect(h).not.toContain(">Info<");
  });
  it("has a checkbox for fixable rows only, checked per selected, labelled by the title", () => {
    const h = page(DOCTOR_REPORT, "ready", { selected: new Set(["hooks.claude"]) });
    expect((h.match(/type="checkbox"/g) ?? []).length).toBe(3);
    expect(h).toMatch(/<label[^>]*><input[^>]*type="checkbox"[^>]*checked=""[^>]*\/>[\s\S]*?Approve from anywhere is not installed/);
    expect((h.match(/checked=""/g) ?? []).length).toBe(1);
    expect(h).not.toMatch(/Antigravity&#x27;s settings file does not parse[\s\S]{0,40}type="checkbox"/);
  });
  it("says why a row cannot be fixed and offers Open Agents on rows that ask for it", () => {
    const h = page(DOCTOR_REPORT, "ready");
    expect(h).toContain("The file does not parse (line 14).");
    expect((h.match(/>Open Agents</g) ?? []).length).toBe(1);
  });
  it("counts the selected fixable items in the fix button", () => {
    expect(page(DOCTOR_REPORT, "ready", { selected: new Set(["hooks.claude", "retention.claude"]) })).toContain("Fix 2 issues");
    expect(page(DOCTOR_REPORT, "ready", { selected: new Set(["hooks.claude"]) })).toContain("Fix 1 issue<");
  });
  it("disables the fix button at 0 selected", () => {
    const h = page(DOCTOR_REPORT, "ready", { selected: new Set() });
    expect(h).toMatch(/<button[^>]*disabled=""[^>]*>Fix 0 issues<\/button>/);
  });
  // Found in the 7a final review: with only Codex trust and the retention item left, the page offered no fix at all.
  it("offers the fix button when only an Info item can be fixed", () => {
    const retention = DOCTOR_REPORT.items.find((i) => i.id === "retention.claude")!;
    const r: DoctorReport = { ...DOCTOR_PROBLEMS, items: [...DOCTOR_PROBLEMS.items, retention] };
    const h = page(r, "ready");
    expect(h).toMatch(/<button[^>]*disabled=""[^>]*>Fix 0 issues<\/button>/);
    expect(h).not.toContain("Nothing to fix");
    expect(page(r, "ready", { selected: new Set(["retention.claude"]) })).toContain("Fix 1 issue<");
  });
  it("offers no fix button when nothing can be fixed", () => {
    const h = page(DOCTOR_PROBLEMS, "ready");
    expect(h).not.toContain("Fix 0");
    expect(h).toContain("Nothing to fix");
  });
  it("uses no severity words", () => {
    const h = page(DOCTOR_REPORT, "ready").replace(/<code[\s\S]*?<\/code>/g, "");
    expect(h).not.toMatch(/\bError\b|\bWarning\b|\bInformation\b/);
  });
  it("summarises the issues and what can be fixed", () => {
    const h = page(DOCTOR_REPORT, "ready");
    expect(h).toContain("3 issues");
    expect(h).toContain("2 can be fixed");
    expect(h).toContain("Run again");
  });
  it("running: says what is being checked and is busy, with no live region", () => {
    const h = page(null, "running", { elapsedMs: 400 });
    expect(h).toContain("Checking Claude Code, Codex, Antigravity and Qwen Code");
    expect(h).toContain('aria-busy="true"');
    expect(h).not.toMatch(/aria-live|role="status"|role="alert"/);
  });
  it("shows nothing for the first 150 ms", () => {
    expect(showProgress(150)).toBe(false);
    expect(showProgress(151)).toBe(true);
    expect(page(null, "running", { elapsedMs: 100 })).not.toContain("Checking Claude Code");
  });
  it("all clear: says so and offers only Run again", () => {
    const h = page(DOCTOR_CLEAR, "ready");
    expect(h).toContain("All clear");
    expect(h).not.toContain("Fix ");
  });
  it("failed: gives the reason and a way to run it again", () => {
    const h = page(null, "failed", { failure: "The core is not running." });
    expect(h).toContain("The doctor could not run: The core is not running.");
    expect(h).toContain("Run again");
  });
  it("fixed: marks the fixed rows, shows the receipt, then keeps Undo last fix in the header", () => {
    const toast = page(DOCTOR_REPORT, "fixed", { undo: UNDO_FIXED, nowMs: at(2_000) });
    expect(toast).toContain("Fixed 3 issues. 2 sessions pick this up after a restart.");
    expect(toast).toContain("Restart them");
    expect((toast.match(/data-state="fixed"/g) ?? []).length).toBe(3);
    expect(toast).toContain("3 issues · 2 fixed · 1 remains");
    const later = page(DOCTOR_REPORT, "fixed", { undo: UNDO_FIXED, nowMs: at(60_000) });
    expect(later).not.toContain("Restart them");
    expect(later).toContain("Undo last fix");
  });
  it("a stale undo says why and offers Show differences, with no undo button", () => {
    const h = page(DOCTOR_REPORT, "fixed", { undo: { ...UNDO_FIXED, files_match: false }, nowMs: at(60_000) });
    expect(h).toContain("A file changed since the fix");
    expect(h).toContain(">Show differences<");
    expect(h).not.toContain("Undo last fix");
  });
  it("names its buttons from the text they show", () => {
    for (const b of page(DOCTOR_REPORT, "ready").match(/<button[\s\S]*?<\/button>/g) ?? []) expect(nameContainsLabel(b)).toBe(true);
  });

  describe("CLI doctors", () => {
    it("shows the button before Run again and the status line under the summary, in ready", () => {
      const h = page(DOCTOR_REPORT, "ready");
      expect(h).toMatch(/>Run CLI doctors<\/button>[\s\S]*>Run again</);
      expect(h).toContain(`<p class="doctor__cli">CLI doctors haven&#x27;t run. Codex&#x27;s doctor contacts OpenAI.</p>`);
      expect(h.indexOf("doctor__summary")).toBeLessThan(h.indexOf("doctor__cli"));
    });
    it("says when they ran, from the report", () => {
      const h = page({ ...DOCTOR_REPORT, cli_checked_at_ms: DOCTOR_REPORT.checked_at_ms + 2_000 }, "ready");
      expect(h).toContain("CLI doctors ran 10 s ago.");
    });
    it("while running, reads Running CLI doctors… with aria-disabled and stays focusable", () => {
      const h = page(DOCTOR_REPORT, "ready", { cliRunning: true });
      expect(h).toMatch(/<button[^>]*aria-disabled="true"[^>]*>Running CLI doctors…<\/button>/);
      expect(h).not.toMatch(/<button[^>]*disabled=""[^>]*>Running CLI doctors/);
    });
    it("is absent while fixing", () => {
      const h = page(DOCTOR_REPORT, "fixing");
      expect(h).not.toContain("CLI doctors…");
      expect(h).not.toContain(">Run CLI doctors<");
    });
    it("renders a CLI item with no checkbox and its cannot_fix text", () => {
      const h = page(DOCTOR_CLI, "ready");
      expect((h.match(/type="checkbox"/g) ?? []).length).toBe(0);
      expect(h).toContain("Qwen Code has no doctor command");
      expect(h).toContain("Marshell only reports what the CLI&#x27;s own doctor says.");
    });
  });
});
