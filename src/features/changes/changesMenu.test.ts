import { describe, expect, it } from "vitest";
import { isAction, type MenuItem, type MenuRow } from "../../components/ContextMenu/menuKeys";
import type { GithubReply } from "../../generated/GithubReply";
import type { PullNote } from "../../generated/PullNote";
import type { ReviewThread } from "../../generated/ReviewThread";
import { addressOf, changesChoice, changesMenuItems, headerMenuItems, lineMenuItems, openLineOf, openOf, threadMenuItems } from "./changesMenu";
import { type BarButton, type BarMenuItem, barOf } from "./pullModel";
import type { Item, Row, Section } from "./model";
import type { SendState } from "./threads";

const section = (rows: Row[], status: Section["status"] = "modified"): Section => ({ path: "a.ts", status, added: 1, removed: 1, rows, folded: false });
const rowItem = (file: Section, at: number): Item => ({ kind: "row", key: `row:${file.path}:${at}`, file, at });
const rowsOf = (items: MenuItem[]): MenuRow[] => items.filter(isAction);
const labels = (items: MenuItem[]) => items.map((i) => (isAction(i) ? i.label : "-"));
const on = { settings: { hide_whitespace: true, wrap: false, pull_requests: true, resolved_comments: false }, failed: false, review: "Ask Claude Code to review (types /code-review)", hasFiles: true, github: [] as BarMenuItem[], main: null as BarButton | null };
const replyOf = (o: Partial<GithubReply> = {}): GithubReply => ({
  kind: "ready", checkout: "C:/p", host: "github.com", repo: "octo-org/widgets", branch: "feat", default_branch: "main", fork_of: null, compare_url: null, pull: null, closed: null,
  on_github: true, to_push: 2, github_ahead: false, can_write: true, merge_methods: ["merge"], auto_merge_allowed: true, push: null, push_blocked: null, reason: null, next_ms: 30_000, ...o,
});

describe("changesMenuItems", () => {
  it("leaves review out for a plain shell", () => {
    expect(labels(changesMenuItems({ ...on, review: null }))).toEqual(["Refresh", "-", "Fold all files", "Unfold all files", "-", "Hide whitespace changes", "Wrap long lines", "Show resolved and outdated comments"]);
    expect(labels(changesMenuItems(on))[1]).toBe(on.review);
  });
  it("adds the bar's rows under the review row", () => {
    const view = barOf(replyOf({ push_blocked: "x" }), "Claude Code")!;
    const items = changesMenuItems({ ...on, github: view.menu, main: view.main });
    expect(labels(items).slice(0, 6)).toEqual(["Refresh", on.review, "-", "Create pull request…", "Push 2 commits", "Ask Claude Code to open the pull request"]);
    const rows = rowsOf(items);
    expect(rows.find((r) => r.label === "Create pull request…")?.disabled).toContain("origin pushes somewhere other than");
    expect(rows.find((r) => r.label === "Ask Claude Code to open the pull request")?.disabled).toBeUndefined();
    expect(changesChoice(rows.find((r) => r.label === "Create pull request…")!.id)).toEqual({ kind: "github", from: "main", id: "create" });
    expect(changesChoice(rows.find((r) => r.label === "Push 2 commits")!.id)).toEqual({ kind: "github", from: "menu", id: "push" });
    expect(changesChoice("github:menu:nope")).toBeNull();
    expect(changesChoice("github:main:retry")).toBeNull();
  });
  it("leaves out the open-pull-request row with no agent", () => {
    const view = barOf(replyOf(), null)!;
    const items = labels(changesMenuItems({ ...on, review: null, github: view.menu, main: view.main }));
    expect(items.some((l) => l.includes("open the pull request"))).toBe(false);
    expect(items).toContain("Push 2 commits");
  });
  it("makes the switches checked rows", () => {
    const rows = rowsOf(changesMenuItems(on));
    expect(rows.find((r) => r.id === "setting:hide_whitespace")?.checked).toBe(true);
    expect(rows.find((r) => r.id === "setting:wrap")?.checked).toBe(false);
  });
  it("lists the resolved comments switch after Wrap long lines and takes its row", () => {
    const rows = rowsOf(changesMenuItems(on));
    expect(rows.find((r) => r.id === "setting:resolved_comments")).toMatchObject({ label: "Show resolved and outdated comments", checked: false });
    expect(changesChoice("setting:resolved_comments")).toEqual({ kind: "setting", key: "resolved_comments" });
    expect(changesChoice("setting:nope")).toBeNull();
  });
  it("disables the switches while the settings are unread and reads defaults after a failed read", () => {
    const loading = rowsOf(changesMenuItems({ ...on, settings: null }));
    expect(loading.find((r) => r.id === "setting:wrap")?.disabled).toBeTruthy();
    const failed = rowsOf(changesMenuItems({ ...on, settings: null, failed: true }));
    expect(failed.find((r) => r.id === "setting:wrap")?.checked).toBe(false);
    expect(failed.find((r) => r.id === "setting:wrap")?.disabled).toBeTruthy();
  });
  it("disables folding all without files", () => {
    const rows = rowsOf(changesMenuItems({ ...on, hasFiles: false }));
    expect(rows.find((r) => r.id === "fold-all")?.disabled).toBeTruthy();
    expect(rows.find((r) => r.id === "unfold-all")?.disabled).toBeTruthy();
    expect(rows.find((r) => r.id === "refresh")?.disabled).toBeUndefined();
  });
});

