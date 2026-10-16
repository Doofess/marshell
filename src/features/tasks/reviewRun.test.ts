import { describe, expect, it, vi } from "vitest";
import type { MergePlan } from "../../generated/MergePlan";
import type { RemovePlan } from "../../generated/RemovePlan";
import type { Task } from "../../generated/Task";
import { ApiError } from "../../lib/api/client";
import {
  PLAN_DELAY_MS,
  REMOVE_KEY,
  canConfirm,
  conflictHeading,
  conflictPrompt,
  confirmMergeRequest,
  confirmRemoveRequest,
  currentLoad,
  initialRun,
  isStale,
  mergePlanRequest,
  mergeRequest,
  mergedToast,
  planFailure,
  planKey,
  readableMessage,
  removeLabel,
  removeRequest,
  removedToast,
  runEdited,
  runFailed,
  runMerge,
  runOpened,
  runRemove,
  type PlanLoad,
  type ReviewApi,
} from "./reviewRun";

const TASK: Task = { id: "t1", name: "fix-login", branch: "marshell/fix-login", base_branch: "main", repo_root: "C:\\dev\\app", path: "C:\\w\\app\\fix-login", created_ms: 1, folder_missing: false };

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
    expect(mergePlanRequest("Fix login", true)).toEqual({ message: "Fix login", remove: true, commands: [], warnings: [] });
  });
  it("keys a plan by its message and checkbox, so a changed one is never mistaken for the shown one", () => {
    expect(planKey("a", true)).toBe(planKey("a", true));
    expect(planKey("a", true)).not.toBe(planKey("a", false));
    expect(planKey("a", true)).not.toBe(planKey("b", true));
  });
});

