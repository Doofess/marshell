import { describe, expect, it, vi } from "vitest";
import type { MergePlan } from "../../generated/MergePlan";
import type { RemovePlan } from "../../generated/RemovePlan";
import type { Task } from "../../generated/Task";
import { ApiError } from "../../lib/api/client";
import {
  PLAN_DELAY_MS,
  conflictHeading,
  conflictPrompt,
  mergePlanRequest,
  mergeRequest,
  mergedToast,
  planFailure,
  planKey,
  readableMessage,
  removeLabel,
  removeRequest,
  removedToast,
  runMerge,
  runRemove,
  type ReviewApi,
} from "./reviewRun";

const TASK: Task = { id: "t1", name: "fix-login", branch: "marshell/fix-login", base_branch: "main", repo_root: "C:\\dev\\app", path: "C:\\w\\app\\fix-login", created_ms: 1 };

describe("readableMessage", () => {
  it("turns hyphens into spaces and capitalises the first letter", () => {
    expect(readableMessage("fix-login")).toBe("Fix login");
    expect(readableMessage("add-dark-mode-toggle")).toBe("Add dark mode toggle");
  });
  it("leaves the rest of the name alone and copes with edge cases", () => {
    expect(readableMessage("Fix-Login")).toBe("Fix Login");
    expect(readableMessage("--fix--login-")).toBe("Fix login");
    expect(readableMessage("")).toBe("");
    expect(readableMessage("édit-page")).toBe("Édit page");
  });
});

describe("the plan request", () => {
  it("waits 250 ms after the last keystroke", () => {
    expect(PLAN_DELAY_MS).toBe(250);
  });
  it("sends an empty command list: a plan is asked for, not confirmed", () => {
    expect(mergePlanRequest("Fix login", true)).toEqual({ message: "Fix login", remove: true, commands: [] });
  });
  it("keys a plan by its message and checkbox, so a changed one is never mistaken for the shown one", () => {
    expect(planKey("a", true)).toBe(planKey("a", true));
    expect(planKey("a", true)).not.toBe(planKey("a", false));
    expect(planKey("a", true)).not.toBe(planKey("b", true));
  });
});

describe("shown is run: what is sent is the list on screen", () => {
  const plan: MergePlan = { commands: ["git add -A", "git commit -m x"], blocker: null };
  const shown = (message: string, remove: boolean, p: MergePlan = plan) => ({ key: planKey(message, remove), plan: p });
  it("sends the message, the checkbox and the shown commands as they are", () => {
    expect(mergeRequest("Fix login", true, shown("Fix login", true))).toEqual({ message: "Fix login", remove: true, commands: ["git add -A", "git commit -m x"] });
  });
  it("sends nothing when there is no plan, or the plan is for another message or checkbox", () => {
    expect(mergeRequest("Fix login", true, null)).toBeNull();
    expect(mergeRequest("Fix login!", true, shown("Fix login", true))).toBeNull();
    expect(mergeRequest("Fix login", false, shown("Fix login", true))).toBeNull();
  });
  it("sends nothing while a blocker is shown", () => {
    expect(mergeRequest("", true, shown("", true, { commands: [], blocker: "Write a commit message." }))).toBeNull();
  });
  it("sends the removal's commands and warnings exactly as shown", () => {
    const rp: RemovePlan = { commands: ["git worktree remove x"], warnings: ["A session is open."], sessions: 1 };
    expect(removeRequest(rp)).toEqual({ commands: ["git worktree remove x"], warnings: ["A session is open."] });
    expect(removeRequest(null)).toBeNull();
  });
});

