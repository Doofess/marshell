import { describe, expect, it } from "vitest";
import type { CiCheck } from "../../generated/CiCheck";
import type { GithubReply } from "../../generated/GithubReply";
import type { PullStatus } from "../../generated/PullStatus";
import { announcementsOf, barOf, checksOf } from "./pullModel";

const check = (name: string, state: CiCheck["state"]): CiCheck => ({ kind: "run", state, name, workflow: null, url: `https://github.com/octo-org/widgets/runs/${name}`, run_id: 1, started_ms: null, completed_ms: null });
const pullOf = (o: Partial<PullStatus> = {}): PullStatus => ({
  id: "PR_1", number: 182, title: "t", url: "https://github.com/octo-org/widgets/pull/182", state: "open", base: "main", head: "0123456789abcdef0123456789abcdef01234567", readiness: "ready", review: "none",
  checks: [], more_checks: 0, auto_merge: null, can_auto_merge: false, ...o,
});
const replyOf = (o: Partial<GithubReply> = {}): GithubReply => ({
  kind: "ready", checkout: "C:/p", host: "github.com", repo: "octo-org/widgets", branch: "feat", default_branch: "main", fork_of: null, compare_url: null, pull: null, closed: null,
  on_github: true, to_push: 0, github_ahead: false, can_write: true, merge_methods: ["merge", "squash"], auto_merge_allowed: true, push: null, push_blocked: null, reason: null, next_ms: 30_000, ...o,
});
const withPull = (p: Partial<PullStatus> = {}, o: Partial<GithubReply> = {}) => replyOf({ pull: pullOf(p), ...o });

