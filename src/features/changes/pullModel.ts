import type { CiCheck } from "../../generated/CiCheck";
import type { GithubReply } from "../../generated/GithubReply";
import type { MergeMethod } from "../../generated/MergeMethod";
import type { PullStatus } from "../../generated/PullStatus";
import { githubCopy as c, METHOD_BUTTONS, METHOD_LABELS, REVIEW_LABELS, SUMMARY_ORDER } from "./githubCopy";

export type BarButtonId = "push" | "create" | "merge" | "auto_merge" | "ready" | "show_output" | "get_gh" | "copy_sign_in" | "retry";
export type BarMenuId = "push" | "merge" | "draft" | "cancel_auto_merge" | "hide" | "ask_open" | "ask_switch" | "copy_switch" | "add_unresolved" | "open_on_github" | "copy_link";

/** A button of the bar. A button that is off stays in the view, with the sentence that says why in `disabled`. */
export type BarButton = {
  id: BarButtonId;
  label: string;
  /** The reason the button is off, or null when it works. */
  disabled: string | null;
  /** Merge and Merge when checks pass: the default method, and the other methods for the ▾. */
  method?: MergeMethod;
  others?: { method: MergeMethod; label: string }[];
  /** An address to open (https only). */
  url?: string;
  /** Text to copy, and what to announce once copied. */
  text?: string;
  done?: string;
};

export type BarMenuItem = {
  id: BarMenuId;
  label: string;
  disabled: string | null;
  url?: string;
  /** The method a "merge" item uses. */
  method?: MergeMethod;
};

export type ChecksView = {
  /** "Checks: 5 passed, 1 failed", or "No checks". */
  summary: string;
  /** Failed, cancelled, running and waiting checks, in the reply's order. */
  listed: CiCheck[];
  /** Passed, neutral and skipped checks, in the reply's order. */
  folded: CiCheck[];
  /** "5 passed, 1 skipped", or "" with nothing folded. */
  foldedLabel: string;
  /** Checks GitHub has that the reply doesn't list. */
  more: number;
};

export type BarView = {
  line: string;
  second: string | null;
  notes: string[];
  main: BarButton | null;
  menu: BarMenuItem[];
  checks: ChecksView | null;
  link: { label: string; url: string } | null;
  hide: boolean;
  /** "branch": a branch with no pull request; "pull": a branch with one; "other": GitHub isn't available, or there is no branch to ask about. */
  scope: "branch" | "pull" | "other";
  /** The pull request's comments to resolve and its reviews and comments: buttons the bar gets once GitHub's threads are read. */
  resolve: { label: string } | null;
  talk: { label: string } | null;
};

const FOLDED = new Set<CiCheck["state"]>(["passed", "neutral", "skipped"]);
const https = (u: string | null | undefined): u is string => typeof u === "string" && u.startsWith("https://");
const bar = (line: string, o: Partial<BarView> = {}): BarView => ({ line, second: null, notes: [], main: null, menu: [], checks: null, link: null, hide: false, scope: "other", resolve: null, talk: null, ...o });

/** "Checks: 5 passed, 1 failed" and the checks split into the ones to list and the ones to fold. */
export function checksOf(pull: PullStatus): ChecksView {
  const counts = (list: CiCheck[]) =>
    SUMMARY_ORDER.map(({ word, states }) => ({ word, count: list.filter((k) => states.includes(k.state)).length }))
      .filter((p) => p.count > 0)
      .map((p) => `${p.count} ${p.word}`)
      .join(", ");
  const folded = pull.checks.filter((k) => FOLDED.has(k.state));
  const all = counts(pull.checks);
  return {
    summary: all === "" ? c.noChecks : c.checksSummary(all),
    listed: pull.checks.filter((k) => !FOLDED.has(k.state)),
    folded,
    foldedLabel: counts(folded),
    more: pull.more_checks,
  };
}

function timeOf(ms: number): string {
  return new Date(ms).toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" });
}

/**
 * What the pull request bar shows for a reply, or null for no bar. Pure: `now` (epoch ms, from the caller) only serves
 * the rate-limited line, which names the time `now + next_ms`; without it the line says "soon".
 */
