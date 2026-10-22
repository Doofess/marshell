import { afterEach, describe, expect, it, vi } from "vitest";
import type { GlyphKind } from "../../components/StatusGlyph/glyphs";
import { mountHook } from "../../test/hooks";
import type { RowModel } from "../sidebar/types";
import { attentionFlashes, useRowFlash } from "./rowFlash";

vi.mock("react", async () => ({ ...await vi.importActual("react"), ...(await import("../../test/hooks")).hooks }));
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

const row = (status: GlyphKind, extra: Partial<RowModel> = {}): RowModel => ({
  id: "one", number: 1, name: "Session", project: "marshell", projectKey: "marshell",
  agent: "claude", status, phrase: "", ...extra,
});

describe("attentionFlashes", () => {
  it("does not flash the first snapshot", () => {
    expect(attentionFlashes(null, [row("needs-permission")])).toEqual([]);
  });
  // The live app reads every finished turn as done-seen (toRow's GLYPH), so a turn ending into done-seen is Done and flashes.
  it.each<GlyphKind>(["needs-permission", "needs-question", "done-unseen", "done-seen", "error"])("flashes working → %s", (status) => {
    expect(attentionFlashes(new Map([["one", "working"]]), [row(status)])).toEqual(["one"]);
  });
  it.each<[GlyphKind, GlyphKind]>([
    ["working", "idle"], ["idle", "working"], ["needs-permission", "needs-permission"], ["done-unseen", "done-seen"],
  ])("does not flash %s → %s", (from, to) => {
    expect(attentionFlashes(new Map([["one", from]]), [row(to)])).toEqual([]);
  });
  it("skips a muted row entering needs-permission", () => {
    expect(attentionFlashes(new Map([["one", "working"]]), [row("needs-permission", { muted: true })])).toEqual([]);
  });
  it("skips a new row", () => {
    expect(attentionFlashes(new Map([["other", "working"]]), [row("needs-permission")])).toEqual([]);
  });
  it("does not flash ten rows restored together", () => {
    const rows = Array.from({ length: 10 }, (_, i) => row("needs-permission", { id: String(i) }));
    expect(attentionFlashes(new Map(), rows)).toEqual([]);
  });
});

function frames() {
  vi.useFakeTimers();
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => setTimeout(() => callback(performance.now()), 16));
  vi.stubGlobal("cancelAnimationFrame", (id: ReturnType<typeof setTimeout>) => clearTimeout(id));
}

describe("useRowFlash", () => {
  it("clears a flash after 600 ms without changing the input rows", () => {
    frames();
    let rows = [row("working")];
    const h = mountHook(() => useRowFlash(rows));
    expect(h.current[0]!.flash).toBeFalsy();
    rows = [row("needs-permission")];
    h.rerender();
    expect(h.current[0]!.flash).toBe(true);
    expect(rows[0]!.flash).toBeUndefined();
    vi.advanceTimersByTime(599);
    expect(h.flush()[0]!.flash).toBe(true);
    vi.advanceTimersByTime(1);
    expect(h.flush()[0]!.flash).toBeFalsy();
    h.unmount();
  });

  it("restarts a flash next frame when a row needs you again within 600 ms", () => {
    frames();
    let rows = [row("working")];
    const h = mountHook(() => useRowFlash(rows));
    rows = [row("needs-permission")];
    h.rerender();
    vi.advanceTimersByTime(200);
    rows = [row("working")];
    h.rerender();
    rows = [row("needs-question")];
    expect(h.rerender()[0]!.flash).toBeFalsy();
    vi.advanceTimersByTime(16);
    expect(h.flush()[0]!.flash).toBe(true);
    vi.advanceTimersByTime(384);
    expect(h.flush()[0]!.flash).toBe(true);
    vi.advanceTimersByTime(215);
    expect(h.flush()[0]!.flash).toBe(true);
    vi.advanceTimersByTime(1);
    expect(h.flush()[0]!.flash).toBeFalsy();
    h.unmount();
  });

  it("keeps independent expiry times through unrelated snapshots", () => {
    frames();
    let rows = [row("working"), row("working", { id: "two" })];
    const h = mountHook(() => useRowFlash(rows));
    rows = [row("error"), row("working", { id: "two" })];
    h.rerender();
    vi.advanceTimersByTime(200);
    rows = [row("error"), row("done-unseen", { id: "two" })];
    h.rerender();
    vi.advanceTimersByTime(400);
    expect(h.flush().map((r) => !!r.flash)).toEqual([false, true]);
    vi.advanceTimersByTime(200);
    expect(h.flush().map((r) => !!r.flash)).toEqual([false, false]);
    h.unmount();
  });

  it("cancels pending frames and timers on unmount", () => {
    frames();
    let rows = [row("working"), row("working", { id: "two" })];
    const h = mountHook(() => useRowFlash(rows));
    rows = [row("error"), row("error", { id: "two" })];
    h.rerender();
    rows = [row("needs-permission"), row("error", { id: "two" })];
    h.rerender();
    h.unmount();
    expect(vi.getTimerCount()).toBe(0);
  });
});
