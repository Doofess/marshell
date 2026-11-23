import { describe, expect, it } from "vitest";
import type { ChangesDrafts } from "../../generated/ChangesDrafts";
import type { PullNote } from "../../generated/PullNote";
import type { PullStatus } from "../../generated/PullStatus";
import type { PullThreads } from "../../generated/PullThreads";
import type { ReviewThread } from "../../generated/ReviewThread";
import type { ThreadComment } from "../../generated/ThreadComment";
import type { Item, Row, Section } from "./model";
import { barOf } from "./pullModel";
import type { GithubReply } from "../../generated/GithubReply";
import type { PickedItem } from "./threadsSend";
import { addAllOf, commentSay, discarded, githubItemsOf, hasList, inDiffOrder, nextToResolve, readNotice, sendCount, sendNotice } from "./threadsPanel";
import type { GithubItems, SendState } from "./threads";

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
const note = (over: Partial<PullNote> = {}): PullNote => ({ id: "N1", kind: "comment", verdict: null, author: "bob", bot: false, body: "Hello", hidden_removed: false, cut: false, created_ms: null, url: null, ...over });
const reply = (over: Partial<PullThreads> = {}): PullThreads => ({ number: 7, viewer: "me", threads: [], more_threads: 0, notes: [], more_notes: false, ...over });
const drafts = (over: Partial<ChangesDrafts> = {}): ChangesDrafts => ({ checkout: "C:/r", base: null, comments: [], folds: {}, picked: [], sent: [], ...over });
const hunk: Row = { kind: "hunk", text: "@@ -0,0 +1,2 @@" };
const add = (text: string, newNo: number): Row => ({ kind: "add", text, newNo });
const section = (path: string, rows: Row[] = [hunk, add("one", 1), add("two", 2)], extra: Partial<Section> = {}): Section => ({ path, status: "modified", added: 0, removed: 0, rows, folded: false, ...extra });
const plain: SendState = { picked: false, note: "", sent: false };

const file = (path: string): Item => ({ kind: "file", key: `file:${path}`, file: section(path) });
const at = (id: string, over: Partial<ReviewThread> = {}): Item => ({ kind: "thread", key: `thread:${id}`, thread: thread({ id, ...over }), placed: null, state: plain, agent: null });
const row = (path: string, n: number): Item => ({ kind: "row", key: `row:${path}:${n}`, file: section(path), at: n });

describe("nextToResolve", () => {
  const items = [at("T0"), file("a.txt"), row("a.txt", 0), at("T1"), row("a.txt", 1), at("T2", { resolved: true }), at("T3")];
  it("gives the next unresolved thread after the focus", () => {
    expect(nextToResolve(items, 0)?.id).toBe("T1");
    expect(nextToResolve(items, 3)?.id).toBe("T3");
  });
  it("wraps to the first one past the end, and the only thread comes back to itself", () => {
    expect(nextToResolve(items, 6)?.id).toBe("T0");
    expect(nextToResolve([at("T9")], 0)?.id).toBe("T9");
  });
  it("is null with nothing to resolve (resolved and outdated threads are not)", () => {
    expect(nextToResolve([file("a.txt"), at("T1", { resolved: true }), at("T2", { outdated: true })], 0)).toBeNull();
    expect(nextToResolve([], 0)).toBeNull();
  });
  it("finds a thread whose folded file hides it, at its file's place", () => {
    const folded = [file("a.txt"), file("b.txt"), at("T1")];
    const hidden = thread({ id: "H", path: "a.txt" });
    // The thread of the folded a.txt comes after its header, before b.txt's.
    expect(nextToResolve(folded, 0, [hidden])?.id).toBe("H");
    expect(nextToResolve(folded, 1, [hidden])?.id).toBe("T1");
    expect(nextToResolve(folded, 2, [hidden])?.id).toBe("H");
  });
});

describe("sendCount", () => {
  it("is the drafts plus the picked items the reply still lists", () => {
    const d = drafts({ comments: [{ id: "c" } as never], picked: [{ id: "PRRT_one", number: 7, note: "" }, { id: "gone", number: 7, note: "" }, { id: "PRRT_one", number: 8, note: "" }] });
    expect(sendCount(d, reply({ threads: [thread()] }))).toBe(2);
    expect(sendCount(d, null)).toBe(1);
    expect(sendCount(undefined, null)).toBe(0);
  });
});

describe("hasList", () => {
  const nothing: GithubItems = { threads: [], placements: new Map(), notes: [], lines: [], state: () => plain, agent: null };
  const listed = { kind: "listed" };
  it("is false for no files and nothing from GitHub, and while the diff isn't listed", () => {
    expect(hasList(listed, [], undefined)).toBe(false);
    expect(hasList(listed, [], nothing)).toBe(false);
    expect(hasList(null, [section("a.txt")], undefined)).toBe(false);
    expect(hasList(null, [], { ...nothing, threads: [thread()] })).toBe(false);
  });
  it("is true for files, and for no files with a thread, an open note or a line", () => {
    expect(hasList(listed, [section("a.txt")], undefined)).toBe(true);
    expect(hasList(listed, [], { ...nothing, threads: [thread()] })).toBe(true);
    expect(hasList(listed, [], { ...nothing, notes: [note()] })).toBe(true);
    expect(hasList(listed, [], { ...nothing, lines: ["And 2 more comment threads on GitHub."] })).toBe(true);
  });
});