export function barOf(reply: GithubReply | null, agent: string | null, now?: number): BarView | null {
  if (reply === null) return null;
  const r = reply;
  switch (r.kind) {
    case "off":
    case "none":
      return null;
    case "no_gh":
      return bar(c.noGh, { main: { id: "get_gh", label: c.getGh, disabled: null, url: c.ghUrl }, menu: [{ id: "hide", label: c.hide, disabled: null }], hide: true });
    case "signed_out":
      return bar(c.signedOut, { main: { id: "copy_sign_in", label: c.copy, disabled: null, text: c.signInCommand, done: c.copied }, menu: [{ id: "hide", label: c.hide, disabled: null }], hide: true });
    case "unreachable":
      return bar(c.unreachable(r.reason ?? c.unreachableFallback), { main: { id: "retry", label: c.tryAgain, disabled: null } });
    case "not_found":
      return bar(c.notFound(r.repo));
    case "rate_limited":
      return bar(now === undefined ? c.rateLimitedSoon : c.rateLimited(timeOf(now + r.next_ms)));
    case "default_branch":
      return bar(c.defaultBranch(r.branch ?? r.default_branch ?? ""));
    case "ready":
      break;
  }
  if (r.branch === null) return null;
  const pull = r.pull !== null && r.pull.state !== "closed" ? r.pull : null;
  const closed = r.closed ?? (r.pull?.state === "closed" ? r.pull.number : null);
  const notes: string[] = [];
  if (r.github_ahead) notes.push(c.githubAhead(r.branch));
  if (r.push_blocked !== null) notes.push(c.pushBlocked(r.repo));
  if (r.fork_of !== null) notes.push(c.fork(r.fork_of));
  const pushOff = r.github_ahead ? c.githubAhead(r.branch) : r.push_blocked !== null ? c.pushBlocked(r.repo) : null;
  const running = r.push !== null;
  const pushing = running ? c.pushing(r.branch) : null;
  const show: BarButton = { id: "show_output", label: c.showOutput, disabled: null };

  if (pull === null) {
    const parts = [c.noPull];
    if (closed !== null) parts.push(c.closedPull(closed));
    parts.push(r.to_push > 0 ? c.toPush(r.to_push) : c.nothingToPush);
    const menu: BarMenuItem[] = [];
    if (r.to_push > 0 && !running) menu.push({ id: "push", label: c.pushButton(r.to_push), disabled: pushOff });
    if (agent !== null) menu.push({ id: "ask_open", label: c.askOpen(agent), disabled: null });
    const link = r.fork_of !== null && https(r.compare_url) ? { label: c.forkLink(r.fork_of), url: r.compare_url } : null;
    // A branch that isn't on GitHub counts its commits from the base: none means there is nothing to open one for.
    // For a pushed branch that isn't known here, and GitHub says so itself.
    const same = !r.on_github && r.to_push === 0 && r.default_branch !== null ? c.nothingDiffers(r.default_branch) : null;
    return bar(pushing ?? parts.join(" · "), { notes, main: running ? show : { id: "create", label: c.create, disabled: pushOff ?? same }, menu, link, scope: "branch" });
  }

  const num = pull.number;
  const methods: MergeMethod[] = r.merge_methods.length > 0 ? r.merge_methods : ["merge"];
  const def = methods[0]!;
  const others = methods.slice(1).map((m) => ({ method: m, label: METHOD_LABELS[m] }));
  const checks = checksOf(pull);
  const menu: BarMenuItem[] = [];
  if (pull.state === "merged") {
    const def2 = r.default_branch;
    if (def2 !== null) menu.push(agent !== null ? { id: "ask_switch", label: c.askSwitch(agent, def2), disabled: null } : { id: "copy_switch", label: c.copySwitch(def2), disabled: null });
  }
  const links: BarMenuItem[] = https(pull.url)
    ? [
        { id: "open_on_github", label: c.openOnGithub, disabled: null, url: pull.url },
        { id: "copy_link", label: c.copyLink, disabled: null, url: pull.url },
      ]
    : [];

  if (pull.state === "merged") {
    return bar(`#${num} ${c.pullState.merged}`, { notes, menu: [...menu, ...links], link: https(pull.url) ? { label: c.mergedLink(num), url: pull.url } : null, scope: "pull" });
  }

  // Why Merge is off, first match: commits to push, no write access, Conflicts, Checking, Blocked (only when auto-merge
  // can't be offered instead), Behind.
  const checksPending = pull.checks.some((k) => k.state === "running" || k.state === "waiting");
  const autoOffered = pull.can_auto_merge && (checksPending || pull.readiness === "blocked");
  const mergeOff =
    r.to_push > 0 ? c.notPushedYet(r.to_push)
    : !r.can_write ? c.noWrite(r.repo)
    : pull.readiness === "conflicts" ? c.conflicts(num, pull.base)
    : pull.readiness === "checking" ? c.checking(num)
    : pull.readiness === "blocked" && !autoOffered ? c.blocked(num)
    : pull.readiness === "behind" ? c.behind(r.branch, pull.base)
    : null;
  const mergeButton = (id: "merge" | "auto_merge", label: string, disabled: string | null): BarButton => ({ id, label, disabled, method: def, others });

  let main: BarButton | null;
  if (running) main = show;
  else if (r.to_push > 0) main = { id: "push", label: c.pushButton(r.to_push), disabled: pushOff };
  else if (pull.state === "draft") main = { id: "ready", label: c.ready, disabled: null };
  else if (pull.auto_merge !== null) main = null;
  else if (autoOffered) main = mergeButton("auto_merge", c.mergeWhenChecksPass, !r.can_write ? c.noWrite(r.repo) : pull.readiness === "conflicts" ? c.conflicts(num, pull.base) : null);
  else main = mergeButton("merge", METHOD_BUTTONS[def], mergeOff);

  if (r.to_push > 0 && !running) menu.push({ id: "merge", label: METHOD_BUTTONS[def], disabled: mergeOff, method: def });
  if (pull.state === "open" && pull.auto_merge === null) menu.push({ id: "draft", label: c.draft, disabled: null });
  if (pull.auto_merge !== null) menu.push({ id: "cancel_auto_merge", label: c.cancelAutoMerge, disabled: null });
  menu.push(...links);

  const parts = [`#${num} ${c.pullState[pull.state]}`];
  if (r.to_push > 0) parts.push(c.toPush(r.to_push));
  if (pull.auto_merge !== null) parts.push(c.mergesWhenChecksPass);
  else parts.push(checks.summary);
  const second = pull.state === "open" && pull.review !== "none" ? REVIEW_LABELS[pull.review] : null;
  return bar(pushing ?? parts.join(" · "), { second: pushing ? null : second, notes, main, menu, checks: pull.checks.length > 0 || pull.more_checks > 0 ? checks : null, scope: "pull" });
}

