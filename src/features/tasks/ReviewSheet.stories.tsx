import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "../../design/ThemePair";
import type { Task } from "../../generated/Task";
import type { TaskChanges } from "../../generated/TaskChanges";
import type { MergePlan } from "../../generated/MergePlan";
import type { RemovePlan } from "../../generated/RemovePlan";
import { ReviewView, type ReviewViewProps } from "./ReviewSheet";
import { initialReview, type ReviewStep } from "./reviewState";

const meta: Meta = { title: "Tasks/Review sheet", parameters: { layout: "fullscreen" } };
export default meta;

const TASK: Task = { id: "t1", name: "fix-login", branch: "marshell/fix-login", base_branch: "main", repo_root: "C:\\dev\\my-app", path: "C:\\Users\\me\\.marshell\\worktrees\\my-app\\fix-login", created_ms: 1 };

const MODIFIED = [
  "diff --git a/src/login.ts b/src/login.ts",
  "index 3f2a1b0..9c8d7e6 100644",
  "--- a/src/login.ts",
  "+++ b/src/login.ts",
  "@@ -10,7 +10,8 @@ export async function login(user: string, password: string) {",
  "   const session = await open(user);",
  "-  if (!check(password)) return null;",
  "+  if (!(await check(password))) {",
  "+    return null;",
  "+  }",
  "   return session;",
  " }",
  "",
].join("\n");
const NEW_FILE = "diff --git a/notes/new dir/todo.txt b/notes/new dir/todo.txt\nnew file mode 100644\n--- /dev/null\n+++ b/notes/new dir/todo.txt\n@@ -0,0 +1,2 @@\n+retry on 401\n+show the lockout message\n";
const BINARY = "diff --git a/assets/logo.png b/assets/logo.png\nnew file mode 100644\nBinary file assets/logo.png\n";

const CHANGES: TaskChanges = {
  files: [
    { path: "assets/logo.png", status: "untracked" },
    { path: "notes/new dir/todo.txt", status: "untracked" },
    { path: "src/legacy.ts", status: "deleted" },
    { path: "src/login.ts", status: "modified" },
    { path: "src/session.ts", status: "renamed" },
    { path: "src/token.ts", status: "added" },
  ],
  diff: BINARY + NEW_FILE + MODIFIED,
  truncated: false,
  ahead: 2,
  uncommitted: true,
};

const noop = () => {};
const show = (label: string, changes: TaskChanges, selected = 0, step: ReviewStep = "changes", extra: Partial<ReviewViewProps> = {}) => (
  <ThemePair label={label} stack>
    <div style={{ position: "relative", inlineSize: 900, maxInlineSize: "100%", blockSize: 520 }}>
      <ReviewView task={TASK} changes={changes} state={{ ...initialReview(), selected, step }} dispatch={noop} onClose={noop} {...extra} />
    </div>
  </ThemePair>
);

export const FileSelected: StoryObj = { render: () => show("A modified file selected: its diff beside the list", CHANGES, 3) };
export const NewFile: StoryObj = { render: () => show("A new file, not committed", CHANGES, 1) };
export const BinaryFile: StoryObj = { render: () => show("A binary file: a sentence instead of a diff", CHANGES, 0) };
export const Truncated: StoryObj = { render: () => show("The diff was cut at the size limit", { ...CHANGES, truncated: true }, 3) };
export const Empty: StoryObj = { render: () => show("No changes yet: Merge back is disabled", { files: [], diff: "", truncated: false, ahead: 0, uncommitted: false }) };

const MERGE_PLAN: MergePlan = { blocker: null, commands: ["git add -A", 'git merge --no-ff -m "Fix login" marshell/fix-login'] };
const REMOVE_PLAN: RemovePlan = { warnings: [], sessions: 0, commands: ["git worktree remove C:\Users\me\.marshell\worktrees\my-app\fix-login", "git branch -d marshell/fix-login"] };
const merge = (over: Partial<NonNullable<ReviewViewProps["merge"]>> = {}) => ({ message: "Fix login", remove: true, plan: MERGE_PLAN, ready: true, error: null, onMessage: noop, onRemoveChange: noop, onConfirm: noop, ...over });

export const MergeStep: StoryObj = { render: () => show("Merge: the message, the checkbox and the commands that will run", CHANGES, 3, "merge", { merge: merge() }) };
export const MergeBlocked: StoryObj = {
  render: () => show("Merge blocked: the reason above the commands, Merge dimmed", CHANGES, 3, "merge", { merge: merge({ plan: { ...MERGE_PLAN, blocker: "main has uncommitted changes. Commit or stash them first." } }) }),
};
export const Conflict: StoryObj = {
  render: () => show("Merging stopped on a conflict, with the agent's button", CHANGES, 3, "conflict", { conflictFiles: ["src/login.ts", "src/session.ts"], agentName: "Claude Code", onAskAgent: noop }),
};
export const RemoveWithWarnings: StoryObj = {
  render: () => show("Remove with warnings: the button says Remove anyway", CHANGES, 3, "remove", { remove: { plan: { ...REMOVE_PLAN, warnings: ["2 commits on marshell/fix-login are not merged."] }, ready: true, error: null, onConfirm: noop } }),
};