describe("sendNotice", () => {
  it("adds the left-over sentence only when something was left", () => {
    expect(sendNotice("Pasted 2 comments into Claude Code.", 0)).toBe("Pasted 2 comments into Claude Code.");
    expect(sendNotice("Pasted 2 comments into Claude Code.", 1)).toBe("Pasted 2 comments into Claude Code. 1 comment from GitHub didn't fit in the send and stays picked.");
    expect(sendNotice("x", 3)).toBe("x 3 comments from GitHub didn't fit in the send and stay picked.");
  });
});

describe("readNotice", () => {
  it("says what failed for the pull request, and nothing without a failure or a pull request", () => {
    expect(readNotice(7, "GitHub is unreachable.")).toBe("Can't read the comments on #7: GitHub is unreachable.");
    expect(readNotice(7, null)).toBeNull();
    expect(readNotice(null, "x")).toBeNull();
  });
});

describe("inDiffOrder", () => {
  const t = (id: string, path: string, start: number | null = 1): PickedItem => ({ kind: "thread", thread: thread({ id, path, start, end: start, original_start: start, original_end: start }), note: "" });
  const n = (id: string): PickedItem => ({ kind: "talk", note: note({ id }), mine: "" });
  const ids = (items: PickedItem[]) => items.map((i) => (i.kind === "thread" ? i.thread.id : i.note.id));
  it("picked threads go in the diff's order, notes last", () => {
    const sections = [section("a.txt"), section("b.txt")];
    const placements = new Map([["B1", { at: 2, changed: false, start: 5, end: 5 }], ["B2", { at: 1, changed: false, start: 3, end: 3 }]]);
    const items = [n("N1"), t("Z1", "gone.txt"), t("B1", "b.txt", 9), t("A2", "a.txt", 8), t("B2", "b.txt", 1), t("A1", "a.txt", 2), n("N2"), t("Z2", "other.txt")];
    // a.txt first (by start), then b.txt by the placement's start (3 before 5), then files not in the diff in the given order, then the notes.
    expect(ids(inDiffOrder(items, sections, placements))).toEqual(["A1", "A2", "B2", "B1", "Z1", "Z2", "N1", "N2"]);
  });
  it("keeps the given order on ties and leaves its input alone", () => {
    const sections = [section("a.txt")];
    const items = [t("X", "a.txt", 4), t("Y", "a.txt", 4)];
    expect(ids(inDiffOrder(items, sections, new Map()))).toEqual(["X", "Y"]);
    expect(ids(items)).toEqual(["X", "Y"]);
  });
});

describe("githubItemsOf", () => {
  const sections = [section("a.txt")];
  const base = { showOld: false, talkOpen: false, sections, agent: "Claude Code" };
  it("is undefined with no reply", () => {
    expect(githubItemsOf(null, undefined, base)).toBeUndefined();
  });
  it("shows the unresolved threads, or all of them when old ones are shown, placed against the sections", () => {
    const r = reply({ threads: [thread({ id: "T1" }), thread({ id: "T2", resolved: true }), thread({ id: "T3", path: "gone.txt" })] });
    const g = githubItemsOf(r, undefined, base)!;
    expect(g.threads.map((x) => x.id)).toEqual(["T1", "T3"]);
    expect(g.placements.get("T1")).toMatchObject({ at: 2 });
    expect(g.placements.get("T3")).toBeNull();
    expect(githubItemsOf(r, undefined, { ...base, showOld: true })!.threads.map((x) => x.id)).toEqual(["T1", "T2", "T3"]);
  });
  it("a picked outdated thread stays in the list with the switch off", () => {
    const r = reply({ threads: [thread({ id: "T1" }), thread({ id: "T2", outdated: true }), thread({ id: "T3", resolved: true }), thread({ id: "T4", outdated: true })] });
    const d = drafts({ picked: [{ id: "T2", number: 7, note: "" }, { id: "T3", number: 8, note: "" }, { id: "N1", number: 7, note: "" }] });
    const g = githubItemsOf(r, d, base)!;
    expect(g.threads.map((x) => x.id)).toEqual(["T1", "T2"]);
    expect(g.placements.get("T2")).toMatchObject({ at: 2 });
    expect(githubItemsOf(r, d, { ...base, showOld: true })!.threads.map((x) => x.id)).toEqual(["T1", "T2", "T3", "T4"]);
    expect(githubItemsOf(r, undefined, base)!.threads.map((x) => x.id)).toEqual(["T1"]);
  });
  it("holds the notes only while the list is open, with the lines for what is cut", () => {
    const r = reply({ notes: [note()], more_threads: 3, more_notes: true });
    expect(githubItemsOf(r, undefined, base)!.notes).toEqual([]);
    expect(githubItemsOf(r, undefined, base)!.lines).toEqual(["And 3 more comment threads on GitHub."]);
    const open = githubItemsOf(r, undefined, { ...base, talkOpen: true })!;
    expect(open.notes).toHaveLength(1);
    expect(open.lines).toEqual(["And 3 more comment threads on GitHub.", "More reviews and comments are on GitHub."]);
    expect(githubItemsOf(reply(), undefined, { ...base, talkOpen: true })!.lines).toEqual([]);
  });
  it("tells how an id stands in the send from the picks and the sent marks of this pull request", () => {
    const r = reply({ threads: [thread({ id: "T1" }), thread({ id: "T2", comments: [comment(), comment({ id: 2 })] })], notes: [note({ id: "N1" })] });
    const d = drafts({ picked: [{ id: "T1", number: 7, note: "mine" }, { id: "T2", number: 8, note: "" }], sent: [{ id: "T2", number: 7, count: 2 }, { id: "N1", number: 7, count: 1 }, { id: "T1", number: 7, count: 5 }] });
    const g = githubItemsOf(r, d, { ...base, talkOpen: true })!;
    expect(g.state("T1")).toEqual({ picked: true, note: "mine", sent: false });
    expect(g.state("T2")).toEqual({ picked: false, note: "", sent: true });
    expect(g.state("N1")).toEqual({ picked: false, note: "", sent: true });
    expect(g.state("nope")).toEqual(plain);
    expect(g.agent).toBe("Claude Code");
  });
});

