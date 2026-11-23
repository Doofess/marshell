import { describe, expect, it } from "vitest";
import type { ChangesReply } from "../../generated/ChangesReply";
import type { DiffFile } from "../../generated/DiffFile";
import type { DraftComment } from "../../generated/DraftComment";
import type { Placement } from "./comments";
import { FOLD_LINES, OPEN_BUDGET, cutFiles, focusIndex, isLockfile, isMinified, itemLabel, itemsOf, sectionsOf, type Item, type Row, type Section } from "./model";

const file = (path: string, extra: Partial<DiffFile> = {}): DiffFile => ({ path, old_path: null, status: "modified", note: null, ...extra });
const reply = (files: DiffFile[], diff: string, extra: Partial<ChangesReply> = {}): ChangesReply => ({
  kind: "listed",
  stamp: "s",
  folder: "C:\\repo",
  moved: false,
  branch: "main",
  detached: null,
  base: "origin/main",
  unrelated: false,
  ahead: 0,
  uncommitted: true,
  files,
  diff,
  truncated: false,
  more_new: 0, new_cut: false,
  reason: null,
  ...extra,
});
const head = (path: string, before = "") => `diff --git a/${path} b/${path}\n${before}index 1..2 100644\n--- a/${path}\n+++ b/${path}\n`;
/** A file whose diff is `n` added lines in one hunk: n + 1 rows. */
const adds = (path: string, n: number) => head(path) + `@@ -0,0 +1,${n} @@\n` + "+x\n".repeat(n);
const draft = (over: Partial<DraftComment> = {}): DraftComment => ({
  id: "d1",
  path: "src/a.ts",
  side: "new",
  start: 2,
  end: 2,
  rows: 1,
  first: "f",
  hash: "h",
  quote: ["x"],
  text: "note",
  ...over,
});
const A = head("src/a.ts") + "@@ -1,3 +1,3 @@\n one\n-two\n+TWO\n three\n";

