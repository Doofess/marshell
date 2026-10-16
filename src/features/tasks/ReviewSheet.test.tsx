import type { ReactElement, ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { Task } from "../../generated/Task";
import type { TaskChanges } from "../../generated/TaskChanges";
import { ReviewSheet, ReviewView, statusWord, type ReviewViewProps } from "./ReviewSheet";
import type { MergeView, RemoveView, ReviewApi } from "./reviewRun";
import { initialReview, type ReviewState } from "./reviewState";

const TASK: Task = { id: "t1", name: "fix-login", branch: "marshell/fix-login", base_branch: "main", repo_root: "C:\\dev\\app", path: "C:\\w\\app\\fix-login", created_ms: 1 };
const A = "diff --git a/src/a.ts b/src/a.ts\nindex 1..2 100644\n--- a/src/a.ts\n+++ b/src/a.ts\n@@ -1,1 +1,1 @@\n-one\n+two\n";
const C = "diff --git a/new dir/c.txt b/new dir/c.txt\nnew file mode 100644\n--- /dev/null\n+++ b/new dir/c.txt\n@@ -0,0 +1,1 @@\n+hello\n";
const BIN = "diff --git a/img/logo.png b/img/logo.png\nnew file mode 100644\nBinary file img/logo.png\n";
const CHANGES: TaskChanges = {
  files: [
    { path: "src/a.ts", status: "modified" },
    { path: "new dir/c.txt", status: "untracked" },
  ],
  diff: A + C,
  truncated: false,
  ahead: 2,
  uncommitted: true,
};
const API: ReviewApi = { mergePlan: async () => ({ commands: [], blocker: null }), merge: async () => ({ kind: "merged", removed: false }), removePlan: async () => ({ commands: [], warnings: [], sessions: 0 }), remove: async () => {} };
const sheet = (changes: Partial<TaskChanges> = {}, props: Partial<Parameters<typeof ReviewSheet>[0]> = {}) =>
  renderToStaticMarkup(<ReviewSheet task={TASK} changes={{ ...CHANGES, ...changes }} agentName="Claude Code" api={API} onClose={() => {}} onDone={() => {}} onAskAgent={() => {}} {...props} />);
const view = (selected: number, changes: Partial<TaskChanges> = {}) =>
  renderToStaticMarkup(<ReviewView task={TASK} changes={{ ...CHANGES, ...changes }} state={{ ...initialReview(), selected }} dispatch={() => {}} />);

const noop = () => {};
const CMDS = ['git -C "C:\\w\\app\\fix-login" add -A', 'git -C "C:\\w\\app\\fix-login" commit -q -m "Fix login"', 'git -C "C:\\dev\\app" merge --squash marshell/fix-login'];
const MERGE: MergeView = { message: "Fix login", remove: true, plan: { commands: CMDS, blocker: null }, ready: true, error: null, onMessage: noop, onRemoveChange: noop, onConfirm: noop };
const REMOVE: RemoveView = { plan: { commands: ['git -C "C:\\dev\\app" worktree remove "C:\\w\\app\\fix-login"'], warnings: [], sessions: 0 }, ready: true, error: null, onConfirm: noop };
const stepProps = (step: ReviewState["step"], extra: Partial<ReviewViewProps> = {}): ReviewViewProps => ({ task: TASK, changes: CHANGES, state: { step, selected: 0 }, dispatch: noop, onClose: noop, ...extra });
const step = (s: ReviewState["step"], extra: Partial<ReviewViewProps> = {}) => renderToStaticMarkup(<ReviewView {...stepProps(s, extra)} />);

/** The buttons ReviewView puts in the heading row, found by their label, so a click can be tried without a DOM. */
type Btn = ReactElement<{ children?: ReactNode; onClick?: () => void; "aria-disabled"?: boolean }>;
function buttonsOf(props: ReviewViewProps): Btn[] {
  const out: Btn[] = [];
  const walk = (n: ReactNode) => {
    if (Array.isArray(n)) return n.forEach(walk);
    if (n === null || typeof n !== "object" || !("props" in n)) return;
    const el = n as Btn;
    if (el.type === "button") out.push(el);
    walk(el.props.children);
  };
  walk((ReviewView(props) as ReactElement<{ actions: ReactNode }>).props.actions);
  return out;
}
const labelOf = (b: Btn) => [b.props.children].flat().join("");
const click = (props: ReviewViewProps, label: string) => {
  const b = buttonsOf(props).find((x) => labelOf(x) === label);
  expect(b, `no button "${label}"`).toBeDefined();
  b!.props.onClick?.();
};

describe("ReviewSheet: the heading", () => {
  const h = sheet();
  it("is a modal dialog titled with the branch", () => {
    expect(h).toMatch(/role="dialog"[^>]*aria-modal="true"/);
    expect(h).toContain("Changes in marshell/fix-login");
  });
  it("says what the changes are measured against", () => {
    expect(h).toContain("Against main · 2 commits ahead · uncommitted changes");
  });
  it("leaves out the parts that are zero or false, and says 'commit' for one", () => {
    expect(sheet({ ahead: 0, uncommitted: false })).toContain("Against main<");
    expect(sheet({ ahead: 1, uncommitted: false })).toContain("Against main · 1 commit ahead<");
    expect(sheet({ ahead: 0, uncommitted: true })).toContain("Against main · uncommitted changes<");
  });
});

describe("ReviewSheet: the file list", () => {
  const h = sheet();
  const list = h.slice(h.indexOf('role="listbox"'), h.indexOf('class="review__diff"'));
  it("is a labelled listbox of options", () => {
    expect(h).toMatch(/role="listbox"[^>]*aria-label="Changed files"|aria-label="Changed files"[^>]*role="listbox"/);
    expect(h.match(/role="option"/g)).toHaveLength(2);
  });
  it("names the status in words, beside the path", () => {
    expect(list).toContain("Modified");
    expect(list).toContain("src/a.ts");
    expect(list).toContain("New, not committed");
    expect(list).toContain("new dir/c.txt");
  });
  it("selects the first file, and points the listbox at it", () => {
    const opts = h.match(/<[^>]*role="option"[^>]*>/g)!;
    expect(opts[0]).toContain('aria-selected="true"');
    expect(opts[1]).toContain('aria-selected="false"');
    const id = opts[0]!.match(/id="([^"]+)"/)![1];
    expect(h).toContain(`aria-activedescendant="${id}"`);
  });
  it("can be reached with Tab", () => {
    expect(h).toMatch(/<[^>]*role="listbox"[^>]*tabindex="0"/);
  });
  it("gives every status a word", () => {
    expect(["added", "modified", "deleted", "renamed", "untracked"].map(statusWord)).toEqual(["Added", "Modified", "Deleted", "Renamed", "New, not committed"]);
    expect(statusWord("copied")).toBe("copied");
  });
});

