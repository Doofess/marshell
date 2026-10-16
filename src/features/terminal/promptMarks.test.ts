import { describe, expect, it } from "vitest";
import { jumpFrom, jumpWords, parseOsc133, promptTarget, trackPrompts } from "./promptMarks";

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

/** A stand-in for xterm's decoration element: it starts with xterm's own class, which positions it. */
function fakeElement() {
  const classes = new Set(["xterm-decoration"]);
  const attrs = new Map<string, string>();
  return {
    classList: {
      add: (c: string) => void classes.add(c),
      remove: (c: string) => void classes.delete(c),
      toggle: (c: string, on: boolean) => void (on ? classes.add(c) : classes.delete(c)),
      contains: (c: string) => classes.has(c),
    },
    get className() {
      return [...classes].join(" ");
    },
    set className(v: string) {
      classes.clear();
      v.split(" ").filter(Boolean).forEach((c) => classes.add(c));
    },
    title: "",
    setAttribute: (k: string, v: string) => void attrs.set(k, v),
    getAttribute: (k: string) => attrs.get(k) ?? null,
  };
}

/** A stand-in terminal: the cursor line moves when a test says so, and each decoration renders once. */
function fakeTerm() {
  let handler: (data: string) => boolean = () => false;
  const t = {
    cursorLine: 0,
    decorations: [] as ReturnType<typeof fakeElement>[],
    send: (data: string) => handler(data),
    parser: { registerOscHandler: (_id: number, h: (data: string) => boolean) => ((handler = h), { dispose: () => {} }) },
    registerMarker: () => ({ line: t.cursorLine, onDispose: () => ({ dispose: () => {} }), dispose: () => {} }),
    registerDecoration: () => ({
      onRender: (cb: (el: HTMLElement) => void) => {
        const el = fakeElement();
        t.decorations.push(el);
        cb(el as unknown as HTMLElement);
      },
    }),
  };
  return t;
}

describe("trackPrompts", () => {
  it("marks a finished command beside its prompt and keeps xterm's positioning class", () => {
    const t = fakeTerm();
    const tracker = trackPrompts(t as never);
    t.send("A");
    t.send("C");
    t.send("D;1");
    t.cursorLine = 3;
    t.send("A");
    t.send("C");
    t.send("D;0");
    expect(t.decorations).toHaveLength(2);
    const [failed, ok] = t.decorations;
    expect(failed!.classList.contains("xterm-decoration")).toBe(true);
    expect(failed!.classList.contains("prompt-mark")).toBe(true);
    expect(failed!.classList.contains("prompt-mark--failed")).toBe(true);
    expect(failed!.getAttribute("aria-label")).toBe("Exit code 1");
    expect(ok!.classList.contains("xterm-decoration")).toBe(true);
    expect(ok!.classList.contains("prompt-mark--failed")).toBe(false);
    expect(ok!.getAttribute("aria-label")).toBe("Exit code 0");
    expect(tracker.prompts.map((p) => p.exit)).toEqual([1, 0]);
  });
  it("marks nothing for a D without a code or without a C", () => {
    const t = fakeTerm();
    trackPrompts(t as never);
    t.send("A");
    t.send("D;1");
    t.cursorLine = 2;
    t.send("A");
    t.send("C");
    t.send("D");
    expect(t.decorations).toHaveLength(0);
  });
  it("counts one prompt when the user's own prompt also sends A on the same line", () => {
    const t = fakeTerm();
    const tracker = trackPrompts(t as never);
    t.send("A");
    t.send("A");
    t.send("C");
    t.send("D;2");
    expect(tracker.prompts).toHaveLength(1);
    expect(t.decorations).toHaveLength(1);
  });
  it("forgets every prompt on a reset", () => {
    const t = fakeTerm();
    const tracker = trackPrompts(t as never);
    t.send("A");
    tracker.reset();
    expect(tracker.prompts).toHaveLength(0);
  });
});

describe("jumpWords", () => {
  it("says where the jump landed, with a failing exit only", () => {
    expect(jumpWords(-1, { index: 3, total: 7, exit: 1 })).toBe("Prompt 3 of 7, exit 1");
    expect(jumpWords(1, { index: 4, total: 7, exit: 0 })).toBe("Prompt 4 of 7");
    expect(jumpWords(1, { index: 4, total: 7, exit: null })).toBe("Prompt 4 of 7");
  });
  it("says there is no prompt that way", () => {
    expect(jumpWords(-1, null)).toBe("No earlier prompt");
    expect(jumpWords(1, null)).toBe("No later prompt");
  });
});

describe("jumpFrom", () => {
  it("counts from the prompt it landed on when the view stopped short of it", () => {
    // The landed prompt is on the last screen (line 95, baseY 90): the view sits at 90.
    expect(jumpFrom(95, 90, 90)).toBe(95);
  });
  it("counts from the top of the view once the user scrolled away", () => {
    expect(jumpFrom(95, 90, 40)).toBe(40);
    expect(jumpFrom(null, 90, 40)).toBe(40);
  });
});