describe("sectionsOf", () => {
  it("leaves out the files past a cut diff, and counts them", () => {
    const cut = reply([file("a.ts"), file("b.ts"), file("c.ts")], adds("a.ts", 1), { truncated: true });
    // With whitespace hidden too, a file past the cut must not read as "Only whitespace changed".
    expect(sectionsOf(cut, {}, [], true).map((x) => x.path)).toEqual(["a.ts"]);
    expect(cutFiles(cut)).toBe(2);
    const whole = reply([file("a.ts"), file("b.ts")], adds("a.ts", 1) + head("b.ts"));
    expect(cutFiles(whole)).toBe(0);
    expect(sectionsOf(whole, {}, [], true).find((x) => x.path === "b.ts")?.note).toBe("whitespace");
  });
  it("comes in the reply's file order, with counts", () => {
    const s = sectionsOf(reply([file("b.ts"), file("src/a.ts")], A + adds("b.ts", 2)), {}, [], false);
    expect(s.map((x) => x.path)).toEqual(["b.ts", "src/a.ts"]);
    expect(s[1]).toMatchObject({ status: "modified", added: 1, removed: 1, folded: false });
    expect(s[0]).toMatchObject({ added: 2, removed: 0 });
  });
  it("numbers the rows and keeps hunk headers as rows", () => {
    const [s] = sectionsOf(reply([file("src/a.ts")], A), {}, [], false);
    expect(s?.rows).toEqual([
      { kind: "hunk", text: "@@ -1,3 +1,3 @@" },
      { kind: "context", text: "one", oldNo: 1, newNo: 1 },
      { kind: "del", text: "two", oldNo: 2 },
      { kind: "add", text: "TWO", newNo: 2 },
      { kind: "context", text: "three", oldNo: 3, newNo: 3 },
    ]);
  });
  it("starts a new file's rows at its first hunk", () => {
    const d = "diff --git a/n.ts b/n.ts\nnew file mode 100644\nsimilarity index 90%\nrename from o.ts\nrename to n.ts\nold mode 100644\nnew mode 100755\nindex 0..1\n--- /dev/null\n+++ b/n.ts\n@@ -0,0 +1 @@\n+hi\n";
    const [s] = sectionsOf(reply([file("n.ts", { status: "added" })], d), {}, [], false);
    expect(s?.rows.map((r) => r.kind)).toEqual(["hunk", "add"]);
  });
  it("gives a section with only a binary line no rows", () => {
    const d = "diff --git a/p.png b/p.png\nindex 1..2 100644\nBinary files a/p.png and b/p.png differ\n";
    const [s] = sectionsOf(reply([file("p.png")], d), {}, [], false);
    expect(s?.rows).toEqual([]);
    expect(s?.note).toBeUndefined();
  });
  it("carries the core's binary and large notes", () => {
    const s = sectionsOf(
      reply([file("p.png", { note: "binary" }), file("big.log", { status: "untracked", note: "large" })], "diff --git a/p.png b/p.png\nBinary file p.png\ndiff --git a/big.log b/big.log\nLarge file big.log\n"),
      {},
      [],
      false,
    );
    expect(s.map((x) => x.note)).toEqual(["binary", "large"]);
    expect(s.map((x) => x.rows)).toEqual([[], []]);
  });
  it("skips a no-newline marker in the numbering", () => {
    const d = head("a.txt") + "@@ -1,2 +1,2 @@\n-a\n\\ No newline at end of file\n+a\n b\n";
    const [s] = sectionsOf(reply([file("a.txt")], d), {}, [], false);
    expect(s?.rows).toEqual([
      { kind: "hunk", text: "@@ -1,2 +1,2 @@" },
      { kind: "del", text: "a", oldNo: 1 },
      { kind: "add", text: "a", newNo: 1 },
      { kind: "context", text: "b", oldNo: 2, newNo: 2 },
    ]);
  });
  it("notes a file with no rows when whitespace is hidden, and not otherwise", () => {
    expect(sectionsOf(reply([file("w.ts")], ""), {}, [], true)[0]?.note).toBe("whitespace");
    expect(sectionsOf(reply([file("w.ts")], ""), {}, [], false)[0]?.note).toBeUndefined();
  });
  it("doesn't call a rename, a new empty file or a mode change whitespace, with whitespace hidden", () => {
    const renamed = "diff --git a/old.ts b/new.ts\nsimilarity index 100%\nrename from old.ts\nrename to new.ts\n";
    const empty = "diff --git a/e.ts b/e.ts\nnew file mode 100644\nindex 0000000..e69de29\n";
    const mode = "diff --git a/m.sh b/m.sh\nold mode 100644\nnew mode 100755\n";
    const r = reply([file("new.ts", { old_path: "old.ts", status: "renamed" }), file("e.ts", { status: "added" }), file("m.sh")], renamed + empty + mode);
    expect(sectionsOf(r, {}, [], true).map((x) => x.note)).toEqual([undefined, undefined, undefined]);
  });
  it("keeps a comment on a file past the cut under a note that says so, where it can be edited or deleted", () => {
    const cut = reply([file("a.ts"), file("b.ts"), file("c.ts", { status: "added" })], adds("a.ts", 1), { truncated: true });
    const s = sectionsOf(cut, {}, [draft({ id: "1", path: "c.ts" })], false);
    expect(s.map((x) => x.path)).toEqual(["a.ts", "c.ts"]);
    expect(s[1]).toMatchObject({ status: "added", note: "cut", rows: [], folded: false });
    // The open box's file too, like a comment's.
    expect(sectionsOf(cut, {}, [], false, "b.ts").map((x) => [x.path, x.note])).toEqual([["a.ts", undefined], ["b.ts", "cut"]]);
  });
  it("keeps a comment on a reverted file under a gone section, at the end in path order", () => {
    const s = sectionsOf(reply([file("src/a.ts")], A), {}, [draft({ id: "1", path: "z.ts" }), draft({ id: "2", path: "m.ts" }), draft({ id: "3", path: "z.ts" })], false);
    expect(s.map((x) => x.path)).toEqual(["src/a.ts", "m.ts", "z.ts"]);
    expect(s[1]).toMatchObject({ note: "gone", rows: [], added: 0, removed: 0, folded: false });
    expect(s.filter((x) => x.path === "src/a.ts")).toHaveLength(1);
  });
  it("keeps the old path and the status of a rename", () => {
    const d = "diff --git a/o.ts b/n.ts\nsimilarity index 90%\nrename from o.ts\nrename to n.ts\n--- a/o.ts\n+++ b/n.ts\n@@ -1 +1 @@\n-a\n+b\n";
    const [s] = sectionsOf(reply([file("n.ts", { status: "renamed", old_path: "o.ts" })], d), {}, [], false);
    expect(s).toMatchObject({ path: "n.ts", oldPath: "o.ts", status: "renamed", added: 1, removed: 1 });
  });
});

