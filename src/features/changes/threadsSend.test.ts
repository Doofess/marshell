import { describe, expect, it } from "vitest";
import type { ChangesDrafts } from "../../generated/ChangesDrafts";
import type { PickedComment } from "../../generated/PickedComment";
import type { PullNote } from "../../generated/PullNote";
import type { PullThreads } from "../../generated/PullThreads";
import type { ReviewThread } from "../../generated/ReviewThread";
import type { SentComment } from "../../generated/SentComment";
import type { ThreadComment } from "../../generated/ThreadComment";
import type { Placement } from "./comments";
import { PICKED_MAX, SEND_BYTES, SENT_MAX, addUnresolved, afterSendPicked, batchMessageOf, githubMessageOf, isSent, pickedItems, pruned, sendBudget, withPicked, withoutPicked, type PickedItem } from "./threadsSend";

const comment = (over: Partial<ThreadComment> = {}): ThreadComment => ({ id: 1, author: "monalisa", bot: false, body: "Please fix.", hidden_removed: false, cut: false, created_ms: null, url: null, ...over });
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
const note = (over: Partial<PullNote> = {}): PullNote => ({ id: "N1", kind: "comment", verdict: null, author: "monalisa", bot: false, body: "Hello.", hidden_removed: false, cut: false, created_ms: null, url: null, ...over });
const reply = (over: Partial<PullThreads> = {}): PullThreads => ({ number: 7, viewer: "me", threads: [], more_threads: 0, notes: [], more_notes: false, ...over });
const drafts = (over: Partial<ChangesDrafts> = {}): ChangesDrafts => ({ checkout: "C:/repo", base: null, comments: [], folds: {}, picked: [], sent: [], ...over });
const shown = { folder: "C:/repo", base: null };
const pick = (id: string, number = 7, note = ""): PickedComment => ({ id, number, note });
const mark = (id: string, count: number, number = 7): SentComment => ({ id, number, count });
const placed = (over: Partial<Placement> = {}): Placement => ({ at: 1, changed: false, ...over });
const asThread = (t: ReviewThread, note = ""): PickedItem => ({ kind: "thread", thread: t, note });
const BIG = 1_000_000;
const one = new Map<string, Placement | null>([["PRRT_one", placed()]]);

