import { describe, expect, it } from "vitest";
import type { LineTokens, Tokenizer } from "../preview/highlight";
import { COLOUR_CAP, LONG_LINE, STEP_LINES, TokenStore } from "./tokens";

type Call = { first: string; count: number; state: unknown };

// One token per line. Its class is the count of lines seen so far in the state, so a wrong restart shows.
function fake() {
  const calls: Call[] = [];
  const tokenize: Tokenizer = (lines, state) => {
    const base = typeof state === "number" ? state : 0;
    calls.push({ first: lines[0] ?? "", count: lines.length, state });
    return { tokens: lines.map((_, i) => [{ from: 0, to: 1, cls: `n${base + i}` }]), state: base + lines.length };
  };
  return { calls, tokenize };
}

const text = (n: number): string[] => Array.from({ length: n }, (_, i) => `L${i}`);
const all = (s: TokenStore, n: number): Array<LineTokens | null> => Array.from({ length: n }, (_, i) => s.line(i));
const finish = (s: TokenStore) => {
  while (s.step());
};

describe("the constants", () => {
  it("are the numbers of the plan", () => {
    expect([STEP_LINES, COLOUR_CAP, LONG_LINE]).toEqual([100, 1024 * 1024, 2000]);
  });
});

describe("TokenStore", () => {
  it("tokens_a_step_colours_at_most_100_lines", () => {
    const f = fake();
    const s = new TokenStore(f.tokenize);
    s.reset(text(250));
    s.want(0, 40);
    expect(s.step()).toBe(true);
    expect(f.calls.reduce((n, c) => n + c.count, 0)).toBe(100);
    expect(s.line(99)).not.toBeNull();
    expect(s.line(100)).toBeNull();
  });

  it("tokens_the_wanted_lines_come_before_the_rest_once_the_lines_above_are_done", () => {
    const f = fake();
    const s = new TokenStore(f.tokenize);
    s.reset(text(1000));
    s.want(250, 260);
    s.step();
    s.step();
    expect(s.line(250)).toBeNull();
    s.step();
    for (let i = 250; i <= 260; i++) expect(s.line(i)).not.toBeNull();
    expect(s.line(300)).toBeNull();
    expect(s.line(0)).toEqual([{ from: 0, to: 1, cls: "n0" }]);
    expect(s.line(255)).toEqual([{ from: 0, to: 1, cls: "n255" }]);
  });

  it("tokens_an_edit_keeps_the_lines_above_and_stales_the_rest", () => {
    const s = new TokenStore(fake().tokenize);
    s.reset(text(250));
    finish(s);
    const above = all(s, 150);
    s.edit(150, 1, ["changed"]);
    expect(all(s, 150)).toEqual(above);
    expect(s.line(150)).toBeNull();
    expect(s.line(200)).toBeNull();
    expect(s.line(249)).toBeNull();
  });

  it("tokens_an_edit_restarts_from_the_boundary_before_it", () => {
    const f = fake();
    const s = new TokenStore(f.tokenize);
    s.reset(text(350));
    finish(s);
    f.calls.length = 0;
    const lines = text(350);
    lines[250] = "changed";
    s.edit(250, 1, ["changed"]);
    s.step();
    expect(f.calls[0]!.first).toBe("L200");
    expect(f.calls[0]!.state).toBe(200);
    expect(s.line(250)).toEqual([{ from: 0, to: 1, cls: "n250" }]);
    expect(s.line(251)).toEqual([{ from: 0, to: 1, cls: "n251" }]);
    f.calls.length = 0;
    s.edit(299, 1, ["again"]);
    s.step();
    expect(f.calls[0]!.first).toBe("L200");
    f.calls.length = 0;
    s.edit(300, 1, ["on the boundary"]);
    s.step();
    expect(f.calls[0]!.first).toBe("on the boundary");
  });

  it("tokens_an_edit_during_stepping_never_gives_tokens_of_the_old_text", () => {
    const f = fake();
    const s = new TokenStore(f.tokenize);
    s.reset(text(250));
    s.step();
    s.edit(3, 1, ["new"]);
    expect(s.line(3)).toBeNull();
    expect(s.line(2)).not.toBeNull();
    expect(s.line(50)).toBeNull();
    finish(s);
    const lines = text(250);
    lines[3] = "new";
    const fresh = new TokenStore(fake().tokenize);
    fresh.reset(lines);
    finish(fresh);
    expect(all(s, 250)).toEqual(all(fresh, 250));
  });

  it("tokens_reset_drops_everything", () => {
    const f = fake();
    const s = new TokenStore(f.tokenize);
    s.reset(text(250));
    finish(s);
    s.reset(["a", "b"]);
    expect(s.line(0)).toBeNull();
    expect(s.line(1)).toBeNull();
    f.calls.length = 0;
    finish(s);
    expect(f.calls).toEqual([{ first: "a", count: 2, state: undefined }]);
    expect(s.line(1)).toEqual([{ from: 0, to: 1, cls: "n1" }]);
    expect(s.line(2)).toBeNull();
  });

  it("tokens_inserting_and_removing_lines_shifts_nothing_above", () => {
    const s = new TokenStore(fake().tokenize);
    s.reset(text(250));
    finish(s);
    const above = all(s, 120);
    s.edit(120, 0, ["a", "b", "c"]);
    expect(all(s, 120)).toEqual(above);
    finish(s);
    const inserted = [...text(120), "a", "b", "c", ...text(250).slice(120)];
    const fresh = new TokenStore(fake().tokenize);
    fresh.reset(inserted);
    finish(fresh);
    expect(all(s, 253)).toEqual(all(fresh, 253));
    expect(s.line(253)).toBeNull();
    s.edit(120, 5, []);
    expect(all(s, 120)).toEqual(above);
    finish(s);
    const removed = [...text(120), ...inserted.slice(125)];
    fresh.reset(removed);
    finish(fresh);
    expect(all(s, 248)).toEqual(all(fresh, 248));
    expect(s.line(248)).toBeNull();
  });

  it("tokens_step_is_false_when_done", () => {
    const f = fake();
    const s = new TokenStore(f.tokenize);
    expect(s.step()).toBe(false);
    s.reset(text(150));
    expect(s.step()).toBe(true);
    expect(s.step()).toBe(true);
    const n = f.calls.length;
    expect(s.step()).toBe(false);
    expect(f.calls.length).toBe(n);
    s.edit(10, 1, ["x"]);
    expect(s.step()).toBe(true);
  });

  it("tokens_version_grows_only_when_a_line_changes", () => {
    const s = new TokenStore(fake().tokenize);
    const v0 = s.version;
    s.reset(text(150));
    const v1 = s.version;
    expect(v1).toBeGreaterThan(v0);
    s.want(0, 30);
    expect(s.version).toBe(v1);
    s.step();
    const v2 = s.version;
    expect(v2).toBeGreaterThan(v1);
    finish(s);
    const v3 = s.version;
    expect(v3).toBeGreaterThan(v2);
    expect(s.step()).toBe(false);
    expect(s.version).toBe(v3);
    s.edit(10, 1, ["x"]);
    const v4 = s.version;
    expect(v4).toBeGreaterThan(v3);
    s.step();
    expect(s.version).toBeGreaterThan(v4);
  });
});