describe("folding", () => {
  it("folds a lockfile, a minified file and a 401-row file", () => {
    const d = adds("pnpm-lock.yaml", 3) + adds("dist/app.min.js", 3) + adds("big.ts", FOLD_LINES) + adds("ok.ts", FOLD_LINES - 1);
    const s = sectionsOf(reply([file("pnpm-lock.yaml"), file("dist/app.min.js"), file("big.ts"), file("ok.ts")], d), {}, [], false);
    expect(s.map((x) => [x.folded, x.foldReason])).toEqual([
      [true, "lockfile"],
      [true, "minified"],
      [true, "big"],
      [false, undefined],
    ]);
    expect(s[3]?.rows).toHaveLength(FOLD_LINES);
  });
  it("folds a file with one row over 1,000 characters as minified", () => {
    const d = head("bundle.js") + "@@ -0,0 +1 @@\n+" + "a".repeat(1001) + "\n";
    expect(sectionsOf(reply([file("bundle.js")], d), {}, [], false)[0]).toMatchObject({ folded: true, foldReason: "minified" });
  });
  it("folds what comes after 10,000 open rows", () => {
    const names = Array.from({ length: 27 }, (_, i) => `f${i}.ts`);
    const s = sectionsOf(reply(names.map((n) => file(n)), names.map((n) => adds(n, FOLD_LINES - 1)).join("")), {}, [], false);
    // Each file is 400 rows: 25 files make exactly 10,000.
    expect(OPEN_BUDGET).toBe(10_000);
    expect(s.slice(0, 25).every((x) => !x.folded)).toBe(true);
    expect(s.slice(25).map((x) => [x.folded, x.foldReason])).toEqual([
      [true, "budget"],
      [true, "budget"],
    ]);
  });
  it("does not count a folded file's rows against the budget", () => {
    const d = adds("a.lock", 3) + adds("Cargo.lock", 10_000) + adds("x.ts", 5);
    const s = sectionsOf(reply([file("a.lock"), file("Cargo.lock"), file("x.ts")], d), {}, [], false);
    expect(s.map((x) => x.folded)).toEqual([false, true, false]);
  });
  it("lets a fold override beat the rule", () => {
    const d = adds("Cargo.lock", 3) + adds("a.ts", 3);
    const s = sectionsOf(reply([file("Cargo.lock"), file("a.ts")], d), { "Cargo.lock": false, "a.ts": true }, [], false);
    expect(s.map((x) => x.folded)).toEqual([false, true]);
  });
  it("counts an override-opened file's rows against the budget", () => {
    const d = adds("Cargo.lock", 10_000) + adds("a.ts", 3);
    const s = sectionsOf(reply([file("Cargo.lock"), file("a.ts")], d), { "Cargo.lock": false }, [], false);
    expect(s[0]?.folded).toBe(false);
    expect(s[1]).toMatchObject({ folded: true, foldReason: "budget" });
  });
  it("never folds a section with no rows", () => {
    const s = sectionsOf(reply([file("w.ts")], ""), { "w.ts": true }, [draft({ path: "gone.ts" })], true);
    expect(s.every((x) => !x.folded)).toBe(true);
  });
});

describe("isLockfile and isMinified", () => {
  it("knows the lockfile names, in any folder", () => {
    for (const n of ["pnpm-lock.yaml", "package-lock.json", "npm-shrinkwrap.json", "yarn.lock", "bun.lock", "bun.lockb", "Cargo.lock", "Gemfile.lock", "composer.lock", "poetry.lock", "uv.lock", "Pipfile.lock", "go.sum", "flake.lock", "packages.lock.json", "pubspec.lock", "mix.lock", "Podfile.lock"]) {
      expect(isLockfile(n)).toBe(true);
      expect(isLockfile(`deep/dir/${n}`)).toBe(true);
    }
    expect(isLockfile("lock.ts")).toBe(false);
    expect(isLockfile("my-Cargo.lock")).toBe(false);
    expect(isLockfile("src/yarn.lock.md")).toBe(false);
  });
  it("knows a minified name, in any case", () => {
    const rows: Row[] = [];
    for (const n of ["a.min.js", "a.min.mjs", "a.min.cjs", "a.min.css", "A.MIN.JS"]) expect(isMinified(n, rows)).toBe(true);
    expect(isMinified("a.min.ts", rows)).toBe(false);
    expect(isMinified("admin.js", rows)).toBe(false);
  });
  it("knows a minified body by a row over 1,000 characters", () => {
    expect(isMinified("a.js", [{ kind: "add", text: "a".repeat(1000) }])).toBe(false);
    expect(isMinified("a.js", [{ kind: "add", text: "a".repeat(1001) }])).toBe(true);
  });
});