describe("ReviewView: the selected file's diff", () => {
  it("shows the first file's diff, expanded, and not the others", () => {
    const h = view(0);
    expect(h).toContain("<figure");
    expect(h).toContain("two");
    expect(h).not.toContain("hello");
    expect(h).not.toContain("Show all");
  });
  it("shows the second file's diff when it is selected, with a path that has a space", () => {
    const h = view(1);
    expect(h).toContain("hello");
    expect(h).toContain("new dir/c.txt");
    expect(h).not.toContain(">two<");
  });
  it("gives a binary file a note and no diff", () => {
    const h = view(0, { files: [{ path: "img/logo.png", status: "added" }], diff: BIN });
    expect(h).toContain("This is a binary file, so there is nothing to preview.");
    expect(h).not.toContain("<figure");
  });
  it("gives a large file a note and no diff", () => {
    const h = view(0, { files: [{ path: "big.log", status: "untracked" }], diff: "diff --git a/big.log b/big.log\nnew file mode 100644\nLarge file big.log\n" });
    expect(h).toContain("This file is too large to preview.");
  });
  it("says so when a file's diff is not in the text (cut off, or a rename with no edits)", () => {
    const h = view(1, { diff: A, truncated: true });
    expect(h).toContain("There is no diff to show for this file.");
  });
  it("reads a renamed file by its new path", () => {
    const d = "diff --git a/old.ts b/new.ts\nrename from old.ts\nrename to new.ts\n";
    const h = view(0, { files: [{ path: "new.ts", status: "renamed" }], diff: d });
    expect(h).toContain("There is no diff to show for this file.");
  });
});

