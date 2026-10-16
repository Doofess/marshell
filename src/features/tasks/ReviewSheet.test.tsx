import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { Task } from "../../generated/Task";
import type { TaskChanges } from "../../generated/TaskChanges";
import { ReviewSheet, ReviewView, statusWord } from "./ReviewSheet";
import { initialReview } from "./reviewState";

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
const sheet = (changes: Partial<TaskChanges> = {}, props: Partial<Parameters<typeof ReviewSheet>[0]> = {}) =>
  renderToStaticMarkup(<ReviewSheet task={TASK} changes={{ ...CHANGES, ...changes }} agentName="Claude Code" onClose={() => {}} onMerge={() => {}} onRemove={() => {}} onAskAgent={() => {}} {...props} />);
const view = (selected: number, changes: Partial<TaskChanges> = {}) =>
  renderToStaticMarkup(<ReviewView task={TASK} changes={{ ...CHANGES, ...changes }} state={{ ...initialReview(), selected }} dispatch={() => {}} onMerge={() => {}} onRemove={() => {}} />);

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
    expect(h).toMatch(/<button[^>]*disabled=""[^>]*>Merge back…<\/button>|<button[^>]*>Merge back…<\/button>/);
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
});