describe("barOf", () => {
  it("reads '#182 Open · Checks: 5 passed, 1 failed'", () => {
    const checks = [...Array.from({ length: 5 }, (_, i) => check(`p${i}`, "passed")), check("f", "failed")];
    const bar = barOf(withPull({ checks, review: "required" }), "Claude Code")!;
    expect(bar.line).toBe("#182 Open \u00b7 Checks: 5 passed, 1 failed");
    expect(bar.second).toBe("Review required");
    expect(bar.main).toMatchObject({ id: "merge", label: "Merge", disabled: null });
    expect(barOf(withPull({ state: "draft", checks: [check("a", "passed"), check("b", "passed")] }), null)!.line).toBe("#182 Draft \u00b7 Checks: 2 passed");
  });
  it("reads 'No pull request · 3 commits to push' and '1 commit to push'", () => {
    expect(barOf(replyOf({ to_push: 3 }), null)!.line).toBe("No pull request \u00b7 3 commits to push");
    expect(barOf(replyOf({ to_push: 1 }), null)!.line).toBe("No pull request \u00b7 1 commit to push");
    expect(barOf(replyOf(), null)!.line).toBe("No pull request \u00b7 Nothing to push");
    expect(barOf(replyOf({ closed: 182, to_push: 3 }), null)!.line).toBe("No pull request \u00b7 #182 was closed \u00b7 3 commits to push");
    expect(barOf(replyOf(), null)!.main).toMatchObject({ id: "create", label: "Create pull request\u2026", disabled: null });
    expect(barOf(replyOf(), "Codex CLI")!.menu.map((m) => m.label)).toContain("Ask Codex CLI to open the pull request");
    expect(barOf(replyOf(), null)!.menu.some((m) => m.id === "ask_open")).toBe(false);
  });
  it("turns Create off when a branch that isn't on GitHub has no commits of its own", () => {
    expect(barOf(replyOf({ on_github: false, to_push: 0 }), null)!.main).toMatchObject({ id: "create", disabled: "Nothing differs from main." });
    // Pushed already: whether it differs from the base isn't known here, so GitHub says.
    expect(barOf(replyOf({ on_github: true, to_push: 0 }), null)!.main).toMatchObject({ id: "create", disabled: null });
    expect(barOf(replyOf({ on_github: false, to_push: 2 }), null)!.main).toMatchObject({ id: "create", disabled: null });
  });
  it("puts Push before Merge while commits wait", () => {
    const bar = barOf(withPull({}, { to_push: 2 }), null)!;
    expect(bar.line).toBe("#182 Open \u00b7 2 commits to push \u00b7 No checks");
    expect(bar.main).toMatchObject({ id: "push", label: "Push 2 commits" });
    expect(bar.menu.find((m) => m.id === "merge")).toMatchObject({ disabled: "2 commits aren't on GitHub yet. Push them first." });
    expect(barOf(withPull({}, { to_push: 1 }), null)!.main!.label).toBe("Push 1 commit");
  });
  it("turns Merge off with the reason for each readiness", () => {
    const reason = (p: Partial<PullStatus>, o: Partial<GithubReply> = {}) => barOf(withPull(p, o), null)!.main!.disabled;
    expect(reason({ readiness: "checking" })).toBe("GitHub is still checking whether #182 can merge.");
    expect(reason({ readiness: "conflicts" })).toBe("#182 has conflicts with main. Resolve them in the terminal.");
    expect(reason({ readiness: "blocked" })).toBe("GitHub won't merge #182 yet: a required check or review is missing.");
    expect(reason({ readiness: "behind" })).toBe("feat must be brought up to date with main first.");
    expect(reason({}, { can_write: false })).toBe("Your GitHub account can't merge in octo-org/widgets.");
    expect(reason({})).toBeNull();
  });
  it("orders the reasons: write access, conflicts, checking, blocked, behind", () => {
    const reason = (p: Partial<PullStatus>, o: Partial<GithubReply> = {}) => barOf(withPull(p, o), null)!.main!.disabled;
    expect(reason({ readiness: "conflicts" }, { can_write: false })).toMatch(/can't merge in/);
    expect(reason({ readiness: "checking" })).toMatch(/still checking/);
    expect(reason({ readiness: "conflicts" })).toMatch(/conflicts/);
  });
  it("offers Merge when checks pass while checks run, and when the merge is blocked", () => {
    expect(barOf(withPull({ can_auto_merge: true, checks: [check("a", "running")] }), null)!.main).toMatchObject({ id: "auto_merge", label: "Merge when checks pass" });
    expect(barOf(withPull({ can_auto_merge: true, readiness: "blocked" }), null)!.main).toMatchObject({ id: "auto_merge" });
    expect(barOf(withPull({ can_auto_merge: false, readiness: "blocked" }), null)!.main).toMatchObject({ id: "merge", disabled: expect.stringContaining("won't merge") });
    const on = barOf(withPull({ auto_merge: "squash" }), null)!;
    expect(on.line).toBe("#182 Open \u00b7 Merges when checks pass");
    expect(on.main).toBeNull();
    expect(on.menu.map((m) => m.label)).toContain("Cancel auto-merge");
  });
  it("turns Push and Create off when origin pushes elsewhere, and keeps Merge", () => {
    const msg = "origin pushes somewhere other than octo-org/widgets, so push from the terminal.";
    const none = barOf(replyOf({ push_blocked: msg, to_push: 2 }), null)!;
    expect(none.notes).toContain(msg);
    expect(none.main).toMatchObject({ id: "create", disabled: msg });
    const pr = barOf(withPull({}, { push_blocked: msg, to_push: 2 }), null)!;
    expect(pr.main).toMatchObject({ id: "push", disabled: msg });
    expect(barOf(withPull({}, { push_blocked: msg }), null)!.main).toMatchObject({ id: "merge", disabled: null });
    const ahead = barOf(replyOf({ github_ahead: true }), null)!;
    expect(ahead.notes).toContain("GitHub's feat has commits this checkout doesn't. Pull them in the terminal.");
    expect(ahead.main!.disabled).toMatch(/Pull them in the terminal/);
  });
  it("an unreachable reply shows the line and no buttons but Try again", () => {
    const bar = barOf(replyOf({ kind: "unreachable", reason: "timed out" }), null)!;
    expect(bar.line).toBe("Can't reach GitHub: timed out");
    expect(bar.main).toMatchObject({ id: "retry", label: "Try again" });
    expect(bar.menu).toEqual([]);
  });
  it("shows no bar when off or not GitHub", () => {
    expect(barOf(null, null)).toBeNull();
    expect(barOf(replyOf({ kind: "off" }), null)).toBeNull();
    expect(barOf(replyOf({ kind: "none" }), null)).toBeNull();
  });
  it("words the setup and error cases", () => {
    expect(barOf(replyOf({ kind: "no_gh" }), null)).toMatchObject({ line: "Pull requests need the GitHub CLI.", main: { id: "get_gh", label: "Get it", url: "https://cli.github.com" }, hide: true });
    expect(barOf(replyOf({ kind: "signed_out" }), null)!.main).toMatchObject({ id: "copy_sign_in", text: "gh auth login" });
    expect(barOf(replyOf({ kind: "not_found" }), null)!.line).toBe("GitHub has no repository octo-org/widgets that this sign-in can see.");
    expect(barOf(replyOf({ kind: "default_branch", branch: "main" }), null)!.line).toBe("main is the default branch on GitHub, so it has no pull request.");
    expect(barOf(replyOf({ kind: "rate_limited", next_ms: 60_000 }), null, Date.UTC(2026, 0, 1, 12, 0))!.line).toMatch(/^GitHub's limit for this sign-in is used up\. Marshell asks again at .+\.$/);
  });
  it("words a merged pull request, a fork and a running push", () => {
    const m = barOf(withPull({ state: "merged" }), "Claude Code")!;
    expect(m.line).toBe("#182 Merged");
    expect(m.main).toBeNull();
    expect(m.menu.map((x) => x.label)).toContain("Ask Claude Code to switch to main and pull");
    expect(barOf(withPull({ state: "merged" }), null)!.menu.map((x) => x.label)).toContain("Copy the commands to switch to main and pull");
    const f = barOf(replyOf({ fork_of: "upstream/widgets", compare_url: "https://github.com/upstream/widgets/compare/x" }), null)!;
    expect(f.notes).toContain("A fork of upstream/widgets");
    expect(f.link).toEqual({ label: "Open a pull request to upstream/widgets on GitHub", url: "https://github.com/upstream/widgets/compare/x" });
    expect(barOf(replyOf({ fork_of: "u/w", compare_url: "http://x" }), null)!.link).toBeNull();
    const p = barOf(withPull({}, { push: "p1" }), null)!;
    expect(p.line).toBe("Pushing feat\u2026");
    expect(p.main).toMatchObject({ id: "show_output", label: "Show output" });
  });
  it("lists Open on GitHub and Copy link only for an https link, with the method choices", () => {
    const bar = barOf(withPull({}), null)!;
    expect(bar.menu.map((m) => m.id)).toEqual(expect.arrayContaining(["draft", "open_on_github", "copy_link"]));
    expect(bar.main).toMatchObject({ method: "merge", others: [{ method: "squash", label: "Squash and merge" }] });
    const bad = barOf(withPull({ url: "http://x/pull/1" }), null)!;
    expect(bad.menu.some((m) => m.id === "open_on_github" || m.id === "copy_link")).toBe(false);
    expect(barOf(withPull({ state: "draft" }), null)!.main).toMatchObject({ id: "ready", label: "Mark as ready for review" });
  });
});

describe("checksOf", () => {
  it("counts cancelled checks, and neutral ones as passed", () => {
    expect(checksOf(pullOf({ checks: [check("a", "passed"), check("b", "neutral"), check("c", "cancelled"), check("d", "running"), check("e", "waiting"), check("f", "failed")] })).summary).toBe("Checks: 2 passed, 1 failed, 1 cancelled, 1 running, 1 waiting");
    expect(checksOf(pullOf()).summary).toBe("No checks");
  });
  it("folds passed and skipped checks behind '5 passed, 1 skipped'", () => {
    const checks = [check("x", "failed"), ...Array.from({ length: 5 }, (_, i) => check(`p${i}`, "passed")), check("s", "skipped"), check("r", "running")];
    const v = checksOf(pullOf({ checks, more_checks: 3 }));
    expect(v.listed.map((c) => c.name)).toEqual(["x", "r"]);
    expect(v.folded).toHaveLength(6);
    expect(v.foldedLabel).toBe("5 passed, 1 skipped");
    expect(v.more).toBe(3);
  });
});

describe("announcementsOf", () => {
  it("announces a change of checks once", () => {
    const run = withPull({ checks: [check("a", "running")] });
    const done = withPull({ checks: [check("a", "passed")] });
    expect(announcementsOf(run, done)).toEqual(["Checks passed on #182."]);
    expect(announcementsOf(done, done)).toEqual([]);
    expect(announcementsOf(null, done)).toEqual([]);
    expect(announcementsOf(run, withPull({ checks: [check("a", "failed")] }))).toEqual(["A check failed on #182: a."]);
    expect(announcementsOf(withPull({ checks: [check("a", "failed")] }), withPull({ checks: [check("a", "failed")] }))).toEqual([]);
    expect(announcementsOf(withPull({}), withPull({ state: "merged" }))).toEqual(["#182 was merged."]);
    expect(announcementsOf(replyOf(), withPull({}))).toEqual(["Pull request #182 was created."]);
    expect(announcementsOf(withPull({ number: 1 }), withPull({ number: 2, checks: [check("a", "passed")] }))).toEqual([]);
  });
});