describe("shown is run: what is sent is the list on screen", () => {
  const plan: MergePlan = { commands: ["git add -A", "git commit -m x"], warnings: ["Closes 1 session running in this worktree."], blocker: null };
  const shown = (message: string, remove: boolean, p: MergePlan = plan) => ({ key: planKey(message, remove), plan: p });
  it("sends the message, the checkbox and the shown commands and warnings as they are", () => {
    expect(mergeRequest("Fix login", true, shown("Fix login", true))).toEqual({
      message: "Fix login",
      remove: true,
      commands: ["git add -A", "git commit -m x"],
      warnings: ["Closes 1 session running in this worktree."],
    });
  });
  it("sends nothing when there is no plan, or the plan is for another message or checkbox", () => {
    expect(mergeRequest("Fix login", true, null)).toBeNull();
    expect(mergeRequest("Fix login!", true, shown("Fix login", true))).toBeNull();
    expect(mergeRequest("Fix login", false, shown("Fix login", true))).toBeNull();
  });
  it("sends nothing while a blocker is shown", () => {
    expect(mergeRequest("", true, shown("", true, { commands: [], warnings: [], blocker: "Write a commit message." }))).toBeNull();
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
    expect(mergedToast(TASK, false, null)).toBe("Merged marshell/fix-login into main.");
    expect(mergedToast(TASK, true, null)).toBe("Merged marshell/fix-login into main. Removed the worktree.");
  });
  it("says why the worktree was kept when the removal didn't happen", () => {
    expect(mergedToast(TASK, false, "The task folder changed during the merge, so the worktree was kept.")).toBe(
      "Merged marshell/fix-login into main. The task folder changed during the merge, so the worktree was kept.",
    );
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
const REQ = { message: "m", remove: true, commands: ["git x"], warnings: [] };
const throws = (e: unknown) => async () => {
  throw e;
};

describe("runMerge", () => {
  it("passes the core's outcome through", async () => {
    expect(await runMerge(api({ merge: async () => ({ kind: "merged", removed: true, removal_note: null }) }), REQ)).toEqual({ kind: "merged", removed: true, removal_note: null });
    expect(await runMerge(api({ merge: async () => ({ kind: "conflict", files: ["a.txt"] }) }), REQ)).toEqual({ kind: "conflict", files: ["a.txt"] });
    expect(await runMerge(api({ merge: async () => ({ kind: "failed", reason: "Git said: no" }) }), REQ)).toEqual({ kind: "failed", reason: "Git said: no" });
  });
  it("sends the request it was given, untouched", async () => {
    const merge = vi.fn().mockResolvedValue({ kind: "merged", removed: false, removal_note: null });
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


const MP = (commands: string[], blocker: string | null = null): MergePlan => ({ commands, warnings: [], blocker });
const load = (key: string, round: number, plan: MergePlan | null, error: string | null = null): PlanLoad<MergePlan> => ({ key, round, plan, error });
const K = planKey("Fix login", true);

describe("currentLoad: is the plan on screen the plan for what is on screen now", () => {
  it("is the load when key and round both match", () => {
    const l = load(K, 2, MP(["git merge a"]));
    expect(currentLoad(l, K, 2)).toBe(l);
  });
  it("is null when the message or the checkbox changed since (an older key)", () => {
    expect(currentLoad(load(planKey("Fix", true), 2, MP(["x"])), K, 2)).toBeNull();
    expect(currentLoad(load(planKey("Fix login", false), 2, MP(["x"])), K, 2)).toBeNull();
  });
  it("is null when the round moved on (a failed run, or a step opened again)", () => {
    expect(currentLoad(load(K, 1, MP(["x"])), K, 2)).toBeNull();
  });
  it("is null when nothing has come back yet (the debounce is pending or the fetch is out)", () => {
    expect(currentLoad(null, K, 2)).toBeNull();
  });
  it("keeps an error load that is current, so its sentence shows", () => {
    const l = load(K, 2, null, "Could not load the commands: boom");
    expect(currentLoad(l, K, 2)).toBe(l);
  });
});

describe("canConfirm: when Merge or Remove can act", () => {
  const ok = { step: "merge" as const, ready: true, blocker: null, running: false };
  it("can when the step is open, the plan is current, nothing blocks and nothing runs", () => {
    expect(canConfirm(ok)).toBe(true);
  });
  it("cannot while the plan is not current (pending debounce, a fetch in flight, a stale plan)", () => {
    expect(canConfirm({ ...ok, ready: false })).toBe(false);
  });
  it("cannot while a blocker shows", () => {
    expect(canConfirm({ ...ok, blocker: "main has uncommitted changes" })).toBe(false);
  });
  it("cannot while a run is going, and cannot on a step that is not merge or remove", () => {
    expect(canConfirm({ ...ok, running: true })).toBe(false);
    expect(canConfirm({ ...ok, step: "merging" })).toBe(false);
    expect(canConfirm({ ...ok, step: "changes" })).toBe(false);
    expect(canConfirm({ ...ok, step: "conflict" })).toBe(false);
  });
  it("lets the remove step act too", () => {
    expect(canConfirm({ ...ok, step: "remove" })).toBe(true);
    expect(canConfirm({ ...ok, step: "removing" })).toBe(false);
  });
});

describe("confirmMergeRequest: the request comes from the current plan only", () => {
  const plan: MergePlan = { commands: ["git add -A", "git merge a"], warnings: ["Closes 2 sessions running in this worktree."], blocker: null };
  const args = { step: "merge" as const, running: false, message: "Fix login", remove: true, key: K, round: 2 };
  it("builds the request from a current plan", () => {
    expect(confirmMergeRequest({ ...args, load: load(K, 2, plan) })).toEqual({ message: "Fix login", remove: true, commands: plan.commands, warnings: plan.warnings });
  });
  it("is null for a plan of an older key", () => {
    expect(confirmMergeRequest({ ...args, load: load(planKey("Fix", true), 2, plan) })).toBeNull();
  });
  it("is null for a plan of an older round", () => {
    expect(confirmMergeRequest({ ...args, load: load(K, 1, plan) })).toBeNull();
  });
  it("is null while the plan has not arrived, or the last fetch failed", () => {
    expect(confirmMergeRequest({ ...args, load: null })).toBeNull();
    expect(confirmMergeRequest({ ...args, load: load(K, 2, null, "boom") })).toBeNull();
  });
  it("is null with a blocker", () => {
    expect(confirmMergeRequest({ ...args, load: load(K, 2, MP(["x"], "blocked")) })).toBeNull();
  });
  it("is null while a run is going, so a second press does nothing", () => {
    expect(confirmMergeRequest({ ...args, running: true, load: load(K, 2, plan) })).toBeNull();
  });
  it("is null off the merge step", () => {
    expect(confirmMergeRequest({ ...args, step: "merging", load: load(K, 2, plan) })).toBeNull();
  });
});

describe("confirmRemoveRequest", () => {
  const plan: RemovePlan = { commands: ["git worktree remove w"], warnings: ["2 commits are not merged."], sessions: 0 };
  const rl = (round: number, p: RemovePlan | null = plan): PlanLoad<RemovePlan> => ({ key: REMOVE_KEY, round, plan: p, error: null });
  const args = { step: "remove" as const, running: false, round: 3 };
  it("sends the commands and warnings of the current plan, as shown", () => {
    expect(confirmRemoveRequest({ ...args, load: rl(3) })).toEqual({ commands: plan.commands, warnings: plan.warnings });
  });
  it("is null for an older round, no plan, a run going, or another step", () => {
    expect(confirmRemoveRequest({ ...args, load: rl(2) })).toBeNull();
    expect(confirmRemoveRequest({ ...args, load: null })).toBeNull();
    expect(confirmRemoveRequest({ ...args, load: rl(3, null) })).toBeNull();
    expect(confirmRemoveRequest({ ...args, running: true, load: rl(3) })).toBeNull();
    expect(confirmRemoveRequest({ ...args, step: "removing", load: rl(3) })).toBeNull();
  });
});

describe("the run state: a refusal shows its reason and makes every plan stale", () => {
  it("starts on round 0 with no failure", () => {
    expect(initialRun).toEqual({ round: 0, failure: null });
  });
  it("a failed run (a 409, or any error) keeps the reason and bumps the round, so the plan is asked for again", () => {
    const reason = "The task changed while you were reading the commands. Review them again.";
    expect(runFailed({ round: 2, failure: null }, reason)).toEqual({ round: 3, failure: reason });
  });
  it("opening a step bumps the round and clears an old reason", () => {
    expect(runOpened({ round: 3, failure: "old" })).toEqual({ round: 4, failure: null });
  });
  it("editing the message or the checkbox clears the reason but keeps the round", () => {
    expect(runEdited({ round: 3, failure: "old" })).toEqual({ round: 3, failure: null });
  });
  it("editing with no reason up changes nothing (the same object, so no render)", () => {
    const s = { round: 3, failure: null };
    expect(runEdited(s)).toBe(s);
  });
});

describe("isStale: commands on screen that no longer match", () => {
  it("is true when a plan shows and it is not the current one", () => {
    expect(isStale(MP(["x"]), false)).toBe(true);
  });
  it("is false for a current plan, and when no plan shows at all", () => {
    expect(isStale(MP(["x"]), true)).toBe(false);
    expect(isStale(null, false)).toBe(false);
  });
});