const allPassed = (p: PullStatus) => p.checks.length > 0 && p.checks.every((k) => FOLDED.has(k.state));

/** The sentences to announce for what changed between two replies. Calls nothing. */
export function announcementsOf(before: GithubReply | null, after: GithubReply): string[] {
  if (before === null || after.pull === null) return [];
  const now = after.pull;
  const out: string[] = [];
  if (before.pull === null) {
    if (before.kind === "ready" && after.kind === "ready" && before.branch === after.branch && (now.state === "open" || now.state === "draft")) out.push(c.created(now.number));
    return out;
  }
  const was = before.pull;
  if (was.number !== now.number) return [];
  if (!allPassed(was) && allPassed(now)) out.push(c.checksPassed(now.number));
  const failedBefore = new Set(was.checks.filter((k) => k.state === "failed").map((k) => k.name));
  const fresh = now.checks.find((k) => k.state === "failed" && !failedBefore.has(k.name));
  if (fresh) out.push(c.checkFailed(now.number, fresh.name));
  if (was.state !== "merged" && now.state === "merged") out.push(c.merged(now.number));
  return out;
}

export type BarCommand = "create" | "push" | "merge" | "open";

/**
 * What a palette command does with the bar: run the button or menu row that is its action, or say why it can't. A null bar is the
 * setting being off (the panel says something else when the setting is on but GitHub has nothing for the checkout).
 */
export function commandOf(view: BarView | null, command: BarCommand): { main: BarButton } | { menu: BarMenuItem } | { say: string } {
  if (view === null) return { say: c.pullsOff };
  const mains: Record<BarCommand, BarButtonId[]> = { create: ["create"], push: ["push"], merge: ["merge", "auto_merge"], open: [] };
  const menus: Record<BarCommand, BarMenuId[]> = { create: [], push: ["push"], merge: ["merge"], open: ["open_on_github"] };
  const main = view.main !== null && mains[command].includes(view.main.id) ? view.main : null;
  const menu = view.menu.find((m) => menus[command].includes(m.id)) ?? null;
  const found = main ?? menu;
  if (found !== null) return found.disabled !== null ? { say: found.disabled } : main !== null ? { main } : { menu: menu! };
  if (view.scope === "branch" && (command === "merge" || command === "open")) return { say: c.noPullHere };
  return { say: view.line };
}
