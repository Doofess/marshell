import { describe, expect, it } from "vitest";
import { COLOUR_CAP } from "../editor/tokens";
import type { Tokenizer } from "../preview/highlight";
import type { Row, Section } from "./model";
import { type ColourCache, colourSections } from "./useDiffColours";

/** Colours a leading `const` or `let` as a keyword and records its calls. */
function counting() {
  const calls = { n: 0, lines: [] as string[][] };
  const tokenize: Tokenizer = (lines, state) => {
    calls.n++;
    calls.lines.push(lines);
    return {
      tokens: lines.map((l) => {
        const m = /^(const|let)/.exec(l);
        return m ? [{ from: 0, to: m[1]!.length, cls: "tok-keyword" }] : [];
      }),
      state,
    };
  };
  return { calls, tokenize };
}
const section = (path: string, rows: Row[], folded = false): Section => ({ path, status: "modified", added: 1, removed: 1, rows, folded });
const rows: Row[] = [
  { kind: "hunk", text: "@@ -1 +1 @@" },
  { kind: "del", text: "const a", oldNo: 1 },
  { kind: "add", text: "let a", newNo: 1 },
  { kind: "context", text: "x", oldNo: 2, newNo: 2 },
];

describe("colourSections", () => {
  it("colours the new and old sides apart", () => {
    const { tokenize } = counting();
    const got = colourSections([section("a.ts", rows)], new Map(), () => tokenize).get("a.ts")!;
    expect(got[0]).toEqual([]);
    expect(got[1]).toEqual([{ from: 0, to: 5, cls: "tok-keyword" }]);
    expect(got[2]).toEqual([{ from: 0, to: 3, cls: "tok-keyword" }]);
  });
  it("feeds each side its own lines in order", () => {
    const { tokenize, calls } = counting();
    colourSections([section("a.ts", rows)], new Map(), () => tokenize);
    expect(calls.lines).toEqual([
      ["let a", "x"],
      ["const a", "x"],
    ]);
  });
  it("skips colours past COLOUR_CAP, for a file with no language and for a folded section", () => {
    const { tokenize } = counting();
    const big = [{ kind: "add" as const, text: "x".repeat(COLOUR_CAP + 1), newNo: 1 }];
    const lang = (l: string) => (l === "typescript" ? tokenize : undefined);
    const out = colourSections([section("big.ts", big), section("notes.txt", rows), section("f.ts", rows, true)], new Map(), lang);
    expect(out.size).toBe(0);
  });
  it("reuses a cached section until its rows change", () => {
    const { tokenize, calls } = counting();
    const cache: ColourCache = new Map();
    colourSections([section("a.ts", rows)], cache, () => tokenize);
    const first = calls.n;
    colourSections([section("a.ts", rows.map((r) => ({ ...r })))], cache, () => tokenize);
    expect(calls.n).toBe(first);
    colourSections([section("a.ts", [...rows, { kind: "add", text: "let z", newNo: 3 }])], cache, () => tokenize);
    expect(calls.n).toBeGreaterThan(first);
  });
  it("leaves a section out, uncached, until its tokenizer has loaded", () => {
    const cache: ColourCache = new Map();
    expect(colourSections([section("a.ts", rows)], cache, () => undefined).size).toBe(0);
    expect(cache.size).toBe(0);
  });
});
