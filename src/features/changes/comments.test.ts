import { describe, expect, it } from "vitest";
import type { ChangesReply } from "../../generated/ChangesReply";
import type { DraftComment } from "../../generated/DraftComment";
import { MAX_COMMENTS, MAX_COMMENT_CHARS, QUOTE_CHARS, QUOTE_LINES, anchorOf, citation, cleanComment, cyrb53, messageOf, place, quoteLine, type Placement } from "./comments";
import { sectionsOf, type Section } from "./model";

const head = (path: string) => `diff --git a/${path} b/${path}\nindex 1..2 100644\n--- a/${path}\n+++ b/${path}\n`;
/** One file's section from a hunk body. Row 0 is the first hunk header. */
const sec = (body: string, path = "src/a.ts"): Section => {
  const reply = { kind: "listed", stamp: "s", folder: "r", moved: false, branch: "main", detached: null, base: "origin/main", unrelated: false, ahead: 0, uncommitted: true, files: [{ path, old_path: null, status: "modified", note: null }], diff: head(path) + body, truncated: false, more_new: 0, new_cut: false, reason: null } as ChangesReply;
  return sectionsOf(reply, {}, [], false)[0]!;
};
// idx: 0 hunk, 1 one(1/1), 2 two(2/2), 3 -old()(old 3), 4 +neu()(new 3), 5 keep()(4/4), 6 tail(5/5)
const A = "@@ -1,5 +1,5 @@\n one\n two\n-old()\n+neu()\n keep()\n tail\n";
const full = (c: Omit<DraftComment, "id" | "text">, over: Partial<DraftComment> = {}): DraftComment => ({ id: "c", text: "note", ...c, ...over });

describe("constants", () => {
  it("holds the limits", () => {
    expect([MAX_COMMENTS, MAX_COMMENT_CHARS, QUOTE_LINES, QUOTE_CHARS]).toEqual([50, 2000, 3, 200]);
  });
});

describe("cleanComment", () => {
  it("cleans a comment of control characters but keeps tabs and line breaks", () => {
    expect(cleanComment("a\tb\r\nc\u001b[201~d")).toBe("a\tb\nc[201~d");
  });
  it("turns a lone CR into a line break and drops DEL, form feed, NUL and C1 controls", () => {
    expect(cleanComment("a\rb\u007fc\fd\u0000e\u009bf")).toBe("a\nbcdef");
  });
  it("cuts at 2,000 code points, never inside a surrogate pair", () => {
    const out = cleanComment("x".repeat(MAX_COMMENT_CHARS - 1) + "😀😀");
    expect([...out]).toHaveLength(MAX_COMMENT_CHARS);
    expect(out.endsWith("😀")).toBe(true);
  });
  it("counts the cut after cleaning", () => {
    expect([...cleanComment("\u001b".repeat(50) + "y".repeat(MAX_COMMENT_CHARS))]).toHaveLength(MAX_COMMENT_CHARS);
  });
});

describe("quoteLine", () => {
  it("turns control characters in quoted code into U+FFFD", () => {
    expect(quoteLine("a\fb\u001b[0mc\rd")).toBe("a�b�[0mc�d");
  });
  it("keeps tabs and turns DEL and C1 controls into U+FFFD", () => {
    expect(quoteLine("\ta\u007f\u0085")).toBe("\ta��");
  });
  it("cuts a quote line by code points, never inside an emoji", () => {
    expect([...quoteLine("x".repeat(199) + "😀😀")].length).toBe(200);
    expect(quoteLine("x".repeat(199) + "😀😀").endsWith("😀")).toBe(true);
  });
});

describe("cyrb53", () => {
  it("gives 14 lowercase hex characters, zero-padded, the same every time", () => {
    for (const s of ["", "a", "context\u0000keep()", "😀"]) {
      expect(cyrb53(s)).toMatch(/^[0-9a-f]{14}$/);
      expect(cyrb53(s)).toBe(cyrb53(s));
    }
    expect(cyrb53("a")).not.toBe(cyrb53("b"));
  });
  it("is the well-known 53-bit hash", () => {
    expect(cyrb53("a")).toBe((7929297801672961).toString(16).padStart(14, "0"));
    expect(cyrb53("b")).toBe((8684336938537663).toString(16).padStart(14, "0"));
  });
});

