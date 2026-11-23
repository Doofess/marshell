import { describe, expect, it } from "vitest";
import type { HunkRow } from "../../generated/HunkRow";
import type { PullNote } from "../../generated/PullNote";
import type { ReviewThread } from "../../generated/ReviewThread";
import type { ThreadComment } from "../../generated/ThreadComment";
import { anchorOf } from "./comments";
import type { Row, Section } from "./model";
import { authorOf, commentCount, noteLabel, placeThread, shownThreads, threadAnchor, threadLabel, toResolve, type SendState } from "./threads";

const comment = (over: Partial<ThreadComment> = {}): ThreadComment => ({ id: 1, author: "alice", bot: false, body: "Please fix.", hidden_removed: false, cut: false, created_ms: null, url: null, ...over });
const thread = (over: Partial<ReviewThread> = {}): ReviewThread => ({
  id: "PRRT_one",
  path: "a.txt",
  side: "new",
  file: false,
  start: 2,
  end: 2,
  original_start: 2,
  original_end: 2,
  resolved: false,
  outdated: false,
  can_resolve: true,
  can_unresolve: true,
  can_reply: true,
  rows: [{ kind: "add", text: "two" }],
  comments: [comment()],
  more_comments: 0,
  minimized: 0,
  ...over,
});
const hunk: Row = { kind: "hunk", text: "@@ -0,0 +1,2 @@" };
const add = (text: string, newNo: number): Row => ({ kind: "add", text, newNo });
const del = (text: string, oldNo: number): Row => ({ kind: "del", text, oldNo });
const section = (path: string, rows: Row[], extra: Partial<Section> = {}): Section => ({ path, status: "modified", added: 0, removed: 0, rows, folded: false, ...extra });
const plain: SendState = { picked: false, note: "", sent: false };

describe("placeThread", () => {
  it("a thread sits on the row its hunk ends at", () => {
    expect(placeThread(thread(), section("a.txt", [hunk, add("one", 1), add("two", 2)]))).toEqual({ at: 2, changed: false, start: 2, end: 2 });
  });
  it("a thread follows its rows when lines above moved them", () => {
    const moved = section("a.txt", [hunk, add("one", 8), add("two", 9)]);
    expect(placeThread(thread(), moved)).toEqual({ at: 2, changed: false, start: 9, end: 9 });
  });
  it("a thread whose rows changed stays at its line, marked", () => {
    const p = placeThread(thread(), section("a.txt", [hunk, add("one", 1), add("TWO", 2)]));
    expect(p).toEqual({ at: 2, changed: true });
  });
  it("a thread on a removed line is placed on the old side", () => {
    const t = thread({ side: "old", start: 3, end: 3, original_start: 3, original_end: 3, rows: [{ kind: "del", text: "three" }] });
    const p = placeThread(t, section("a.txt", [hunk, add("one", 1), del("three", 3)]));
    expect(p).toEqual({ at: 2, changed: false, start: 3, end: 3 });
  });
  it("an outdated thread is placed by its original lines", () => {
    const t = thread({ start: null, end: null, original_start: 1, original_end: 2, outdated: true, rows: [{ kind: "add", text: "one" }, { kind: "add", text: "two" }] });
    expect(placeThread(t, section("a.txt", [hunk, add("one", 1), add("two", 2)]))).toEqual({ at: 2, changed: false, start: 1, end: 2 });
  });
  it("a file comment sits above the first row", () => {
    const t = thread({ file: true, rows: [], start: null, end: null, original_start: null, original_end: null });
    expect(placeThread(t, section("a.txt", [hunk, add("one", 1)]))).toEqual({ at: null, changed: false });
  });
  it("a thread finds its rows in a checkout whose lines hold what the core took out of them", () => {
    // The core sends a thread's rows without control and hidden characters; the checkout's own diff keeps them.
    const crlf = section("a.txt", [hunk, add("one\r", 1), add("two\r", 2)]);
    expect(placeThread(thread(), crlf)).toEqual({ at: 2, changed: false, start: 2, end: 2 });
    const marked = section("a.txt", [hunk, add("\u{feff}one", 1), add("t\u{200b}w\fo\u{e0041}", 2)]);
    expect(placeThread(thread(), marked)).toEqual({ at: 2, changed: false, start: 2, end: 2 });
    const joined = thread({ rows: [{ kind: "add", text: "\u{1f937}\u{200d}\u{2642}\u{fe0f}" }] });
    expect(placeThread(joined, section("a.txt", [hunk, add("one", 1), add("\u{1f937}\u{200d}\u{2642}\u{fe0f}\r", 2)]))).toEqual({ at: 2, changed: false, start: 2, end: 2 });
  });
  it("a tab and a space still tell two rows apart, and the quote keeps the row as it was sent", () => {
    expect(placeThread(thread(), section("a.txt", [hunk, add("one", 1), add("t wo", 2)]))).toEqual({ at: 2, changed: true });
    expect(placeThread(thread(), section("a.txt", [hunk, add("one", 1), add("t\two", 2)]))).toEqual({ at: 2, changed: true });
    const joined = thread({ rows: [{ kind: "add", text: "\u{1f937}\u{200d}\u{2642}\u{fe0f}" }] });
    expect(threadAnchor(joined)?.quote).toEqual(["\u{1f937}\u{200d}\u{2642}\u{fe0f}"]);
  });
  it("a thread whose file is not in the diff has no placement", () => {
    expect(placeThread(thread(), undefined)).toBeNull();
    expect(placeThread(thread(), section("a.txt", [], { note: "cut" }))).toBeNull();
  });
});