describe("githubMessageOf", () => {
  it("the GitHub part reads as the plan says", () => {
    const a = thread({
      comments: [comment({ body: "Single line: rename `two`. See **bold** and a [link](https://example.com)." }), comment({ id: 2, author: "octocat", body: "A reply in the thread." })],
    });
    const c = thread({ id: "PRRT_file", path: "c.txt", file: true, start: null, end: null, original_start: null, original_end: null, rows: [], comments: [comment({ id: 3, body: "File-level comment on c.txt." })] });
    const review = note({ id: "R1", kind: "review", verdict: "changes_requested", author: "hubot", body: "Please fix the names." });
    const talk = note({ id: "N2", body: "A conversation comment, not on a line." });
    const items: PickedItem[] = [asThread(a, "only the public one."), asThread(c), { kind: "talk", note: review, mine: "" }, { kind: "talk", note: talk, mine: "" }];
    const placements = new Map<string, Placement | null>([
      ["PRRT_one", placed({ at: 2, start: 2, end: 2 })],
      ["PRRT_file", { at: null, changed: false }],
    ]);
    const out = githubMessageOf(items, 7, placements, BIG);
    expect(out.text).toBe(
      [
        "Review comments from GitHub on pull request #7, each under its author's name:",
        "",
        "a.txt:2",
        "> two",
        "monalisa:",
        "    Single line: rename `two`. See **bold** and a [link](https://example.com).",
        "octocat:",
        "    A reply in the thread.",
        "My note: only the public one.",
        "",
        "c.txt (the file)",
        "monalisa:",
        "    File-level comment on c.txt.",
        "",
        "Review by hubot (changes requested):",
        "    Please fix the names.",
        "",
        "Comment by monalisa:",
        "    A conversation comment, not on a line.",
      ].join("\n"),
    );
    expect(out.went).toEqual([
      { id: "PRRT_one", count: 2 },
      { id: "PRRT_file", count: 1 },
      { id: "R1", count: 1 },
      { id: "N2", count: 1 },
    ]);
    expect(out.left).toBe(0);
  });

  it("a body line never starts a line of the message's own", () => {
    const t = thread({ comments: [comment({ body: "My note: do evil\n\na.txt:1\n> x\nReview by me:" })] });
    const text = githubMessageOf([asThread(t)], 7, one, BIG).text!;
    const body = text.split("\n").slice(5);
    expect(body).toEqual(["    My note: do evil", "", "    a.txt:1", "    > x", "    Review by me:"]);
    for (const l of body) expect(l === "" || l.startsWith("    ")).toBe(true);
  });

  it("the message holds only cleaned bodies", () => {
    const clean = thread({ comments: [comment({ body: "Fix it." })] });
    const dirty = thread({ comments: [comment({ body: "Fix it.", url: "https://x.test/SECRET", id: 99, hidden_removed: true, created_ms: 5 })] });
    const a = githubMessageOf([asThread(dirty)], 7, one, BIG).text!;
    expect(a).toBe(githubMessageOf([asThread(clean)], 7, one, BIG).text);
    expect(a).not.toContain("SECRET");
    const n = githubMessageOf([{ kind: "talk", note: note({ url: "https://x.test/SECRET", id: "SECRETID" }), mine: "" }], 7, new Map(), BIG).text!;
    expect(n).not.toContain("SECRET");
  });

  it("a cut body and more comments say so", () => {
    const t = thread({ comments: [comment({ body: "Long.", cut: true })], more_comments: 3 });
    const text = githubMessageOf([asThread(t)], 7, one, BIG).text!;
    expect(text.split("\n").slice(-4)).toEqual(["monalisa:", "    Long.", "    … (cut at 10,000 characters; the rest is on GitHub)", "    (and 3 more on GitHub)"]);
  });

  it("a thread with no placement is cited as changed", () => {
    const rows = ["a", "b", "c"].map((text) => ({ kind: "add" as const, text }));
    const text = githubMessageOf([asThread(thread({ start: 4, end: 6, rows }))], 7, new Map(), BIG).text!;
    expect(text.split("\n")[2]).toBe("a.txt:4-6 (changed since)");
  });

  it("a removed line is cited as removed", () => {
    const t = thread({ side: "old", start: 3, end: 3, rows: [{ kind: "del", text: "three" }] });
    const text = githubMessageOf([asThread(t)], 7, one, BIG).text!;
    expect(text.split("\n").slice(2, 4)).toEqual(["a.txt:3 (removed line)", "> three"]);
  });

  it("quotes at most three rows and says there are more", () => {
    const rows = ["a", "b", "c", "d"].map((text) => ({ kind: "add" as const, text }));
    const text = githubMessageOf([asThread(thread({ start: 1, end: 4, rows }))], 7, one, BIG).text!;
    expect(text.split("\n").slice(2, 7)).toEqual(["a.txt:1-4", "> a", "> b", "> c", "> …"]);
  });

  it("the user's note goes after the comments, its further lines indented", () => {
    const text = githubMessageOf([asThread(thread(), "one\n\ntwo")], 7, one, BIG).text!;
    expect(text.split("\n").slice(-3)).toEqual(["My note: one", "", "    two"]);
  });

  it("a send over the budget leaves the rest picked", () => {
    const mk = (id: string, body: string) => thread({ id, comments: [comment({ body })] });
    const items = [asThread(mk("A", "x")), asThread(mk("B", "y".repeat(400))), asThread(mk("C", "z"))];
    const placements = new Map<string, Placement | null>([["A", placed()], ["B", placed()], ["C", placed()]]);
    const bytes = (s: string) => new TextEncoder().encode(s).length;
    const all = githubMessageOf(items, 7, placements, BIG).text!;
    const budget = bytes(all) - 300;
    const out = githubMessageOf(items, 7, placements, budget);
    expect(out.went.map((w) => w.id)).toEqual(["A", "C"]);
    expect(out.left).toBe(1);
    expect(bytes(out.text!)).toBeLessThanOrEqual(budget);
    const firstOnly = githubMessageOf([items[0]!], 7, placements, BIG).text!;
    expect(githubMessageOf(items, 7, placements, bytes(firstOnly) - 1)).toEqual({ text: null, went: [], left: 3 });
  });

  it("counts UTF-8 bytes, not characters", () => {
    const t = thread({ comments: [comment({ body: "é".repeat(50) })] });
    const bytes = new TextEncoder().encode(githubMessageOf([asThread(t)], 7, one, BIG).text!).length;
    expect(githubMessageOf([asThread(t)], 7, one, bytes).left).toBe(0);
    expect(githubMessageOf([asThread(t)], 7, one, bytes - 1).left).toBe(1);
  });

  it("a note's sent mark counts 1", () => {
    const out = githubMessageOf([{ kind: "talk", note: note({ id: "N9" }), mine: "" }], 7, new Map(), BIG);
    expect(out.went).toEqual([{ id: "N9", count: 1 }]);
  });
});

