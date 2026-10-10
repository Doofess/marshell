import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { parseUnifiedDiff } from "../../components/DiffView/diff";
import { unifiedDiff } from "../../lib/diff";
import { DoctorPage } from "./DoctorPage";
import { FixSheet } from "./FixSheet";
import { DOCTOR_PROBLEMS, DOCTOR_REPORT, UNDO_FIXED } from "./fixtures";
import { mergeDiffs } from "./mergeDiff";
import { groupByFile } from "./report";
import { UndoToast } from "./UndoToast";

const at = (ms: number) => UNDO_FIXED.fixedAtMs + ms;

describe("the toast offers Undo only while undo is safe", () => {
  it("renders no empty actions strip when undo is withdrawn and nothing needs a restart", () => {
    const stale = { ...UNDO_FIXED, filesMatch: false };
    const h = renderToStaticMarkup(<UndoToast undo={stale} nowMs={at(2_000)} restart={[]} />);
    expect(h).toContain("toast__text");
    expect(h).not.toContain("toast__actions");
    expect(renderToStaticMarkup(<UndoToast undo={stale} nowMs={at(2_000)} restart={[3]} />)).toContain("Restart them");
  });
  it("has no Undo button once a file changed, and the page says why", () => {
    const stale = { ...UNDO_FIXED, filesMatch: false };
    const h = renderToStaticMarkup(<DoctorPage report={DOCTOR_REPORT} phase="fixed" undo={stale} nowMs={at(2_000)} />);
    expect(h).toContain("A file changed since the fix");
    expect(h).not.toMatch(/toast__btn">Undo</);
  });
  it("puts Undo first in the toast, so a keyboard user reaches it before Restart them", () => {
    const h = renderToStaticMarkup(<DoctorPage report={DOCTOR_REPORT} phase="fixed" undo={UNDO_FIXED} nowMs={at(2_000)} />);
    expect(h.indexOf("toast__btn\">Undo<")).toBeGreaterThan(-1);
    expect(h.indexOf("toast__btn\">Undo<")).toBeLessThan(h.indexOf("Restart them"));
  });
});

describe("the summary after a fix only says all fixed when nothing remains", () => {
  const mixed = { ...DOCTOR_REPORT, items: [...DOCTOR_REPORT.items.filter((i) => i.fixable), ...DOCTOR_PROBLEMS.items] };
  it("says how many were fixed and how many remain", () => {
    const h = renderToStaticMarkup(<DoctorPage report={mixed} phase="fixed" undo={UNDO_FIXED} nowMs={at(2_000)} />);
    expect(h).toContain("7 issues · 4 fixed · 3 remain");
    expect(h).not.toContain("all fixed");
  });
  it("does not mark an item Marshell cannot fix as Fixed", () => {
    const h = renderToStaticMarkup(<DoctorPage report={mixed} phase="fixed" undo={UNDO_FIXED} nowMs={at(2_000)} />);
    expect((h.match(/data-state="fixed"/g) ?? []).length).toBe(4);
  });
  it("still says all fixed when nothing is left", () => {
    const h = renderToStaticMarkup(<DoctorPage report={DOCTOR_REPORT} phase="fixed" undo={UNDO_FIXED} nowMs={at(2_000)} />);
    expect(h).toContain("4 issues · all fixed");
  });
});

describe("chaining the status line keeps the original visible in the fix", () => {
  it("adds the file that keeps the user's own command, and the added lines contain it", () => {
    const groups = groupByFile(DOCTOR_REPORT.items);
    const chain = groups.find((g) => g.file.endsWith("statusline-chain.json"));
    expect(chain).toBeDefined();
    const added = parseUnifiedDiff(chain!.diff).lines.filter((l) => l.kind === "add").map((l) => l.text);
    expect(added.some((t) => t.includes("python ~/.claude/statusline.py"))).toBe(true);
  });
  it("quotes the bridge path, which has a space, in every added command", () => {
    for (const g of groupByFile(DOCTOR_REPORT.items)) {
      for (const l of parseUnifiedDiff(g.diff).lines) {
        if (l.kind === "add" && l.text.includes("marshell-hook")) expect(l.text, l.text).toMatch(/\\"C:\/Users\/Home Office\/\.marshell\/bin\/marshell-hook\.exe\\"/);
      }
    }
  });
});

describe("mergeDiffs", () => {
  const FILE = ["a", "b", "c", "d", "e", "f", "g", "h"].join("\n") + "\n";
  it("merges changes far apart into one diff under one header", () => {
    const one = unifiedDiff("f.txt", FILE, FILE.replace("b", "B"), 1);
    const two = unifiedDiff("f.txt", FILE, FILE.replace("g", "G"), 1);
    const merged = mergeDiffs([one, two]);
    expect((merged.match(/^--- /gm) ?? []).length).toBe(1);
    expect(parseUnifiedDiff(merged).added).toBe(2);
  });
  it("keeps changes that overlap as separate sections, never as one diff that would not apply", () => {
    const one = unifiedDiff("f.txt", FILE, FILE.replace("c", "C"), 2);
    const two = unifiedDiff("f.txt", FILE, FILE.replace("d", "D"), 2);
    const merged = mergeDiffs([one, two]);
    expect((merged.match(/^--- /gm) ?? []).length).toBe(2);
    const p = parseUnifiedDiff(merged);
    expect([p.added, p.removed]).toEqual([2, 2]);
    expect(p.files).toEqual(["f.txt", "f.txt"]);
  });
});

describe("FixSheet", () => {
  it("scrolls its body, and keeps the actions outside it so Cancel and the fix are always reachable", () => {
    const h = renderToStaticMarkup(<FixSheet report={DOCTOR_REPORT} />);
    expect(h).toMatch(/<div class="fix-sheet__body"[^>]*>[\s\S]*<\/div><footer class="fix-sheet__foot">/);
    const body = /<div class="fix-sheet__body"[^>]*>([\s\S]*)<\/div><footer/.exec(h)![1]!;
    expect(body).not.toContain("fix-sheet__foot");
    expect(body).not.toContain("fix-sheet__btn");
  });
  it("names the chain file as its own group", () => {
    const h = renderToStaticMarkup(<FixSheet report={DOCTOR_REPORT} />);
    expect(h).toContain("statusline-chain.json");
  });
});