describe("lineMenuItems", () => {
  const file = section([
    { kind: "hunk", text: "@@ -1,2 +1,2 @@" },
    { kind: "del", text: "const a", oldNo: 1 },
    { kind: "add", text: "let a", newNo: 1 },
    { kind: "context", text: "b", oldNo: 2, newNo: 2 },
  ]);
  it("disables Comment on a hunk", () => {
    expect(rowsOf(lineMenuItems(rowItem(file, 0))).find((i) => i.label === "Comment")?.disabled).toBeTruthy();
    expect(rowsOf(lineMenuItems(rowItem(file, 2))).find((i) => i.label === "Comment")?.disabled).toBeUndefined();
  });
  it("names the copy by how many lines", () => {
    expect(labels(lineMenuItems(rowItem(file, 2)))).toEqual(["Comment", "Open in Files at this line", "Copy line"]);
    expect(labels(lineMenuItems(rowItem(file, 2), 3))[2]).toBe("Copy lines");
  });
  it("cannot open a deleted file", () => {
    const gone = section(file.rows, "deleted");
    expect(rowsOf(lineMenuItems(rowItem(gone, 2))).find((i) => i.id === "open")?.disabled).toBeTruthy();
  });
});

describe("openLineOf", () => {
  const rows: Row[] = [
    { kind: "hunk", text: "@@" },
    { kind: "context", text: "x", oldNo: 4, newNo: 4 },
    { kind: "del", text: "gone", oldNo: 5 },
    { kind: "add", text: "new", newNo: 5 },
    { kind: "del", text: "last", oldNo: 6 },
  ];
  it("gives a row its own new line", () => expect(openLineOf(rows, 3)).toBe(5));
  it("gives a removed row the next new line, else the one before", () => {
    expect(openLineOf(rows, 2)).toBe(5);
    expect(openLineOf(rows, 4)).toBe(5);
  });
  it("gives a hunk the first new line after it", () => expect(openLineOf(rows, 0)).toBe(4));
  it("has no line when no row has one", () => expect(openLineOf([{ kind: "del", text: "x", oldNo: 1 }], 0)).toBeUndefined());
});

describe("openOf", () => {
  const rows: Row[] = [
    { kind: "hunk", text: "@@" },
    { kind: "del", text: "gone", oldNo: 5 },
    { kind: "add", text: "new", newNo: 5 },
  ];
  const file = section(rows);
  it("opens what the menu's Open opens: a header at no line, a row at its line or a removed row's next one", () => {
    expect(openOf({ kind: "file", key: "file:a.ts", file })).toEqual({ file, line: undefined });
    expect(openOf(rowItem(file, 1))).toEqual({ file, line: 5 });
  });
  it("refuses a deleted file with the menu's reason, from its header or a row", () => {
    const deleted = section(rows, "deleted");
    expect(openOf({ kind: "file", key: "file:a.ts", file: deleted })).toEqual({ refused: "The file is deleted, so there is nothing to open." });
    expect(openOf(rowItem(deleted, 1))).toEqual({ refused: "The file is deleted, so there is nothing to open." });
  });
  it("opens nothing for a missing item", () => expect(openOf(undefined)).toBeNull());
});

describe("headerMenuItems", () => {
  it("offers Open in Files, Fold or Unfold, and Copy path", () => {
    const open = section([{ kind: "add", text: "x", newNo: 1 }]);
    expect(labels(headerMenuItems(open))).toEqual(["Open in Files", "Fold", "Copy path"]);
    expect(labels(headerMenuItems({ ...open, folded: true }))[1]).toBe("Unfold");
  });
  it("cannot open a deleted file", () => {
    expect(rowsOf(headerMenuItems(section([], "deleted"))).find((i) => i.id === "open")?.disabled).toBeTruthy();
  });
});

