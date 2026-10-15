import { describe, expect, it } from "vitest";
import { contrast } from "../../design/contrast";
import { MIN_CONTRAST, PALETTES } from "./palettes";
import { SCRIPTED_LINES, SCRIPTED_SESSION, SCRIPTED_SIZE } from "./scriptedSession";

const rgbs = [...SCRIPTED_SESSION.matchAll(/\x1b\[38;2;(\d+);(\d+);(\d+)m/g)].map((m) => `#${[1, 2, 3].map((i) => Number(m[i]).toString(16).padStart(2, "0")).join("")}`);

describe("scripted session", () => {
  it("fits its stated size without wrapping", () => {
    expect(SCRIPTED_LINES.length).toBeLessThanOrEqual(SCRIPTED_SIZE.rows);
    for (const l of SCRIPTED_LINES) {
      const visible = l.replace(/\x1b\[[0-9;]*m/g, "");
      expect([...visible].length, visible).toBeLessThanOrEqual(SCRIPTED_SIZE.cols);
    }
  });
  it("ends with a line ending and uses CRLF between lines", () => {
    expect(SCRIPTED_SESSION.endsWith("\r\n")).toBe(true);
    expect(SCRIPTED_SESSION).not.toMatch(/[^\r]\n/);
  });
  it("uses the 16 palette colours, so a theme switch visibly changes them", () => {
    expect(SCRIPTED_SESSION).toMatch(/\x1b\[3[1-6]m/);
    expect(SCRIPTED_SESSION).toMatch(/\x1b\[9[0-7]m/);
  });
  it("paints some 24-bit colours that fail 4.5:1 on a dark terminal and some on a light one (so protection has work to do)", () => {
    expect(rgbs.length).toBeGreaterThan(2);
    expect(rgbs.some((c) => contrast(c, PALETTES.dark.background!) < MIN_CONTRAST)).toBe(true);
    expect(rgbs.some((c) => contrast(c, PALETTES.light.background!) < MIN_CONTRAST)).toBe(true);
  });
  it("contains the beats of a Claude Code session", () => {
    const visible = SCRIPTED_SESSION.replace(/\x1b\[[0-9;]*m/g, "");
    for (const s of ["Claude Code", "retry", "Read(", "Update(", "Bash(npm test)", "passed"]) expect(visible).toContain(s);
  });
  it("draws its welcome box exactly 60 columns wide on every row", () => {
    const box = SCRIPTED_LINES.filter((l) => /^\x1b\[38;2;217;119;87m[╭│╰]/.test(l)).map((l) => [...l.replace(/\x1b\[[0-9;]*m/g, "")].length);
    expect(box.length).toBe(4);
    for (const w of box) expect(w).toBe(60);
  });
});
