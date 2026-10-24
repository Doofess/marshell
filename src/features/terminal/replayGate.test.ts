import { describe, expect, it } from "vitest";
import { isQueryReply, ReplayGate } from "./replayGate";

const colourReply = "\x1b]10;rgb:e5e5/e5e5/e5e5\x1b\\\x1b]11;rgb:0000/0000/0000\x1b\\";

describe("isQueryReply", () => {
  it.each([
    ["the colour replies from #60", colourReply],
    ["a colour reply ended by BEL", "\x1b]11;rgb:1e1e/1e1e/1e1e\x07"],
    ["a cursor position", "\x1b[12;40R"],
    ["a primary device attributes reply", "\x1b[?1;2c"],
    ["a secondary device attributes reply", "\x1b[>0;276;0c"],
    ["a mode report", "\x1b[?2004;1$y"],
    ["a setting report", "\x1bP1$r0m\x1b\\"],
    ["a status report", "\x1b[0n"],
    ["a window report", "\x1b[8;30;120t"],
    ["keyboard protocol flags", "\x1b[?0u"],
    ["a run of replies", "\x1b[12;40R\x1b[?1;2c\x1b]11;rgb:0000/0000/0000\x07"],
  ])("is true for %s", (_name, data) => {
    expect(isQueryReply(data)).toBe(true);
  });

  it.each([
    ["a letter", "a"],
    ["Enter", "\r"],
    ["an arrow key", "\x1b[A"],
    ["a focus report", "\x1b[I"],
    ["a mouse report", "\x1b[<35;10;5M"],
    ["an empty string", ""],
    ["a cut-off OSC reply", "\x1b]11;rgb:0"],
    ["a cut-off CSI reply", "\x1b[12;40"],
    ["a paste holding an OSC sequence", "\x1b[200~\x1b]11;rgb:0000/0000/0000\x07\x1b[201~"],
    ["a reply followed by a key", "\x1b[12;40Ry"],
    ["a key followed by a reply", "y\x1b[12;40R"],
  ])("is false for %s", (_name, data) => {
    expect(isQueryReply(data)).toBe(false);
  });
});

describe("ReplayGate", () => {
  it("the gate starts open", () => {
    expect(new ReplayGate().allows(colourReply)).toBe(true);
  });
  it("a reply to the replay is dropped", () => {
    const gate = new ReplayGate();
    gate.live(100);
    gate.parsed(40);
    expect(gate.allows(colourReply)).toBe(false);
  });
  it("typing during the replay passes", () => {
    const gate = new ReplayGate();
    gate.live(100);
    gate.parsed(40);
    expect(gate.allows("x")).toBe(true);
    expect(gate.allows("\x1b[I")).toBe(true);
  });
  it("a reply after the live offset passes", () => {
    const gate = new ReplayGate();
    gate.live(100);
    gate.parsed(100);
    expect(gate.allows(colourReply)).toBe(true);
  });
  it("a second live frame moves the boundary", () => {
    const gate = new ReplayGate();
    gate.live(100);
    gate.parsed(100);
    gate.live(300);
    expect(gate.allows(colourReply)).toBe(false);
  });
});
