import { describe, expect, it } from "vitest";
import { parseOsc133, promptTarget } from "./promptMarks";

describe("parseOsc133", () => {
  it("reads the mark kind", () => {
    expect(parseOsc133("A")).toEqual({ kind: "A", exit: null });
  });
  it("reads the exit code of D", () => {
    expect(parseOsc133("D;1")).toEqual({ kind: "D", exit: 1 });
    expect(parseOsc133("D;0;aid=2")).toEqual({ kind: "D", exit: 0 });
  });
  it("gives no code for D without one or with a broken one", () => {
    expect(parseOsc133("D")).toEqual({ kind: "D", exit: null });
    expect(parseOsc133("D;x")).toEqual({ kind: "D", exit: null });
  });
  it("ignores an unknown mark", () => {
    expect(parseOsc133("Z")).toBeNull();
  });
});

describe("promptTarget", () => {
  const lines = [0, 10, 20];
  it("finds the prompt above the top of the view", () => {
    expect(promptTarget(lines, 15, -1)).toBe(10);
    expect(promptTarget(lines, 10, -1)).toBe(0);
  });
  it("finds the prompt below the top of the view", () => {
    expect(promptTarget(lines, 10, 1)).toBe(20);
  });
  it("is null when there is no prompt in that direction", () => {
    expect(promptTarget(lines, 0, -1)).toBeNull();
    expect(promptTarget(lines, 20, 1)).toBeNull();
    expect(promptTarget([], 5, 1)).toBeNull();
  });
});