describe("addAllOf", () => {
  const shown = { folder: "C:/r", base: null };
  it("picks the unresolved threads and says how many it added and how many go", () => {
    const r = reply({ threads: [thread({ id: "T1" }), thread({ id: "T2" })] });
    const out = addAllOf(drafts({ comments: [{ id: "c" } as never] }), shown, r);
    expect(out.drafts?.picked.map((p) => p.id)).toEqual(["T1", "T2"]);
    expect(out.say).toBe("Added 2 comments from GitHub. 3 to send.");
  });
  it("says there is nothing left to add, and changes nothing", () => {
    const r = reply({ threads: [thread({ id: "T1" })] });
    const out = addAllOf(drafts({ picked: [{ id: "T1", number: 7, note: "" }] }), shown, r);
    expect(out.drafts).toBeNull();
    expect(out.say).toBe("No unresolved comment is left to add.");
  });
  it("says the send is full at 200 and adds nothing more", () => {
    const picked = Array.from({ length: 200 }, (_, i) => ({ id: `P${i}`, number: 7, note: "" }));
    const out = addAllOf(drafts({ picked }), shown, reply({ threads: [thread({ id: "T1" })] }));
    expect(out.drafts).toBeNull();
    expect(out.say).toBe("A send holds at most 200 comments from GitHub.");
  });
});

describe("discarded", () => {
  it("clears the drafts and this pull request's picks; the sent marks and other pull requests' picks stay", () => {
    const d = drafts({ comments: [{ id: "c" } as never], picked: [{ id: "A", number: 7, note: "" }, { id: "B", number: 8, note: "" }], sent: [{ id: "S", number: 7, count: 1 }] });
    expect(discarded(d, 7)).toEqual({ ...d, comments: [], picked: [{ id: "B", number: 8, note: "" }] });
    expect(discarded(d, null).picked).toHaveLength(2);
  });
});

describe("commentSay", () => {
  const ready = (over: Partial<GithubReply> = {}) =>
    ({ kind: "ready", checkout: "C:/r", host: "github.com", repo: "o/r", branch: "feat", default_branch: "main", fork_of: null, compare_url: null, pull: null, closed: null, on_github: true, to_push: 0, github_ahead: false, can_write: true, merge_methods: ["merge"], auto_merge_allowed: false, push: null, push_blocked: null, reason: null, next_ms: 1, ...over }) as GithubReply;
  it("says why the commands can't run: off, nothing here, no pull request", () => {
    expect(commentSay(null, true)).toBe("Pull requests from GitHub are off in Settings.");
    expect(commentSay(null, false)).toBe("Pull requests aren't available for this checkout.");
    expect(commentSay(barOf(ready(), null), false)).toBe("This branch has no pull request.");
    expect(commentSay(barOf({ kind: "no_gh" } as GithubReply, null), false)).toBe("Pull requests need the GitHub CLI.");
  });
  it("is null for a bar of a pull request", () => {
    const pull = { id: "PR_1", number: 7, title: "t", url: "https://github.com/o/r/pull/7", state: "open", base: "main", head: "feat", readiness: "ready", review: "none", checks: [], more_checks: 0, auto_merge: null, can_auto_merge: false } as PullStatus;
    expect(commentSay(barOf(ready({ pull }), null), false)).toBeNull();
  });
});