describe("anchorOf", () => {
  it("quotes a mixed range with signs, and cites the new numbers", () => {
    const a = anchorOf(sec(A), 3, 5);
    expect(a.quote).toEqual(["-old()", "+neu()", " keep()"]);
    expect(a).toMatchObject({ path: "src/a.ts", side: "new", start: 3, end: 4, rows: 3 });
    expect(a.first).toBe(cyrb53("del\u0000old()"));
    expect(a.hash).toBe(cyrb53(["del\u0000old()", "add\u0000neu()", "context\u0000keep()"].join("\n")));
  });
  it("takes the side with numbers: a removed-only range is on the old side", () => {
    const s = sec("@@ -7,3 +7,0 @@\n-a\n-b\n-c\n");
    expect(anchorOf(s, 1, 3)).toMatchObject({ side: "old", start: 7, end: 9, rows: 3, quote: ["a", "b", "c"] });
  });
  it("quotes a single-kind range plainly", () => {
    const s = sec("@@ -0,0 +1,2 @@\n+p\n+q\n");
    expect(anchorOf(s, 1, 2).quote).toEqual(["p", "q"]);
  });
  it("takes the range whichever way it was dragged", () => {
    expect(anchorOf(sec(A), 5, 3)).toEqual(anchorOf(sec(A), 3, 5));
  });
  it("quotes only the first three rows and keeps the row count", () => {
    const a = anchorOf(sec("@@ -0,0 +1,5 @@\n+a\n+b\n+c\n+d\n+e\n"), 1, 5);
    expect(a.quote).toEqual(["a", "b", "c"]);
    expect(a.rows).toBe(5);
  });
  it("never lets a quote line hold CR, LF or ESC", () => {
    const a = anchorOf(sec("@@ -0,0 +1,1 @@\n+a\u001b[2Jb\n"), 1, 1);
    expect(a.quote).toEqual(["a�[2Jb"]);
    expect(a.quote.join("")).not.toMatch(/[\r\n\u001b]/);
  });
  it("cuts a quote line at 200 code points, sign included", () => {
    const a = anchorOf(sec("@@ -1,1 +1,1 @@\n-" + "y".repeat(300) + "\n+z\n"), 1, 2);
    expect([...a.quote[0]!]).toHaveLength(QUOTE_CHARS);
    expect(a.quote[0]!.startsWith("-y")).toBe(true);
  });
});