describe("ReviewSheet: truncated and empty", () => {
  it("says when the diff is too large to show in full", () => {
    expect(sheet({ truncated: true })).toContain("The diff is too large to show in full.");
    expect(sheet()).not.toContain("too large to show in full");
  });
  it("shows the empty state, with no list, and disables Merge back", () => {
    const h = sheet({ files: [], diff: "", ahead: 0, uncommitted: false });
    expect(h).toContain("No changes yet.");
    expect(h).not.toContain('role="listbox"');
    expect(h.match(/<button[^>]*>Merge back…/)![0]).toContain('disabled=""');
  });
});

describe("ReviewSheet: the buttons", () => {
  const h = sheet();
  it("has Merge back…, Remove… and Close", () => {
    expect(h).toContain(">Merge back…</button>");
    expect(h).toContain(">Remove…</button>");
    expect(h).toContain(">Close</button>");
    expect(h.match(/<button[^>]*>Merge back…/)![0]).not.toContain("disabled");
  });
  it("is a dialog that takes focus itself, so Esc and Tab are handled by useDialogFocus", () => {
    expect(h).toMatch(/<div[^>]*role="dialog"[^>]*tabindex="-1"/);
  });
  it("opens the merge and the remove steps, and nothing else", () => {
    const dispatch = vi.fn();
    click(stepProps("changes", { dispatch }), "Merge back…");
    click(stepProps("changes", { dispatch }), "Remove…");
    expect(dispatch.mock.calls).toEqual([[{ type: "open", step: "merge" }], [{ type: "open", step: "remove" }]]);
  });
  it("has no live region while nothing runs", () => {
    expect(h).not.toContain('role="status"');
    expect(h).not.toContain('role="alert"');
  });
});

describe("the merge step", () => {
  const h = step("merge", { merge: MERGE });
  it("has a Commit message field with the task name made readable", () => {
    expect(h).toMatch(/<label[^>]*for="([^"]+)"[^>]*>Commit message<\/label>/);
    const id = h.match(/<label[^>]*for="([^"]+)"[^>]*>Commit message/)![1];
    const input = h.match(/<input[^>]*>/g)!.find((i) => i.includes(`id="${id}"`))!;
    expect(input).toContain('value="Fix login"');
    expect(input).toContain('type="text"');
    expect(input).toContain('class="field__control review__message"');
  });
  it("has the checkbox 'Remove the worktree and branch afterwards', checked, and unchecked when it is off", () => {
    const box = h.match(/<input[^>]*type="checkbox"[^>]*>/)![0];
    expect(box).toContain('checked=""');
    expect(h).toContain("Remove the worktree and branch afterwards");
    expect(step("merge", { merge: { ...MERGE, remove: false } }).match(/<input[^>]*type="checkbox"[^>]*>/)![0]).not.toContain("checked");
  });
  it("shows the commands from the plan", () => {
    expect(h).toContain('class="command-list"');
    expect(h).toContain("merge --squash marshell/fix-login");
  });
  it("names the step in the heading, with the branch and what it merges into", () => {
    expect(h).toContain("Merge marshell/fix-login into main");
  });
  it("enables Merge when the shown commands are current", () => {
    expect(h.match(/<button[^>]*>Merge<\/button>/)![0]).not.toContain("aria-disabled");
  });
  it("disables Merge, without taking it out of the tab order, while the commands are being fetched", () => {
    const b = step("merge", { merge: { ...MERGE, ready: false } }).match(/<button[^>]*>Merge<\/button>/)![0];
    expect(b).toContain('aria-disabled="true"');
    expect(b).not.toMatch(/(?<!aria-)disabled=/);
  });
  it("shows the blocker above the commands and disables Merge", () => {
    const blocked = step("merge", { merge: { ...MERGE, plan: { commands: CMDS, blocker: "Write a commit message." } } });
    expect(blocked).toContain("Write a commit message.");
    expect(blocked.indexOf("Write a commit message.")).toBeLessThan(blocked.indexOf('class="command-list"'));
    expect(blocked.match(/<button[^>]*>Merge<\/button>/)![0]).toContain('aria-disabled="true"');
  });
  it("shows no list before a plan has come", () => {
    expect(step("merge", { merge: { ...MERGE, plan: null, ready: false } })).not.toContain('class="command-list"');
  });
  it("shows why a merge failed, with Merge enabled again", () => {
    const failed = step("merge", { merge: { ...MERGE, error: "Git said: index.lock exists" } });
    expect(failed).toContain("Git said: index.lock exists");
    expect(failed).toMatch(/role="alert"[^>]*>Git said: index.lock exists/);
    expect(failed.match(/<button[^>]*>Merge<\/button>/)![0]).not.toContain("aria-disabled");
  });
  it("has Back, and Back goes to the changes", () => {
    expect(h).toContain(">Back</button>");
    const dispatch = vi.fn();
    click(stepProps("merge", { merge: MERGE, dispatch }), "Back");
    expect(dispatch).toHaveBeenCalledWith({ type: "back" });
  });
  it("confirms only when the commands on screen are current", () => {
    const onConfirm = vi.fn();
    click(stepProps("merge", { merge: { ...MERGE, ready: false, onConfirm } }), "Merge");
    click(stepProps("merge", { merge: { ...MERGE, plan: { commands: CMDS, blocker: "Write a commit message." }, onConfirm } }), "Merge");
    expect(onConfirm).not.toHaveBeenCalled();
    click(stepProps("merge", { merge: { ...MERGE, onConfirm } }), "Merge");
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });
});