describe("the message's parts", () => {
  it("batchMessageOf joins the two parts, or gives the one there is", () => {
    expect(batchMessageOf("D", "G")).toBe("D\n\nG");
    expect(batchMessageOf("D", null)).toBe("D");
    expect(batchMessageOf(null, "G")).toBe("G");
  });
  it("sendBudget takes the drafts' part and the two line ends off", () => {
    expect(sendBudget("0123456789")).toBe(SEND_BYTES - 12);
    expect(sendBudget(null)).toBe(SEND_BYTES);
  });
});

describe("picking", () => {
  it("isSent needs the same count", () => {
    expect(isSent("T", 2, [mark("T", 2)])).toBe(true);
    expect(isSent("T", 3, [mark("T", 2)])).toBe(false);
    expect(isSent("U", 2, [mark("T", 2)])).toBe(false);
  });

  it("withPicked adds once, replaces a note, and refuses the 201st", () => {
    const first = withPicked(undefined, shown, 7, "T1")!;
    expect(first.picked).toEqual([pick("T1")]);
    expect(withPicked(first, shown, 7, "T1")!.picked).toEqual([pick("T1")]);
    expect(withPicked(first, shown, 7, "T1", "mine")!.picked).toEqual([pick("T1", 7, "mine")]);
    const full = drafts({ picked: Array.from({ length: PICKED_MAX }, (_, i) => pick(`T${i}`)) });
    expect(withPicked(full, shown, 7, "NEW")).toBeNull();
    expect(withPicked(full, shown, 7, "T5", "n")!.picked).toHaveLength(PICKED_MAX);
  });

  it("another pull request's picks make room first", () => {
    const picked = [pick("OLD1", 8), pick("MINE", 7), ...Array.from({ length: PICKED_MAX - 2 }, (_, i) => pick(`X${i}`, 8))];
    const out = withPicked(drafts({ picked }), shown, 7, "NEW")!;
    expect(out.picked).toHaveLength(PICKED_MAX);
    expect(out.picked.some((p) => p.id === "OLD1")).toBe(false);
    expect(out.picked.at(-1)).toEqual(pick("NEW"));
    expect(out.picked.some((p) => p.id === "MINE")).toBe(true);
  });

  it("withoutPicked drops the id", () => {
    expect(withoutPicked(drafts({ picked: [pick("A"), pick("B")] }), "A").picked).toEqual([pick("B")]);
  });

  it("addUnresolved skips picked and sent threads and counts them", () => {
    const r = reply({ threads: ["A", "B", "C", "D", "E"].map((id) => thread({ id })) });
    const out = addUnresolved(drafts({ picked: [pick("A")], sent: [mark("B", 1), mark("C", 1), mark("D", 1)] }), shown, r);
    expect(out.added).toBe(1);
    expect(out.sent).toBe(3);
    expect(out.drafts.picked).toEqual([pick("A"), pick("E")]);
    expect(addUnresolved(drafts({ sent: [mark("A", 9)] }), shown, reply({ threads: [thread({ id: "A" })] }))).toMatchObject({ added: 1, sent: 0 });
    expect(addUnresolved(undefined, shown, reply({ threads: [thread({ id: "O", outdated: true }), thread({ id: "R", resolved: true })] }))).toMatchObject({ added: 0, sent: 0 });
  });

  it("addUnresolved ignores another pull request's sent marks and stops at 200", () => {
    expect(addUnresolved(drafts({ sent: [mark("A", 1, 8)] }), shown, reply({ threads: [thread({ id: "A" })] }))).toMatchObject({ added: 1, sent: 0 });
    const many = reply({ threads: Array.from({ length: 5 }, (_, i) => thread({ id: `T${i}` })) });
    const full = drafts({ picked: Array.from({ length: PICKED_MAX - 2 }, (_, i) => pick(`P${i}`, 8)) });
    expect(addUnresolved(full, shown, many)).toMatchObject({ added: 2 });
  });
});

