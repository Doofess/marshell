import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { fitAndRedraw, privateRedraw, reportMissingRedraw, resetMissingRedrawReport } from "./syncRedraw";

function fakeTerm() {
  const service = {
    drawn: [] as [number, number][],
    _renderRows(start: number, end: number) {
      this.drawn.push([start, end]);
    },
  };
  return { term: { _core: { _renderService: service } }, service };
}

describe("privateRedraw", () => {
  it("calls xterm's private redraw as a method of its render service", () => {
    const { term, service } = fakeTerm();
    const redraw = privateRedraw(term);
    redraw?.(0, 23);
    expect(service.drawn).toEqual([[0, 23]]);
  });

  it("is null when xterm no longer has the function", () => {
    expect(privateRedraw({})).toBeNull();
    expect(privateRedraw({ _core: {} })).toBeNull();
    expect(privateRedraw({ _core: { _renderService: {} } })).toBeNull();
    expect(privateRedraw({ _core: { _renderService: { _renderRows: 1 } } })).toBeNull();
    expect(privateRedraw(null)).toBeNull();
  });
});

describe("fitAndRedraw", () => {
  const grid = (cols: number, rows: number) => ({ cols, rows });

  it("draws every row of the new grid when the fit changed it", () => {
    const term = grid(80, 24);
    const redraw = vi.fn();
    fitAndRedraw(term, () => Object.assign(term, grid(100, 40)), redraw);
    expect(redraw.mock.calls).toEqual([[0, 39]]);
  });

  it("draws when only the columns changed", () => {
    const term = grid(80, 24);
    const redraw = vi.fn();
    fitAndRedraw(term, () => Object.assign(term, grid(81, 24)), redraw);
    expect(redraw.mock.calls).toEqual([[0, 23]]);
  });

  it("draws nothing when the grid stayed the same", () => {
    const term = grid(80, 24);
    const redraw = vi.fn();
    const fit = vi.fn();
    fitAndRedraw(term, fit, redraw);
    expect(fit).toHaveBeenCalledTimes(1);
    expect(redraw).not.toHaveBeenCalled();
  });
});

describe("reportMissingRedraw", () => {
  beforeEach(resetMissingRedrawReport);

  it("reports once, however many terminals ask", () => {
    const log = vi.fn();
    reportMissingRedraw(log);
    reportMissingRedraw(log);
    expect(log).toHaveBeenCalledTimes(1);
    expect(log.mock.calls[0]?.[0]).toContain("RenderService._renderRows");
  });
});

describe("the installed xterm", () => {
  const CHANGED = "xterm changed: if this release carries PR #5529 (6.1 or later), delete syncRedraw's private call and let xterm draw; otherwise find the new name";
  const lib = dirname(createRequire(import.meta.url).resolve("@xterm/xterm"));

  it("is still 6.0, which needs the private call", () => {
    const { version } = JSON.parse(readFileSync(join(lib, "..", "package.json"), "utf8")) as { version: string };
    expect(version, CHANGED).toMatch(/^6\.0\./);
  });

  it.each(["xterm.js", "xterm.mjs"])("still has the private redraw in %s", (file) => {
    const bundle = readFileSync(join(lib, file), "utf8");
    expect(bundle.includes("_renderService"), CHANGED).toBe(true);
    expect(bundle.includes("_renderRows("), CHANGED).toBe(true);
  });
});