describe("itemsOf", () => {
  const sections = () => sectionsOf(reply([file("src/a.ts"), file("Cargo.lock"), file("p.png", { note: "binary" })], A + adds("Cargo.lock", 3) + "diff --git a/p.png b/p.png\nBinary file p.png\n"), {}, [], false);
  const kinds = (items: Item[]) => items.map((i) => i.kind);

  it("puts each file's header first, then its rows", () => {
    const items = itemsOf(sections().slice(0, 1), [], new Map());
    expect(kinds(items)).toEqual(["file", "row", "row", "row", "row", "row"]);
    expect(items.filter((i) => i.kind === "row").map((i) => (i.kind === "row" ? i.at : -1))).toEqual([0, 1, 2, 3, 4]);
  });
  it("gives a folded file a show item and no rows, and a note its own item", () => {
    const items = itemsOf(sections().slice(1), [], new Map());
    expect(kinds(items)).toEqual(["file", "show", "file", "note"]);
    expect(items[3]).toMatchObject({ kind: "note", text: "Binary file, not shown." });
  });
  it("puts a placed comment right after its row, and an unplaced one after the header", () => {
    const s = sectionsOf(reply([file("src/a.ts")], A), {}, [], false);
    const d1 = draft({ id: "c1" });
    const d2 = draft({ id: "c2", start: 1, end: 1 });
    const placements = new Map<string, Placement>([
      ["c1", { at: 3, changed: false }],
      ["c2", { at: null, changed: true }],
    ]);
    const items = itemsOf(s, [d1, d2], placements);
    expect(items.map((i) => (i.kind === "comment" ? `comment:${i.comment.id}` : i.kind === "row" ? `row:${i.at}` : i.kind))).toEqual([
      "file",
      "comment:c2",
      "row:0",
      "row:1",
      "row:2",
      "row:3",
      "comment:c1",
      "row:4",
    ]);
  });
  it("keeps a gone section's comment under a note", () => {
    const s = sectionsOf(reply([], ""), {}, [draft({ id: "c1", path: "gone.ts" })], false);
    const items = itemsOf(s, [draft({ id: "c1", path: "gone.ts" })], new Map([["c1", { at: null, changed: true }]]));
    expect(kinds(items)).toEqual(["file", "note", "comment"]);
    expect(items[1]).toMatchObject({ text: "No changes in this file now." });
  });
  it("gives every item a distinct key", () => {
    const s = sections();
    const items = itemsOf(s, [draft({ id: "c1" })], new Map([["c1", { at: 2, changed: false }]]));
    expect(new Set(items.map((i) => i.key)).size).toBe(items.length);
  });
  it("keeps a row's key when rows go in above it, so the keyboard stays on the same line", () => {
    const before = itemsOf(sectionsOf(reply([file("src/a.ts")], A), {}, [], false), [], new Map());
    const moved = head("src/a.ts") + "@@ -1,3 +1,8 @@\n+p\n+p\n+p\n+p\n+p\n one\n-two\n+TWO\n three\n";
    const after = itemsOf(sectionsOf(reply([file("src/a.ts")], moved), {}, [], false), [], new Map());
    const keyOf = (items: Item[], text: string) => items.find((i) => i.kind === "row" && i.file.rows[i.at]!.text === text)?.key;
    expect(keyOf(after, "TWO")).toBeDefined();
    expect(keyOf(after, "TWO")).toBe(keyOf(before, "TWO"));
    expect(keyOf(after, "three")).toBe(keyOf(before, "three"));
    // Five rows with the same text still get five keys.
    expect(new Set(after.map((i) => i.key)).size).toBe(after.length);
  });
});