describe("place", () => {
  const mk = (s: Section, a: number, b: number) => full(anchorOf(s, a, b));

  it("keeps a comment on an unchanged file where it is", () => {
    const s = sec(A);
    expect(place(mk(s, 3, 5), s)).toEqual({ at: 5, changed: false, start: 3, end: 4 });
  });
  it("re-anchors a comment when lines above it move", () => {
    const c = mk(sec(A), 3, 5);
    const moved = sec("@@ -1,10 +1,10 @@\n p1\n p2\n p3\n p4\n p5\n one\n two\n-old()\n+neu()\n keep()\n tail\n");
    // Five rows went in above, so the old row 5 is now row 10, and its lines 3-4 are now 8-9.
    expect(place(c, moved)).toEqual({ at: 10, changed: false, start: 8, end: 9 });
  });
  it("marks a comment changed when its line's text changed", () => {
    const c = mk(sec(A), 3, 5);
    const edited = sec("@@ -1,5 +1,5 @@\n one\n two\n-old()\n+newer()\n keep()\n tail\n");
    // The first row is still found but the window differs: the row numbered 4 on the new side is the closest.
    expect(place(c, edited)).toEqual({ at: 5, changed: true });
  });
  it("marks a comment changed when its first row is gone, and falls on the nearest smaller line", () => {
    const c = mk(sec(A), 5, 5); // keep(), new 4
    const rewritten = sec("@@ -1,3 +1,3 @@\n one\n two\n-keep()\n+kept()\n");
    // new numbers: one 1, two 2, kept 3; 4 is past the end, so the nearest smaller is 3.
    expect(place(c, rewritten)).toEqual({ at: 4, changed: true });
  });
  it("places on the old side's numbers for a removed-only comment", () => {
    const s = sec("@@ -7,3 +7,0 @@\n-a\n-b\n-c\n");
    const c = mk(s, 1, 3);
    expect(place(c, s)).toEqual({ at: 3, changed: false, start: 7, end: 9 });
    const other = sec("@@ -7,3 +7,1 @@\n-a\n-b\n-X\n+Y\n");
    expect(place(c, other)).toEqual({ at: 3, changed: true });
  });
  it("picks the identical window nearest the comment's start", () => {
    const s = sec("@@ -1,3 +1,3 @@\n a\n dup\n b\n@@ -19,3 +19,3 @@\n x\n dup\n y\n");
    // idx: 0 hunk, 1 a, 2 dup (2), 3 b, 4 hunk, 5 x, 6 dup (20), 7 y
    const c = mk(s, 6, 6);
    expect(place({ ...c, start: 19, end: 19 }, s)).toEqual({ at: 6, changed: false, start: 20, end: 20 });
    expect(place({ ...c, start: 3, end: 3 }, s)).toEqual({ at: 2, changed: false, start: 2, end: 2 });
  });
  it("never matches a window that holds a hunk row", () => {
    const s = sec("@@ -1,2 +1,2 @@\n a\n b\n@@ -19,2 +19,2 @@\n x\n y\n");
    const hunk = "hunk\u0000@@ -19,2 +19,2 @@";
    const c = full({ path: "src/a.ts", side: "new", start: 2, end: 19, rows: 2, first: cyrb53("context\u0000b"), hash: cyrb53(["context\u0000b", hunk].join("\n")), quote: ["b", "x"] });
    expect(place(c, s).changed).toBe(true);
  });
  it("puts a comment of a reverted file at the top, changed", () => {
    expect(place(mk(sec(A), 3, 5), undefined)).toEqual({ at: null, changed: true });
    expect(place(mk(sec(A), 3, 5), sec("", "src/a.ts"))).toEqual({ at: null, changed: true });
  });
  it("puts a comment above every remaining row when all of them are numbered past its end", () => {
    const c = mk(sec(A), 3, 5);
    expect(place(c, sec("@@ -90,1 +90,1 @@\n q\n"))).toEqual({ at: null, changed: true });
  });
});

describe("citation", () => {
  it("cites a line or a range by its new numbers", () => {
    const s = sec(A);
    expect(citation(full(anchorOf(s, 4, 4), { path: "src/a.ts" }), false)).toBe("src/a.ts:3");
    expect(citation(full(anchorOf(s, 3, 6)), false)).toBe("src/a.ts:3-5");
  });
  it("cites a removed-only range by its old numbers", () => {
    const anchor = full(anchorOf(sec("@@ -7,3 +7,0 @@\n-a\n-b\n-c\n", "src/old.ts"), 1, 3));
    expect(citation(anchor, false)).toBe("src/old.ts:7-9 (removed lines)");
  });
  it("names a single removed line", () => {
    expect(citation(full(anchorOf(sec("@@ -7,1 +6,0 @@\n-a\n", "src/old.ts"), 1, 1)), false)).toBe("src/old.ts:7 (removed line)");
  });
  it("adds (changed since) to a changed comment", () => {
    const s = sec(A);
    expect(citation(full(anchorOf(s, 4, 4)), true)).toBe("src/a.ts:3 (changed since)");
    expect(citation(full(anchorOf(sec("@@ -7,1 +6,0 @@\n-a\n", "src/old.ts"), 1, 1)), true)).toBe("src/old.ts:7 (removed line) (changed since)");
  });
});