describe("the comment menu", () => {
  const thread = (over: Partial<ReviewThread> = {}): ReviewThread =>
    ({ id: "PRRT_1", path: "a.ts", side: "new", file: false, start: 2, end: 2, original_start: 2, original_end: 2, resolved: false, outdated: false, can_resolve: true, can_unresolve: true, can_reply: true, rows: [], comments: [{ id: 1, author: "alice", bot: false, body: "x", hidden_removed: false, cut: false, created_ms: null, url: "https://github.com/o/r/pull/7#discussion_r1" }], more_comments: 0, minimized: 0, ...over }) as ReviewThread;
  const none: SendState = { picked: false, note: "", sent: false };
  const threadItem = (t: ReviewThread, state = none): Extract<Item, { kind: "thread" }> => ({ kind: "thread", key: `thread:${t.id}`, thread: t, placed: null, state, agent: null });
  const noteItem = (over: Partial<PullNote> = {}): Extract<Item, { kind: "talk" }> => ({ kind: "talk", key: "talk:N1", note: { id: "N1", kind: "comment", verdict: null, author: "bob", bot: false, body: "hi", hidden_removed: false, cut: false, created_ms: null, url: "https://github.com/o/r/pull/7#issuecomment-1", ...over }, state: none, agent: null });

  it("offers add, resolve, open and copy for an unresolved thread", () => {
    const rows = rowsOf(threadMenuItems(threadItem(thread()), none));
    expect(rows.map((r) => [r.id, r.label, r.disabled])).toEqual([
      ["pick", "Add to send", undefined],
      ["note", "Add your note…", undefined],
      ["reply", "Reply on GitHub…", undefined],
      ["resolve", "Resolve", undefined],
      ["open", "Open on GitHub", undefined],
      ["copy-link", "Copy link", undefined],
    ]);
  });
  it("offers Edit your note once the thread has one", () => {
    const state = { picked: true, note: "mine", sent: false };
    const rows = rowsOf(threadMenuItems(threadItem(thread(), state), state));
    expect(rows.map((r) => r.label).slice(0, 3)).toEqual(["Remove from send", "Edit your note…", "Reply on GitHub…"]);
    expect(rows[1]!.id).toBe("note");
  });
  it("gives the reason Reply is off when the thread can't be replied to", () => {
    const rows = rowsOf(threadMenuItems(threadItem(thread({ can_reply: false })), none));
    expect(rows.find((r) => r.id === "reply")).toMatchObject({ label: "Reply on GitHub…", disabled: "GitHub doesn't let this sign-in reply to this thread." });
  });
  it("shows Reply off with Replying… while a reply is out", () => {
    const rows = rowsOf(threadMenuItems(threadItem(thread()), none, true));
    expect(rows.find((r) => r.id === "reply")).toMatchObject({ label: "Reply on GitHub…", disabled: "Replying…" });
    expect(rowsOf(threadMenuItems(threadItem(thread()), none, false)).find((r) => r.id === "reply")?.disabled).toBeUndefined();
    expect(rowsOf(threadMenuItems(threadItem(thread({ can_reply: false })), none, true)).find((r) => r.id === "reply")?.disabled).toBe("GitHub doesn't let this sign-in reply to this thread.");
  });
  it("offers Unresolve for a resolved thread, with the reason when the sign-in can't", () => {
    const rows = rowsOf(threadMenuItems(threadItem(thread({ resolved: true, can_unresolve: false })), none));
    expect(rows.find((r) => r.id === "unresolve")).toMatchObject({ label: "Unresolve", disabled: "GitHub doesn't let this sign-in unresolve this thread." });
    expect(rows.find((r) => r.id === "resolve")).toBeUndefined();
    expect(rowsOf(threadMenuItems(threadItem(thread({ resolved: true })), none)).find((r) => r.id === "unresolve")?.disabled).toBeUndefined();
  });
  it("gives the reason Resolve is off", () => {
    const rows = rowsOf(threadMenuItems(threadItem(thread({ can_resolve: false })), none));
    expect(rows.find((r) => r.id === "resolve")?.disabled).toBe("GitHub doesn't let this sign-in resolve this thread.");
  });
  it("offers Remove from send for a picked thread", () => {
    const state = { picked: true, note: "", sent: false };
    const rows = rowsOf(threadMenuItems(threadItem(thread(), state), state));
    expect(rows[0]).toMatchObject({ id: "unpick", label: "Remove from send" });
  });
  it("gives a note no resolve row, and no Open or Copy link without an address", () => {
    expect(labels(threadMenuItems(noteItem(), none))).toEqual(["Add to send", "Open on GitHub", "Copy link"]);
    expect(labels(threadMenuItems(noteItem({ url: null }), none))).toEqual(["Add to send"]);
    expect(labels(threadMenuItems(threadItem(thread({ comments: [] })), none))).toEqual(["Add to send", "Add your note…", "Reply on GitHub…", "Resolve"]);
  });
  it("gives a note neither a note row nor a reply row: its text is not what a send carries of it", () => {
    const ids = rowsOf(threadMenuItems(noteItem(), none, false)).map((r) => r.id);
    expect(ids).not.toContain("reply");
    expect(ids).not.toContain("note");
  });
  it("takes only an https address", () => {
    expect(addressOf(noteItem({ url: "http://x.test/" }))).toBeNull();
    expect(addressOf(noteItem())).toBe("https://github.com/o/r/pull/7#issuecomment-1");
    expect(addressOf(threadItem(thread()))).toBe("https://github.com/o/r/pull/7#discussion_r1");
  });
  it("knows the bar's row that adds the unresolved comments", () => {
    expect(changesChoice("github:menu:add_unresolved")).toEqual({ kind: "github", from: "menu", id: "add_unresolved" });
  });
});
