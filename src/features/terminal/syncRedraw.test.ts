import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { privateRedraw, reportMissingRedraw, resetMissingRedrawReport, syncRedraw } from "./syncRedraw";

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

  /** A terminal in or out of synchronized output, with a renderer that records what it drew. */
  function syncTerm(synchronizedOutputMode: boolean, withRenderer = true) {
    const { term, service } = fakeTerm();
    const renderer = {
      drawn: [] as [number, number][],
      renderRows(start: number, end: number) {
        this.drawn.push([start, end]);
      },
    };
    const full = { ...term, modes: { synchronizedOutputMode }, _core: { _renderService: Object.assign(service, { _renderer: { value: withRenderer ? renderer : undefined } }) } };
    return { term: full, service, renderer };
  }

  it("asks the renderer itself while a program holds synchronized output, so the cleared canvas isn't left empty", () => {
    const { term, service, renderer } = syncTerm(true);
    privateRedraw(term)?.(0, 23);
    expect(renderer.drawn).toEqual([[0, 23]]);
    expect(service.drawn).toEqual([]);
  });

  it("reads the mode at each draw", () => {
    const { term, service, renderer } = syncTerm(false);
    const redraw = privateRedraw(term);
    redraw?.(0, 23);
    term.modes.synchronizedOutputMode = true;
    redraw?.(0, 23);
    expect(service.drawn).toEqual([[0, 23]]);
    expect(renderer.drawn).toEqual([[0, 23]]);
  });

  it("falls back to the render service when the renderer can't be reached", () => {
    const { term, service } = syncTerm(true, false);
    privateRedraw(term)?.(0, 23);
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

describe("syncRedraw", () => {
  const grid = (cols: number, rows: number) => ({ cols, rows });
  /** A terminal whose fit changes its grid to `next` and gives its canvas the size xterm computes for it. */
  function setup(next = grid(100, 40)) {
    const term = grid(80, 24);
    const canvas = { width: 800, height: 480 };
    const redraw = vi.fn();
    const fit = vi.fn(() => {
      const changed = term.cols !== next.cols || term.rows !== next.rows;
      Object.assign(term, next);
      if (changed) Object.assign(canvas, { width: next.cols * 10, height: next.rows * 20 });
    });
    return { term, canvas, redraw, fit, sync: syncRedraw(term, fit, redraw, () => canvas) };
  }

  it("draws every row of the new grid when the fit changed it", () => {
    const { sync, redraw } = setup();
    sync.fit();
    expect(redraw.mock.calls).toEqual([[0, 39]]);
  });

  it("draws when only the columns changed", () => {
    const { sync, redraw } = setup(grid(81, 24));
    sync.fit();
    expect(redraw.mock.calls).toEqual([[0, 23]]);
  });

  it("draws nothing when the grid stayed the same", () => {
    const { sync, redraw, fit } = setup(grid(80, 24));
    sync.fit();
    expect(fit).toHaveBeenCalledTimes(1);
    expect(redraw).not.toHaveBeenCalled();
  });

  it("draws again when xterm sets the canvas's size once more after the draw", () => {
    const { sync, redraw, canvas } = setup();
    sync.fit();
    // xterm's device-pixel observer: the true size differs by a pixel at a fractional display scale, and setting it clears the canvas.
    canvas.width += 1;
    sync.deviceResized();
    expect(redraw.mock.calls).toEqual([
      [0, 39],
      [0, 39],
    ]);
    sync.deviceResized();
    expect(redraw).toHaveBeenCalledTimes(2);
  });

  it("draws nothing more when the canvas kept the size it was drawn at", () => {
    const { sync, redraw } = setup();
    sync.fit();
    sync.deviceResized();
    expect(redraw).toHaveBeenCalledTimes(1);
  });

  it("leaves the first paint to xterm: a device size reported before any resize draws nothing", () => {
    const { sync, redraw } = setup();
    sync.deviceResized();
    expect(redraw).not.toHaveBeenCalled();
  });

  it("draws nothing for a device size when there is no canvas", () => {
    const term = grid(80, 24);
    const redraw = vi.fn();
    const sync = syncRedraw(term, () => Object.assign(term, grid(90, 30)), redraw, () => null);
    sync.fit();
    sync.deviceResized();
    expect(redraw.mock.calls).toEqual([[0, 29]]);
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
    // The draw during synchronized output: the renderer behind the service, and the public mode flag.
    expect(bundle.includes("_renderer.value.renderRows("), CHANGED).toBe(true);
    expect(bundle.includes("synchronizedOutputMode"), CHANGED).toBe(true);
  });
});