describe("while merging or removing", () => {
  const merging = step("merging", { merge: MERGE });
  it("says 'Merging…' once, in a polite status line", () => {
    expect(merging.match(/Merging…/g)).toHaveLength(1);
    expect(merging).toMatch(/<p[^>]*role="status"[^>]*>Merging…<\/p>/);
    expect(merging).not.toContain("aria-live");
  });
  it("keeps the commands on screen and the buttons where they were, but inert", () => {
    expect(merging).toContain('class="command-list"');
    expect(merging.match(/<button[^>]*>Merge<\/button>/)![0]).toContain('aria-disabled="true"');
    expect(merging.match(/<button[^>]*>Back<\/button>/)![0]).toContain('aria-disabled="true"');
    expect(merging).not.toContain(">Close</button>");
    expect(merging).not.toContain("Merge back…");
  });
  it("starts nothing and goes nowhere when a button is pressed", () => {
    const dispatch = vi.fn();
    const onConfirm = vi.fn();
    const onClose = vi.fn();
    const props = stepProps("merging", { merge: { ...MERGE, onConfirm }, dispatch, onClose });
    for (const b of buttonsOf(props)) b.props.onClick?.();
    expect(onConfirm).not.toHaveBeenCalled();
    expect(dispatch).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });
  it("says 'Removing…' once while a removal runs, and its buttons are inert too", () => {
    const removing = step("removing", { remove: REMOVE });
    expect(removing.match(/Removing…/g)).toHaveLength(1);
    expect(removing).toMatch(/<p[^>]*role="status"[^>]*>Removing…<\/p>/);
    expect(removing.match(/<button[^>]*>Remove<\/button>/)![0]).toContain('aria-disabled="true"');
    const onConfirm = vi.fn();
    const dispatch = vi.fn();
    for (const b of buttonsOf(stepProps("removing", { remove: { ...REMOVE, onConfirm }, dispatch }))) b.props.onClick?.();
    expect(onConfirm).not.toHaveBeenCalled();
    expect(dispatch).not.toHaveBeenCalled();
  });
});