describe("which threads show", () => {
  const open = thread({ id: "open" });
  const resolved = thread({ id: "resolved", resolved: true });
  const outdated = thread({ id: "outdated", outdated: true });
  it("toResolve leaves out resolved and outdated threads", () => {
    expect(toResolve([open, resolved, outdated]).map((t) => t.id)).toEqual(["open"]);
  });
  it("shownThreads lists them all when asked", () => {
    expect(shownThreads([open, resolved, outdated], true).map((t) => t.id)).toEqual(["open", "resolved", "outdated"]);
    expect(shownThreads([open, resolved, outdated], false).map((t) => t.id)).toEqual(["open"]);
  });
  it("counts the comments GitHub holds", () => {
    expect(commentCount(thread({ comments: [comment(), comment()], more_comments: 3 }))).toBe(5);
  });
});

describe("threadAnchor", () => {
  it("hashes rows as a draft's anchor does", () => {
    const s = section("a.txt", [hunk, { kind: "context", text: "one", oldNo: 1, newNo: 1 }, del("two", 2), add("TWO", 2)]);
    const rows: HunkRow[] = [{ kind: "context", text: "one" }, { kind: "del", text: "two" }, { kind: "add", text: "TWO" }];
    const a = anchorOf(s, 1, 3);
    const t = threadAnchor(thread({ rows, start: 1, end: 2 }))!;
    expect(t.hash).toBe(a.hash);
    expect(t.first).toBe(a.first);
    expect(t.rows).toBe(3);
    expect(t.quote).toEqual(a.quote);
  });
  it("is null for a file comment and a thread with no rows", () => {
    expect(threadAnchor(thread({ file: true, rows: [] }))).toBeNull();
    expect(threadAnchor(thread({ rows: [] }))).toBeNull();
  });
});

describe("labels", () => {
  it("names a bot", () => {
    expect(authorOf({ author: "alice", bot: false })).toBe("alice");
    expect(authorOf({ author: "dependabot", bot: true })).toBe("dependabot (bot)");
  });
  it("a thread's label says who, where, how many and each mark", () => {
    const one = thread({ start: 12, end: 12 });
    expect(threadLabel(one, { at: 3, changed: false, start: 12, end: 12 }, plain, null)).toBe("GitHub comment by alice on line 12: Please fix.");

    const many = thread({ start: 10, end: 14, resolved: true, comments: [comment(), comment({ id: 2, author: "bob", body: "Done." }), comment({ id: 3, author: "dependabot", bot: true, body: "Ok." })] });
    expect(threadLabel(many, { at: 3, changed: false, start: 10, end: 14 }, { picked: false, note: "", sent: true }, "Claude Code")).toBe(
      "GitHub comment by alice on lines 10 to 14, 2 replies: Please fix.. Reply by bob: Done.. Reply by dependabot (bot): Ok.. Resolved. Sent to Claude Code",
    );

    const file = thread({ file: true, rows: [], start: null, end: null, original_start: null, original_end: null, comments: [comment(), comment({ id: 2, author: "bob", body: "Yes" })] });
    expect(threadLabel(file, { at: null, changed: false }, { picked: true, note: "", sent: false }, null)).toBe("GitHub comment by alice on the file, 1 reply: Please fix.. Reply by bob: Yes. In this send");

    expect(threadLabel(one, { at: 3, changed: true }, plain, null)).toBe("GitHub comment by alice on line 12: Please fix.. The line changed");
    const range = thread({ start: 10, end: 14, outdated: true, comments: [comment({ hidden_removed: true })] });
    expect(threadLabel(range, { at: 3, changed: true }, { picked: true, note: "", sent: true }, null)).toBe(
      "GitHub comment by alice on lines 10 to 14: Please fix.. Outdated. The lines changed. Hidden text removed. In this send. Copied",
    );
  });
  it("reads ten comments of a long thread, then how many more", () => {
    const comments = Array.from({ length: 12 }, (_, i) => comment({ id: i, author: `u${i}`, body: `b${i}` }));
    const label = threadLabel(thread({ comments, more_comments: 0 }), { at: 2, changed: false, start: 2, end: 2 }, plain, null);
    expect(label).toContain("11 replies: b0");
    expect(label).toContain(". Reply by u9: b9. And 2 more on GitHub");
    expect(label).not.toContain("u10");
  });
  it("cuts a body at 200 characters", () => {
    const label = threadLabel(thread({ comments: [comment({ body: "x".repeat(300) })] }), null, plain, null);
    expect(label).toBe(`GitHub comment by alice on line 2: ${"x".repeat(200)}`);
  });
  // The item's inside is hidden from a screen reader, so the label is where the user's own note is read.
  it("reads the user's note after the marks, cut at 200 characters", () => {
    const one = thread({ start: 12, end: 12 });
    expect(threadLabel(one, { at: 3, changed: false, start: 12, end: 12 }, { picked: true, note: "Use four.", sent: false }, null)).toBe("GitHub comment by alice on line 12: Please fix.. In this send. Your note: Use four.");
    expect(threadLabel(one, null, { picked: true, note: "n".repeat(300), sent: false }, null).endsWith(`. Your note: ${"n".repeat(200)}`)).toBe(true);
    expect(threadLabel(one, null, plain, null)).not.toContain("Your note");
  });
  it("a note's label", () => {
    const note: PullNote = { id: "n", kind: "review", verdict: "changes_requested", author: "alice", bot: false, body: "Rework it.", hidden_removed: true, cut: false, created_ms: null, url: null };
    expect(noteLabel(note, { picked: true, note: "", sent: true }, "Claude Code")).toBe("Review by alice · Changes requested: Rework it.. Hidden text removed. In this send. Sent to Claude Code");
    expect(noteLabel({ ...note, kind: "comment", verdict: null, hidden_removed: false }, plain, null)).toBe("Comment by alice: Rework it.");
  });
});
