import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { nameContainsLabel } from "../../lib/labelInName";
import { DoctorPage } from "./DoctorPage";
import { showProgress } from "./doctorUi";
import { DOCTOR_CLEAR, DOCTOR_CLI, DOCTOR_KEEP, DOCTOR_LOADS, DOCTOR_PROBLEMS, DOCTOR_REPORT, UNDO_FIXED } from "./fixtures";
import { initialSelection } from "./report";
import type { DoctorReport } from "./types";

const noop = () => {};
type Props = Parameters<typeof DoctorPage>[0];
const page = (report: DoctorReport | null, phase: Props["phase"], extra: Partial<Props> = {}) =>
  renderToStaticMarkup(
    <DoctorPage report={report} phase={phase} selected={report ? initialSelection(report) : new Set()} nowMs={(report?.checked_at_ms ?? 0) + 12_000} onToggle={noop} onFix={noop} onRunAgain={noop} onUndo={noop} onShowDifferences={noop} onOpenAgents={noop} cliRunning={false} onRunCliDoctors={noop} measuring={false} onMeasure={noop} {...extra} />,
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
    expect(h).toContain("Checking Claude Code, Codex, Antigravity, Qwen Code and Copilot CLI…");
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
  });

  describe("Measure MCP servers", () => {
    it("sits between Run CLI doctors and Run again", () => {
      expect(page(DOCTOR_REPORT, "ready")).toMatch(/>Run CLI doctors<\/button>[\s\S]*>Measure MCP servers<\/button>[\s\S]*>Run again</);
    });
    it("while measuring, reads Measuring MCP servers… with aria-disabled", () => {
      const h = page(DOCTOR_REPORT, "ready", { measuring: true });
      expect(h).toMatch(/<button[^>]*aria-disabled="true"[^>]*>Measuring MCP servers…<\/button>/);
    });
    it("is absent while fixing", () => {
      const h = page(DOCTOR_REPORT, "fixing");
      expect(h).not.toContain("Measure MCP servers");
      expect(h).not.toContain("Measuring MCP servers");
    });
    it("shows its line below the CLI line", () => {
      const h = page(DOCTOR_REPORT, "ready");
      expect(h).toContain(`<p class="doctor__cli">MCP servers haven&#x27;t been measured.`);
      expect(h.indexOf("CLI doctors haven")).toBeLessThan(h.indexOf("MCP servers haven"));
    });
  });

  describe("CLI items", () => {
    it("renders a CLI item with no checkbox and its cannot_fix text", () => {
      const h = page(DOCTOR_CLI, "ready");
      expect((h.match(/type="checkbox"/g) ?? []).length).toBe(0);
      expect(h).toContain("Qwen Code has no doctor command");
      expect(h).toContain("Reinstall Codex to restore the sandbox helpers.");
    });
    it("a CLI with no doctor command says so under Info, Copilot CLI too", () => {
      const h = page(DOCTOR_CLI, "ready");
      expect(h).toMatch(/>Info <span class="doctor__group-count">/);
      const info = h.slice(h.search(/>Info <span class="doctor__group-count">/));
      expect(info).toContain("Copilot CLI has no doctor command.");
      expect(info).toContain("Qwen Code has no doctor command.");
    });
  });

  describe("Keep my file and Stop keeping", () => {
    it("a row with action keep has a Keep my file button after its checkbox and text, named with the title", () => {
      const h = page(DOCTOR_KEEP, "ready", { onKeep: noop });
      expect(h).toMatch(/<button type="button"[^>]*aria-label="Keep my file: [^"]*isn&#x27;t Marshell&#x27;s file any more"[^>]*>Keep my file<\/button>/);
      expect(h.indexOf(`type="checkbox"`)).toBeLessThan(h.indexOf(">Keep my file<"));
      expect(h.indexOf("holds something Marshell")).toBeLessThan(h.indexOf(">Keep my file<"));
    });
    it("a row with action stopKeeping has a Stop keeping button and no checkbox", () => {
      const h = page(DOCTOR_KEEP, "ready", { onKeep: noop });
      expect(h).toMatch(/aria-label="Stop keeping: Copilot CLI&#x27;s hooks file has your changes"[^>]*>Stop keeping<\/button>/);
      expect((h.match(/type="checkbox"/g) ?? []).length).toBe(1);
    });
    it("rows without an action have neither button", () => {
      const h = page(DOCTOR_REPORT, "ready");
      expect(h).not.toContain("Keep my file");
      expect(h).not.toContain("Stop keeping");
    });
    it("the busy row's button keeps the focus (aria-disabled) and reads Keeping…, and the unchecked row stays unchecked", () => {
      const h = page(DOCTOR_KEEP, "ready", { onKeep: noop, keeping: "hooksfile.copilot" });
      expect(h).toMatch(/<button type="button"[^>]*aria-disabled="true"[^>]*>Keeping…<\/button>/);
      expect(h).not.toMatch(/<button[^>]*disabled=""[^>]*>Keeping…/);
      // Both rows are one file's: the button that replaces the pressed one is found by this mark.
      expect((h.match(/data-doctor-keep="copilot\|"/g) ?? []).length).toBe(2);
      expect(h).toContain(">Stop keeping<");
      expect((h.match(/checked=""/g) ?? []).length).toBe(0);
    });
    it("while a fix is applied, the Keep my file and Stop keeping buttons are aria-disabled, as the checkboxes are disabled", () => {
      const h = page(DOCTOR_KEEP, "fixing", { onKeep: noop });
      expect(h).toMatch(/<button type="button"[^>]*aria-disabled="true"[^>]*>Keep my file<\/button>/);
      expect(h).toMatch(/<button type="button"[^>]*aria-disabled="true"[^>]*>Stop keeping<\/button>/);
      // After a fix the checkboxes wait for the next run, and the two buttons work: a press starts that run.
      for (const phase of ["ready", "fixed"] as const) {
        expect(page(DOCTOR_KEEP, phase, { onKeep: noop })).not.toMatch(/<button type="button"[^>]*aria-disabled="true"[^>]*>(Keep my file|Stop keeping)<\/button>/);
      }
    });
    it("a failed press shows the server's message in the page's error line", () => {
      const h = page(DOCTOR_KEEP, "ready", { onKeep: noop, keepError: "This item can't be fixed any more. Run the check again." });
      expect(h).toMatch(/<p class="doctor__failed"[^>]*>[^<]*Run the check again\./);
    });
  });
  describe("instruction files", () => {
    const rows = (h: string) => h.split('<li class="doctor__row"').slice(1);
    it("a row with loads has a closed disclosure", () => {
      const row = rows(page(DOCTOR_LOADS, "ready"))[0]!;
      expect(row).toMatch(/<details class="doctor__loads"><summary[^>]*>Show the files<\/summary>/);
      expect(row).not.toMatch(/<details[^>]*\sopen/);
      expect(row).toContain('<p class="doctor__load-cli">Claude Code: about 2.4k tokens at start</p>');
      expect(row).toContain('<p class="doctor__load-cli">Codex: about 600 tokens at start</p>');
      expect(row).toContain('<li data-when="notLoaded" style="--depth:0"><code dir="ltr">AGENTS.md</code> <span>not loaded: AGENTS.override.md is here</span></li>');
      expect(row).toContain('<li data-when="start" style="--depth:1"><code dir="ltr">AGENTS.md</code> <span>about 600 tokens, imported by CLAUDE.md</span></li>');
    });
    it("a row without loads has no disclosure", () => {
      expect(rows(page(DOCTOR_LOADS, "ready"))[1]).not.toContain("<details");
      expect(page(DOCTOR_REPORT, "ready")).not.toContain("<details");
    });
    it("the summary is described by its row's title", () => {
      const row = rows(page(DOCTOR_LOADS, "ready"))[0]!;
      const titleId = /<p class="doctor__title" id="([^"]+)"/.exec(row)?.[1];
      expect(titleId).toBeTruthy();
      expect(/<summary aria-describedby="([^"]+)"/.exec(row)?.[1]).toBe(titleId);
    });
    it("a load's notes show under its heading", () => {
      const row = rows(page(DOCTOR_LOADS, "ready"))[0]!;
      expect(row).toContain('Codex: about 600 tokens at start</p><p class="doctor__load-note">Marshell stopped looking for files in subfolders after 10,000 entries.</p><ul class="doctor__load-files">');
    });
  });
});
