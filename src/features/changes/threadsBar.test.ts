import { describe, expect, it } from "vitest";
import type { ChangesDrafts } from "../../generated/ChangesDrafts";
import type { PullNote } from "../../generated/PullNote";
import type { PullThreads } from "../../generated/PullThreads";
import type { ReviewThread } from "../../generated/ReviewThread";
import type { ThreadComment } from "../../generated/ThreadComment";
import type { BarView } from "./pullModel";
import { newCommentsOf, threadsBarOf, withThreads } from "./threadsBar";

const comment = (over: Partial<ThreadComment> = {}): ThreadComment => ({ id: 1, author: "monalisa", bot: false, body: "x", hidden_removed: false, cut: false, created_ms: null, url: null, ...over });
const thread = (over: Partial<ReviewThread> = {}): ReviewThread => ({
  id: "T",
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
const note = (id: string, over: Partial<PullNote> = {}): PullNote => ({ id, kind: "comment", verdict: null, author: "monalisa", bot: false, body: "x", hidden_removed: false, cut: false, created_ms: null, url: null, ...over });
const reply = (over: Partial<PullThreads> = {}): PullThreads => ({ number: 7, viewer: "me", threads: [], more_threads: 0, notes: [], more_notes: false, ...over });
const drafts = (over: Partial<ChangesDrafts> = {}): ChangesDrafts => ({ checkout: "C:/repo", base: null, comments: [], folds: {}, picked: [], sent: [], ...over });
const many = (n: number) => Array.from({ length: n }, (_, i) => thread({ id: `T${i}` }));
const marks = (ids: number[]) => ids.map((i) => ({ id: `T${i}`, number: 7, count: 1 }));

describe("threadsBarOf", () => {
  it("the bar counts the comments to resolve", () => {
    const t = threadsBarOf(reply({ threads: [...many(3), thread({ id: "R", resolved: true }), thread({ id: "O", outdated: true })] }), undefined);
    expect(t.resolve).toEqual({ label: "3 comments to resolve", first: "T0" });
    expect(threadsBarOf(reply(), undefined).resolve).toBeNull();
    expect(threadsBarOf(null, undefined)).toEqual({ resolve: null, talk: null, add: null });
  });

  it("the bar offers the reviews and comments", () => {
    expect(threadsBarOf(reply({ notes: [note("A"), note("B")] }), undefined).talk).toEqual({ label: "2 reviews and comments" });
    expect(threadsBarOf(reply(), undefined).talk).toBeNull();
  });

  it("add all names how many, and how many were sent", () => {
    expect(threadsBarOf(reply({ threads: many(5) }), undefined).add).toEqual({ label: "Add all 5 unresolved" });
    expect(threadsBarOf(reply({ threads: many(1) }), undefined).add).toEqual({ label: "Add the unresolved comment to send" });
    const some = drafts({ sent: marks([0, 1, 2]) });
    expect(threadsBarOf(reply({ threads: many(5) }), some).add).toEqual({ label: "Add 2 unresolved · 3 already sent" });
    const none = drafts({ picked: many(5).map((t) => ({ id: t.id, number: 7, note: "" })) });
    expect(threadsBarOf(reply({ threads: many(5) }), none).add).toBeNull();
  });
});

const view = (over: Partial<BarView> = {}): BarView => ({
  line: "#7 Open",
  second: null,
  notes: [],
  main: null,
  menu: [
    { id: "draft", label: "Convert to draft", disabled: null },
    { id: "open_on_github", label: "Open on GitHub", disabled: null },
    { id: "copy_link", label: "Copy link", disabled: null },
  ],
  checks: null,
  link: null,
  hide: false,
  scope: "pull",
  resolve: null,
  talk: null,
  ...over,
});

describe("withThreads", () => {
  it("puts the row before Open on GitHub and leaves a bar without a pull request alone", () => {
    const t = { resolve: { label: "3 comments to resolve", first: "T0" }, talk: { label: "2 reviews and comments" }, add: { label: "Add all 3 unresolved" } };
    const out = withThreads(view(), t);
    expect(out.menu.map((m) => m.id)).toEqual(["draft", "add_unresolved", "open_on_github", "copy_link"]);
    expect(out.menu[1]).toEqual({ id: "add_unresolved", label: "Add all 3 unresolved", disabled: null });
    expect(out.resolve).toEqual({ label: "3 comments to resolve" });
    expect(out.talk).toEqual({ label: "2 reviews and comments" });
    const branch = view({ scope: "branch" });
    expect(withThreads(branch, t)).toBe(branch);
    expect(withThreads(view(), { resolve: null, talk: null, add: null }).menu).toHaveLength(3);
  });
});

describe("newCommentsOf", () => {
  it("new comments are announced once, without the viewer's own", () => {
    const before = reply({ threads: [thread({ comments: [comment({ id: 1 }), comment({ id: 2 })] })] });
    const after = reply({ threads: [thread({ comments: [comment({ id: 1 }), comment({ id: 2 }), comment({ id: 3 }), comment({ id: 4, author: "me" })] })] });
    expect(newCommentsOf(before, after)).toBe("1 new comment on #7.");
    expect(newCommentsOf(null, after)).toBeNull();
    expect(newCommentsOf(before, { ...after, number: 8 })).toBeNull();
    expect(newCommentsOf(after, after)).toBeNull();
    expect(newCommentsOf(after, reply({ threads: after.threads, notes: [note("N1"), note("N2")] }))).toBe("2 new comments on #7.");
  });
});