describe("focusIndex", () => {
  const items = itemsOf(sectionsOf(reply([file("src/a.ts")], A), {}, [], false), [], new Map());

  it("finds the focused item by its key", () => {
    expect(focusIndex(items, items[3]!.key, 0)).toBe(3);
  });
  it("stays at the index it was on when its item is gone, within the list", () => {
    expect(focusIndex(items, "row:gone", 2)).toBe(2);
    expect(focusIndex(items, "row:gone", 99)).toBe(items.length - 1);
    expect(focusIndex(items, null, 0)).toBe(0);
    expect(focusIndex([], "row:gone", 2)).toBe(0);
  });
});

describe("itemLabel", () => {
  const s: Section = { path: "src/a.ts", status: "modified", added: 2, removed: 1, rows: [], folded: false };
  const row = (r: Row, at = 0): Item => ({ kind: "row", key: "k", file: { ...s, rows: [r] }, at });
  it("labels rows for a screen reader", () => {
    expect(itemLabel(row({ kind: "add", text: "const x = 1;", newNo: 42 }))).toBe("Added, line 42: const x = 1;");
    expect(itemLabel(row({ kind: "del", text: "old();", oldNo: 7 }))).toBe("Removed, line 7: old();");
    expect(itemLabel(row({ kind: "context", text: "keep", oldNo: 11, newNo: 12 }))).toBe("Line 12: keep");
  });
  it("reads an empty line as blank line", () => {
    expect(itemLabel(row({ kind: "add", text: "", newNo: 3 }))).toBe("Added, line 3: blank line");
  });
  it("labels a hunk by its new-file line", () => {
    expect(itemLabel(row({ kind: "hunk", text: "@@ -8,3 +10,4 @@ fn x()" }))).toBe("Changed block at line 10");
  });
  it("labels file headers", () => {
    const h = (f: Partial<Section>): Item => ({ kind: "file", key: "k", file: { ...s, ...f } });
    expect(itemLabel(h({}))).toBe("src/a.ts, modified, 2 added, 1 removed");
    expect(itemLabel(h({ status: "added" }))).toBe("src/a.ts, new, 2 added, 1 removed");
    expect(itemLabel(h({ status: "untracked" }))).toBe("src/a.ts, new, 2 added, 1 removed");
    expect(itemLabel(h({ status: "deleted" }))).toBe("src/a.ts, deleted, 2 added, 1 removed");
    expect(itemLabel(h({ status: "renamed", oldPath: "src/o.ts" }))).toBe("src/a.ts, renamed from src/o.ts, 2 added, 1 removed");
  });
  it("labels a show item with the row count", () => {
    const rows: Row[] = Array.from({ length: 1234 }, () => ({ kind: "add", text: "x", newNo: 1 }));
    expect(itemLabel({ kind: "show", key: "k", file: { ...s, rows } })).toBe("Show 1,234 lines");
    expect(itemLabel({ kind: "show", key: "k", file: { ...s, rows: rows.slice(0, 1) } })).toBe("Show 1 line");
  });
  it("labels a comment, with the changed sentence", () => {
    const c = (over: Partial<DraftComment>, changed: boolean): Item => ({ kind: "comment", key: "k", comment: draft(over), placed: { at: 1, changed } });
    expect(itemLabel(c({ start: 42, end: 42, text: "Use PORT." }, false))).toBe("Comment on line 42: Use PORT.");
    expect(itemLabel(c({ start: 10, end: 14, text: "Why?" }, false))).toBe("Comment on lines 10 to 14: Why?");
    expect(itemLabel(c({ start: 42, end: 42, text: "Use PORT." }, true))).toBe("Comment on line 42: Use PORT.. The line changed");
    expect(itemLabel(c({ start: 10, end: 14, text: "Why?" }, true))).toBe("Comment on lines 10 to 14: Why?. The lines changed");
  });
  it("labels a moved comment by the lines it is on now", () => {
    const item: Item = { kind: "comment", key: "k", comment: draft({ start: 3, end: 4, text: "Why?" }), placed: { at: 10, changed: false, start: 8, end: 9 } };
    expect(itemLabel(item)).toBe("Comment on lines 8 to 9: Why?");
  });
  it("labels a note by its text", () => {
    expect(itemLabel({ kind: "note", key: "k", file: s, text: "Only whitespace changed." })).toBe("Only whitespace changed.");
  });
});