describe("the words", () => {
  it("says merging stopped, with the file count in the right number", () => {
    expect(conflictHeading(1, "main")).toBe("Merging stopped: 1 file conflicts with main. Nothing on main was changed.");
    expect(conflictHeading(2, "main")).toBe("Merging stopped: 2 files conflict with main. Nothing on main was changed.");
    expect(conflictHeading(3, "develop")).toBe("Merging stopped: 3 files conflict with develop. Nothing on develop was changed.");
  });
  it("asks the agent in one line, with the files joined by a comma", () => {
    expect(conflictPrompt(TASK, ["a.txt"])).toBe("Merging marshell/fix-login into main conflicts in a.txt. Rebase this branch on main and resolve those conflicts, then commit.");
    expect(conflictPrompt(TASK, ["a.txt", "src/b.ts"])).toContain("conflicts in a.txt, src/b.ts. Rebase");
  });
  it("never puts a line break in the prompt, whatever a file is called", () => {
    expect(conflictPrompt(TASK, ["a\nb.txt", "c\r\nd"])).not.toMatch(/[\r\n]/);
  });
  it("reports a merge, and the removal when it happened", () => {
    expect(mergedToast(TASK, false)).toBe("Merged marshell/fix-login into main.");
    expect(mergedToast(TASK, true)).toBe("Merged marshell/fix-login into main. Removed the worktree.");
    expect(removedToast(TASK)).toBe("Removed marshell/fix-login.");
  });
  it("labels the removal 'Remove anyway' when there are warnings", () => {
    expect(removeLabel([])).toBe("Remove");
    expect(removeLabel(["Unmerged commits."])).toBe("Remove anyway");
  });
  it("shows a failed plan call as one sentence", () => {
    expect(planFailure(new ApiError(404, "no such task"))).toBe("Couldn't load the commands: no such task");
    expect(planFailure(new Error("offline"))).toBe("Couldn't load the commands: Error: offline");
  });
});

const api = (over: Partial<ReviewApi>): ReviewApi => ({
  mergePlan: vi.fn(),
  merge: vi.fn(),
  removePlan: vi.fn(),
  remove: vi.fn(),
  ...over,
});
const REQ = { message: "m", remove: true, commands: ["git x"] };
const throws = (e: unknown) => async () => {
  throw e;
};

describe("runMerge", () => {
  it("passes the core's outcome through", async () => {
    expect(await runMerge(api({ merge: async () => ({ kind: "merged", removed: true }) }), REQ)).toEqual({ kind: "merged", removed: true });
    expect(await runMerge(api({ merge: async () => ({ kind: "conflict", files: ["a.txt"] }) }), REQ)).toEqual({ kind: "conflict", files: ["a.txt"] });
    expect(await runMerge(api({ merge: async () => ({ kind: "failed", reason: "Git said: no" }) }), REQ)).toEqual({ kind: "failed", reason: "Git said: no" });
  });
  it("sends the request it was given, untouched", async () => {
    const merge = vi.fn().mockResolvedValue({ kind: "merged", removed: false });
    await runMerge(api({ merge }), REQ);
    expect(merge).toHaveBeenCalledWith(REQ);
  });
  it("turns a refusal into a failure with the core's sentence", async () => {
    const reason = "The task changed while you were reading the commands. Review them again.";
    expect(await runMerge(api({ merge: throws(new ApiError(409, reason)) }), REQ)).toEqual({ kind: "failed", reason });
  });
  it("turns any other error into a sentence", async () => {
    expect(await runMerge(api({ merge: throws(new Error("offline")) }), REQ)).toEqual({ kind: "failed", reason: "Couldn't merge: Error: offline" });
  });
});

describe("runRemove", () => {
  it("is ok when the core says 204, and sends the request untouched", async () => {
    const remove = vi.fn().mockResolvedValue(undefined);
    expect(await runRemove(api({ remove }), { commands: ["a"], warnings: ["w"] })).toEqual({ ok: true });
    expect(remove).toHaveBeenCalledWith({ commands: ["a"], warnings: ["w"] });
  });
  it("returns the core's reason on a refusal, and a sentence on any other error", async () => {
    expect(await runRemove(api({ remove: throws(new ApiError(409, "Changed.")) }), { commands: [], warnings: [] })).toEqual({ ok: false, reason: "Changed." });
    expect(await runRemove(api({ remove: throws(new Error("x")) }), { commands: [], warnings: [] })).toEqual({ ok: false, reason: "Couldn't remove: Error: x" });
  });
});
