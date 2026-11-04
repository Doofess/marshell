import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { UNDO_FIXED } from "./fixtures";
import { UndoToast } from "./UndoToast";
import { TOAST_MS, toastVisible, undoOffer, undoState } from "./undo";

const noop = () => {};
const u = undoState({ ...UNDO_FIXED, fixed_at_ms: 1_000 });
const at = (ms: number) => UNDO_FIXED.fixed_at_ms + ms;

describe("undo", () => {
  it("reads the state from DoctorLastFix", () => {
    expect(u).toEqual({ fixedAtMs: 1_000, count: 3, filesMatch: true });
  });
  it("shows the toast for 10 seconds, then leaves Undo last fix in the header", () => {
    expect(TOAST_MS).toBe(10_000);
    expect(toastVisible(u, 1_000 + 9_999)).toBe(true);
    expect(toastVisible(u, 1_000 + 10_000)).toBe(false);
    expect(undoOffer(u, 2_000)).toEqual({ kind: "toast", text: "Undo" });
    expect(undoOffer(u, 20_000)).toEqual({ kind: "header", text: "Undo last fix" });
  });
  it("stops offering undo when the files no longer match, and says why", () => {
    expect(undoOffer({ ...u, filesMatch: false }, 20_000)).toEqual({ kind: "stale", text: "A file changed since the fix, so undo would overwrite it. Show differences." });
  });
});

describe("UndoToast", () => {
  it("says what was fixed and offers Undo then Restart them when sessions need a restart", () => {
    const h = renderToStaticMarkup(<UndoToast undo={UNDO_FIXED} nowMs={at(2_000)} onUndo={noop} onRestart={noop} />);
    expect(h).toContain("Fixed 3 issues. 2 sessions pick this up after a restart.");
    expect(h.indexOf('toast__btn">Undo<')).toBeGreaterThan(-1);
    expect(h.indexOf('toast__btn">Undo<')).toBeLessThan(h.indexOf("Restart them"));
  });
  it("offers Restart them only when restart is not empty", () => {
    const h = renderToStaticMarkup(<UndoToast undo={{ ...UNDO_FIXED, restart: [] }} nowMs={at(2_000)} onUndo={noop} onRestart={noop} />);
    expect(h).not.toContain("Restart them");
    expect(h).toContain(">Undo<");
  });
  it("renders no actions strip when undo is withdrawn and nothing needs a restart", () => {
    const h = renderToStaticMarkup(<UndoToast undo={{ ...UNDO_FIXED, restart: [], files_match: false }} nowMs={at(2_000)} onUndo={noop} onRestart={noop} />);
    expect(h).toContain("toast__text");
    expect(h).not.toContain("toast__actions");
  });
  it("is gone after 10 seconds", () => {
    expect(renderToStaticMarkup(<UndoToast undo={UNDO_FIXED} nowMs={at(10_000)} onUndo={noop} onRestart={noop} />)).toBe("");
  });
});