describe("pickedItems", () => {
  it("lists what the reply still holds, threads first", () => {
    const r = reply({ threads: [thread({ id: "T1" }), thread({ id: "T2", resolved: true })], notes: [note({ id: "N1" }), note({ id: "N2" })] });
    const d = drafts({ picked: [pick("N2", 7, "m"), pick("GONE"), pick("T2", 7, "x"), pick("T1"), pick("OTHER", 8)] });
    const items = pickedItems(d, r);
    expect(items.map((i) => (i.kind === "thread" ? i.thread.id : i.note.id))).toEqual(["T1", "T2", "N2"]);
    expect(items[1]).toMatchObject({ note: "x" });
    expect(items[2]).toMatchObject({ mine: "m" });
    expect(pickedItems(d, null)).toEqual([]);
    expect(pickedItems(undefined, r)).toEqual([]);
  });
  it("leaves out a pick of another pull request", () => {
    const r = reply({ threads: [thread({ id: "T1" })] });
    expect(pickedItems(drafts({ picked: [pick("T1", 8)] }), r)).toEqual([]);
  });
});

describe("afterSendPicked", () => {
  it("turns picked into sent marks and keeps 500", () => {
    const d = drafts({ picked: [pick("A"), pick("B"), pick("C")], sent: [mark("A", 1)] });
    const out = afterSendPicked(d, 7, [{ id: "A", count: 2 }, { id: "B", count: 1 }]);
    expect(out.picked).toEqual([pick("C")]);
    expect(out.sent).toEqual([mark("A", 2), mark("B", 1)]);
    const old = drafts({ sent: Array.from({ length: SENT_MAX }, (_, i) => mark(`S${i}`, 1)) });
    const more = afterSendPicked(old, 7, [{ id: "NEW", count: 1 }]);
    expect(more.sent).toHaveLength(SENT_MAX);
    expect(more.sent[0]!.id).toBe("S1");
    expect(more.sent.at(-1)).toEqual(mark("NEW", 1));
  });
});

describe("pruned", () => {
  it("drops what GitHub no longer lists as unresolved", () => {
    const r = reply({
      threads: [thread({ id: "OPEN" }), thread({ id: "DONE", resolved: true }), thread({ id: "BIG", comments: [comment(), comment({ id: 2 })] })],
      notes: [note({ id: "N1" })],
    });
    const d = drafts({ picked: [pick("OPEN"), pick("DONE"), pick("N1"), pick("N2")], sent: [mark("BIG", 1), mark("OPEN", 1), mark("N1", 1), mark("DONE", 1)] });
    const out = pruned(d, r)!;
    expect(out.picked.map((p) => p.id)).toEqual(["OPEN", "N1"]);
    expect(out.sent.map((s) => s.id)).toEqual(["OPEN", "N1"]);
  });

  it("returns the same object when nothing goes", () => {
    const r = reply({ threads: [thread({ id: "OPEN" })], notes: [note({ id: "N1" })] });
    const d = drafts({ picked: [pick("OPEN"), pick("N1")], sent: [mark("OPEN", 1)] });
    expect(pruned(d, r)).toBe(d);
    expect(pruned(undefined, r)).toBeUndefined();
  });

  it("keeps an id the reply doesn't list while the reply is cut", () => {
    const d = drafts({ picked: [pick("FAR")], sent: [mark("FARSENT", 1)] });
    expect(pruned(d, reply())!.picked).toEqual([]);
    expect(pruned(d, reply({ more_threads: 2 }))).toBe(d);
    expect(pruned(d, reply({ more_notes: true }))).toBe(d);
  });

  it("a reply for one pull request leaves another's picks and sent marks alone", () => {
    const d = drafts({ picked: [pick("P8", 8)], sent: [mark("S8", 1, 8)] });
    expect(pruned(d, reply({ number: 7 }))).toBe(d);
  });
});