describe("messageOf", () => {
  const c = (over: Partial<DraftComment>): DraftComment => ({ id: "x", path: "src/a.ts", side: "new", start: 1, end: 1, rows: 1, first: "f", hash: "h", quote: ["q"], text: "t", ...over });
  const stub = (path: string): Section => ({ path, status: "modified", added: 0, removed: 0, rows: [], folded: false });
  const none = new Map<string, Placement>();

  it("writes the message in the exact format", () => {
    const drafts = [
      c({ id: "2", path: "src/old.ts", side: "old", start: 7, end: 7, quote: ["legacyInit();"], text: "Still needed?" }),
      c({ id: "1", path: "src/a.ts", start: 42, end: 42, quote: ["const port = 3000;"], text: "Use PORT." }),
    ];
    expect(messageOf(drafts, none, [stub("src/a.ts"), stub("src/old.ts")], "origin/main")).toBe(
      "Comments on the changes (compared with origin/main):\n\nsrc/a.ts:42\n> const port = 3000;\nUse PORT.\n\nsrc/old.ts:7 (removed line)\n> legacyInit();\nStill needed?",
    );
  });
  it("names uncommitted changes when there is no base", () => {
    expect(messageOf([c({})], none, [stub("src/a.ts")], null).split("\n")[0]).toBe("Comments on the uncommitted changes:");
  });
  it("ends a quote of more than three rows with an ellipsis line", () => {
    const m = messageOf([c({ rows: 5, start: 1, end: 5, quote: ["a", "b", "c"], text: "Why?" })], none, [stub("src/a.ts")], "main");
    expect(m).toContain("> c\n> …\nWhy?");
    expect(m).toContain("…");
    expect(messageOf([c({ rows: 3, start: 1, end: 3, quote: ["a", "b", "c"] })], none, [stub("src/a.ts")], "main")).not.toContain("> …");
  });
  it("orders by section, then by start", () => {
    const drafts = [c({ id: "1", path: "b.ts", start: 1, text: "B1" }), c({ id: "2", path: "a.ts", start: 9, end: 9, text: "A9" }), c({ id: "3", path: "a.ts", start: 2, end: 2, text: "A2" })];
    const m = messageOf(drafts, none, [stub("a.ts"), stub("b.ts")], "main");
    expect(m.indexOf("A2")).toBeLessThan(m.indexOf("A9"));
    expect(m.indexOf("A9")).toBeLessThan(m.indexOf("B1"));
  });
  it("keeps each file's comments together when the files aren't in the diff (a send from another checkout)", () => {
    const drafts = [c({ id: "1", path: "b.ts", start: 1, text: "B1" }), c({ id: "2", path: "a.ts", start: 9, end: 9, text: "A9" }), c({ id: "3", path: "b.ts", start: 20, end: 20, text: "B20" })];
    const m = messageOf(drafts, none, [], "main", "r");
    expect(m.indexOf("A9")).toBeLessThan(m.indexOf("B1"));
    expect(m.indexOf("B1")).toBeLessThan(m.indexOf("B20"));
  });
  it("adds (changed since) from the placements, and keeps the text as typed", () => {
    const placements = new Map<string, Placement>([["1", { at: 3, changed: true }]]);
    const m = messageOf([c({ id: "1", text: "  two\n\tlines " })], placements, [stub("src/a.ts")], "main");
    expect(m).toBe("Comments on the changes (compared with main):\n\nsrc/a.ts:1 (changed since)\n> q\n  two\n\tlines ");
  });
  it("cites the lines a moved comment's rows are on now", () => {
    const placements = new Map<string, Placement>([["1", { at: 10, changed: false, start: 8, end: 9 }]]);
    expect(messageOf([c({ id: "1", start: 3, end: 4, text: "Why?" })], placements, [stub("src/a.ts")], "main")).toBe("Comments on the changes (compared with main):\n\nsrc/a.ts:8-9\n> q\nWhy?");
  });
  it("has no trailing newline", () => {
    expect(messageOf([c({ text: "end\n" })], none, [stub("src/a.ts")], "main").endsWith("\n")).toBe(false);
  });
  it("puts a gone file's comment last, after the files that are there", () => {
    const m = messageOf([c({ id: "1", path: "gone.ts", text: "G" }), c({ id: "2", path: "a.ts", text: "A" })], none, [stub("a.ts"), stub("gone.ts")], "main");
    expect(m.indexOf("A")).toBeLessThan(m.indexOf("gone.ts:1"));
  });
});
