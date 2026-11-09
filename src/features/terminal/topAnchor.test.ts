import { Terminal } from "@xterm/xterm";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { keepFullScreenTop, reportMissingAnchor, resetMissingAnchorReport, topAnchoredFit } from "./topAnchor";

const write = (term: Terminal, data: string) => new Promise<void>((done) => term.write(data, done));
const rowOf = (term: Terminal, text: string) => {
  const buffer = term.buffer.active;
  for (let y = 0; y < term.rows; y++) if (buffer.getLine(buffer.viewportY + y)?.translateToString(true).includes(text)) return y;
  return -1;
};

/** A 20x10 terminal running a full-screen program: "HELLO" on the third row, the cursor parked on `cursorRow` (1-based). */
async function fullScreen(cursorRow: number) {
  const term = new Terminal({ cols: 20, rows: 10, allowProposedApi: true });
  await write(term, `\x1b[?1049h\x1b[3;1HHELLO\x1b[${cursorRow};1H`);
  return term;
}

/** A 20x10 terminal at a shell: ten lines of output, "HELLO" on the third, the cursor on the last row. */
async function shell() {
  const term = new Terminal({ cols: 20, rows: 10, allowProposedApi: true });
  await write(term, "one\r\ntwo\r\nHELLO\r\n4\r\n5\r\n6\r\n7\r\n8\r\n9\r\n$ ");
  return term;
}

describe("keepFullScreenTop", () => {
  it("is needed: xterm alone drops a full-screen program's top rows when its cursor sits on the last one", async () => {
    const term = await fullScreen(10);
    term.resize(20, 8);
    expect(rowOf(term, "HELLO"), "xterm changed: if it now keeps a full-screen program's top rows itself, delete topAnchor").toBe(0);
  });

  it("keeps a full-screen program's text where it was when the terminal loses rows", async () => {
    const term = await fullScreen(10);
    expect(keepFullScreenTop(term, { cols: 20, rows: 8 })).toBe(true);
    term.resize(20, 8);
    expect(rowOf(term, "HELLO")).toBe(2);
    expect(term.buffer.active.cursorY).toBe(7);
  });

  it("keeps the text in place when the cursor is only partly below the new last row", async () => {
    const term = await fullScreen(9);
    keepFullScreenTop(term, { cols: 20, rows: 6 });
    term.resize(20, 6);
    expect(rowOf(term, "HELLO")).toBe(2);
    expect(term.buffer.active.cursorY).toBe(5);
  });

  it("leaves the cursor alone when it stays on the screen", async () => {
    const term = await fullScreen(4);
    keepFullScreenTop(term, { cols: 20, rows: 8 });
    expect(term.buffer.active.cursorY).toBe(3);
  });

  it("leaves a shell's screen alone: its top rows scroll into the history", async () => {
    const term = await shell();
    expect(keepFullScreenTop(term, { cols: 20, rows: 8 })).toBe(true);
    expect(term.buffer.active.cursorY).toBe(9);
    term.resize(20, 8);
    expect(term.buffer.active.baseY).toBe(2);
    expect(rowOf(term, "HELLO")).toBe(0);
  });

  it("does nothing when the rows grow, stay, or can't be measured", async () => {
    for (const next of [{ cols: 20, rows: 12 }, { cols: 20, rows: 10 }, { cols: Number.NaN, rows: 8 }, { cols: 20, rows: Number.NaN }, { cols: 20, rows: 0 }, undefined]) {
      const term = await fullScreen(10);
      expect(keepFullScreenTop(term, next)).toBe(true);
      expect(term.buffer.active.cursorY).toBe(9);
    }
  });

  it("answers false when it is needed and xterm no longer has the private cursor", () => {
    const fullScreenWithout = (core: unknown) => ({ rows: 10, buffer: { active: { type: "alternate" } }, _core: core });
    expect(keepFullScreenTop(fullScreenWithout({}), { cols: 20, rows: 8 })).toBe(false);
    expect(keepFullScreenTop(fullScreenWithout({ _bufferService: { buffers: { alt: {} } } }), { cols: 20, rows: 8 })).toBe(false);
    expect(keepFullScreenTop(fullScreenWithout({ _bufferService: { buffers: { alt: { y: "9" } } } }), { cols: 20, rows: 8 })).toBe(false);
    // Not needed, so nothing is missing.
    expect(keepFullScreenTop(fullScreenWithout({}), { cols: 20, rows: 12 })).toBe(true);
    expect(keepFullScreenTop({ rows: 10, buffer: { active: { type: "normal" } } }, { cols: 20, rows: 8 })).toBe(true);
    expect(keepFullScreenTop(null, { cols: 20, rows: 8 })).toBe(true);
  });
});

describe("topAnchoredFit", () => {
  it("anchors for the size the fit is about to set, then fits", async () => {
    const term = await fullScreen(10);
    const fit = { proposeDimensions: () => ({ cols: 20, rows: 8 }), fit: vi.fn(() => term.resize(20, 8)) };
    const missing = vi.fn();
    topAnchoredFit(term, fit, missing)();
    expect(fit.fit).toHaveBeenCalledTimes(1);
    expect(rowOf(term, "HELLO")).toBe(2);
    expect(missing).not.toHaveBeenCalled();
  });

  it("measures nothing for a shell: the fit addon's own measurement is the only one", async () => {
    const term = await shell();
    const fit = { proposeDimensions: vi.fn(() => ({ cols: 20, rows: 8 })), fit: vi.fn() };
    topAnchoredFit(term, fit, vi.fn())();
    expect(fit.proposeDimensions).not.toHaveBeenCalled();
    expect(fit.fit).toHaveBeenCalledTimes(1);
  });

  it("still fits, and says so, when xterm's private cursor is gone", () => {
    const term = { rows: 10, buffer: { active: { type: "alternate" } }, _core: {} };
    const fit = { proposeDimensions: () => ({ cols: 20, rows: 8 }), fit: vi.fn() };
    const missing = vi.fn();
    topAnchoredFit(term, fit, missing)();
    expect(fit.fit).toHaveBeenCalledTimes(1);
    expect(missing).toHaveBeenCalledTimes(1);
  });
});

describe("reportMissingAnchor", () => {
  beforeEach(resetMissingAnchorReport);

  it("reports once, however many terminals ask", () => {
    const log = vi.fn();
    reportMissingAnchor(log);
    reportMissingAnchor(log);
    expect(log).toHaveBeenCalledTimes(1);
    expect(log.mock.calls[0]?.[0]).toContain("BufferService.buffers.alt.y");
  });
});