describe("the conflict step", () => {
  const files = ["a.txt", "src/b.ts"];
  const h = step("conflict", { conflictFiles: files, agentName: "Claude Code", onAskAgent: noop });
  it("says merging stopped and that main is unchanged, in the right number", () => {
    expect(h).toContain("Merging stopped: 2 files conflict with main. Nothing on main was changed.");
    expect(step("conflict", { conflictFiles: ["a.txt"] })).toContain("Merging stopped: 1 file conflicts with main. Nothing on main was changed.");
  });
  it("lists the files", () => {
    expect(h).toMatch(/<li[^>]*>[^<]*a\.txt/);
    expect(h).toMatch(/<li[^>]*>[^<]*src\/b\.ts/);
  });
  it("has 'Ask <Agent> to fix them' and Close", () => {
    expect(h).toContain(">Ask Claude Code to fix them</button>");
    expect(h).toContain(">Close</button>");
  });
  it("leaves out the ask button when no running agent belongs to the task", () => {
    const none = step("conflict", { conflictFiles: files, agentName: null });
    expect(none).not.toContain("to fix them");
    expect(none).toContain(">Close</button>");
  });
  it("hands the agent one line, never an Enter, then closes the sheet", () => {
    const onAskAgent = vi.fn();
    const onClose = vi.fn();
    click(stepProps("conflict", { conflictFiles: ["a.txt"], agentName: "Claude Code", onAskAgent, onClose }), "Ask Claude Code to fix them");
    expect(onAskAgent).toHaveBeenCalledWith("Merging marshell/fix-login into main conflicts in a.txt. Rebase this branch on main and resolve those conflicts, then commit.");
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onAskAgent.mock.invocationCallOrder[0]).toBeLessThan(onClose.mock.invocationCallOrder[0]!);
  });
  it("joins the files with a comma in the prompt", () => {
    const onAskAgent = vi.fn();
    click(stepProps("conflict", { conflictFiles: files, agentName: "Claude Code", onAskAgent }), "Ask Claude Code to fix them");
    expect(onAskAgent.mock.calls[0]![0]).toContain("conflicts in a.txt, src/b.ts.");
  });
  it("closes with Close", () => {
    const onClose = vi.fn();
    click(stepProps("conflict", { conflictFiles: files, onClose }), "Close");
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});

describe("the remove step", () => {
  const h = step("remove", { remove: REMOVE });
  it("shows the commands and a Remove button, and Back", () => {
    expect(h).toContain('class="command-list"');
    expect(h).toContain("worktree remove");
    expect(h).toContain(">Remove</button>");
    expect(h).toContain(">Back</button>");
    expect(h).toContain("Remove marshell/fix-login");
  });
  it("lists the warnings and reads 'Remove anyway' when there are some", () => {
    const warned = step("remove", { remove: { ...REMOVE, plan: { ...REMOVE.plan!, warnings: ["Two commits are not on main.", "A session is open."] } } });
    expect(warned).toMatch(/<ul[^>]*>[\s\S]*<li[^>]*>Two commits are not on main\.<\/li>[\s\S]*<li[^>]*>A session is open\.<\/li>/);
    expect(warned).toContain(">Remove anyway</button>");
    expect(warned.indexOf("Two commits are not on main.")).toBeLessThan(warned.indexOf('class="command-list"'));
    expect(h).not.toContain("Remove anyway");
    expect(h).not.toContain("<ul");
  });
  it("disables Remove while the plan is being fetched, and confirms only when it is current", () => {
    const wait = step("remove", { remove: { ...REMOVE, ready: false } });
    expect(wait.match(/<button[^>]*>Remove<\/button>/)![0]).toContain('aria-disabled="true"');
    const onConfirm = vi.fn();
    click(stepProps("remove", { remove: { ...REMOVE, ready: false, onConfirm } }), "Remove");
    expect(onConfirm).not.toHaveBeenCalled();
    click(stepProps("remove", { remove: { ...REMOVE, onConfirm } }), "Remove");
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });
  it("shows why a removal failed", () => {
    expect(step("remove", { remove: { ...REMOVE, error: "The task changed while you were reading the commands. Review them again." } })).toMatch(/role="alert"[^>]*>The task changed/);
  });
});
