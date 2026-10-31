import type { AgentSetup } from "../../generated/AgentSetup";
import type { ConfigPreview } from "../../generated/ConfigPreview";
import type { HookPlanRequest } from "../../generated/HookPlanRequest";
import type { Profile } from "../../generated/Profile";
import { deletedFiles, previewDiff, toDetection, type HookAgent } from "../../lib/api/agents";
import { addsStatusLine, CLI_NAME, consentHeadline, fileName, opensDiff, removalCopy, skillUndo, type Capability, type CliId, type ConsentPlan, type Detection } from "../onboarding/consent";

/** The capabilities offered for a new Claude Code setup. */
export const OFFER: Capability[] = ["status", "approve", "statusline", "skill"];
/** Codex has no status line, so its card has no status line controls. */
const CODEX_OFFER: Capability[] = ["status", "approve", "skill"];
export type Want = HookPlanRequest;
export const DEFAULT_WANT: Want = { status: true, statusline: true, approve: true, skill: true };
/** Approval can be added on its own; chaining still needs status. */
export const installRequest = (w: Want, agent: HookAgent = "claude", distro?: string): HookPlanRequest => ({
  status: w.status,
  statusline: agent !== "codex" && w.status && w.statusline,
  approve: w.approve,
  // The core installs no skill file inside a distro.
  skill: distro === undefined && w.skill,
  ...(distro === undefined ? {} : { distro }),
});
const onOf = (w: Want, agent: HookAgent = "claude", distro?: string) => installRequest(w, agent, distro);
const ON = onOf(DEFAULT_WANT);

/** An existing status install needs only the permission hook. */
export const setupWant = (s: AgentSetup): Want => ({ ...(s.hooks === "installed" ? { status: false, statusline: false, approve: true } : DEFAULT_WANT), skill: s.skill !== "installed" && s.skill !== "unavailable" });

/**
 * The WSL distros the session profiles name, once each. The name is the profile's `-d` / `--distribution` argument in `command`
 * (what the launch passes to wsl.exe), else the profile's own name (which detection sets to the distro). No distro is asked for.
 */
export function wslDistros(profiles: Pick<Profile, "name" | "command" | "family">[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const p of profiles) {
    if (p.family !== "wsl") continue;
    const at = p.command.findIndex((a) => a === "-d" || a === "--distribution");
    const fromFlag = at >= 0 ? p.command[at + 1] : p.command.find((a) => a.startsWith("--distribution="))?.slice("--distribution=".length);
    const name = fromFlag?.trim() || p.name.trim();
    if (name === "" || seen.has(name.toLowerCase())) continue;
    seen.add(name.toLowerCase());
    out.push(name);
  }
  return out;
}

/** Where the Agents dialog opens from a session row's "set up" link: on that row's agent's Set up card. */
export type SetupStart = { setup: HookAgent };
export const rowSetupStart = (agent: string): SetupStart => ({ setup: agent === "codex" || agent === "antigravity" ? agent : "claude" });

/** Whether an agent's live status is on: it is unless the agent is in the settings' `status_off`. */
export const liveStatusOn = (statusOff: string[], agent: string): boolean => !statusOff.includes(agent);

/** The setup's file does not parse: there is nothing to set up or turn off until it is fixed. */
const refused = (s: AgentSetup) => s.settings_state === "unparsable";

/** The agents whose card the first run offers, in order: each found one that has something to set up. One card each, never a bulk allow. */
export const consentQueue = (setups: AgentSetup[]): AgentSetup[] => setups.filter((s) => actions(s).includes("setup"));

/** One line on what Marshell's hooks are doing in an agent's settings file (Claude Code's settings, Codex's hooks.json). */
export function setupLine(s: AgentSetup): string {
  const name = CLI_NAME[s.agent as CliId];
  if (!s.found) return `${name} was not found on this computer.`;
  if (refused(s)) return consentHeadline(toDetection(s), ON);
  // Antigravity keeps its hooks in a file of their own.
  const file = s.hooks_path ?? s.settings_path;
  switch (s.hooks) {
    case "none":
      return "Not set up. Sessions show limited status.";
    case "installed":
      // Codex runs hooks only after the person trusts them (slice 6a decision 10), so an untrusted or changed set reports nothing yet.
      return s.agent === "codex" && (s.trust === "untrusted" || s.trust === "changed")
        ? `Hooks on in ${file}. New sessions report status once Codex trusts them.`
        : `Hooks on in ${file}. New sessions report status.`;
    case "partial":
      return `Some hooks are missing in ${file}. Set up again to repair them.`;
    case "orphaned": {
      // agy blocks a tool call whose PreToolUse command can't start (6d research, A3); the others only report it.
      const effect = s.agent === "antigravity" ? "blocks every tool call, outside Marshell too" : "reports a failing hook each time one fires";
      return `Marshell's hooks are in ${file}, but the bridge at ${s.bridge_path} is missing, so ${name} ${effect}. Turn the hooks off, or set up again.`;
    }
  }
}

/** What the person may do about it. Orphaned hooks can be turned off or set up again. */
export function actions(s: AgentSetup): Array<"setup" | "hooksOff"> {
  if (!s.found || refused(s)) return [];
  switch (s.hooks) {
    case "none":
      return ["setup"];
    case "installed":
      return s.approve_installed ? ["hooksOff"] : ["setup", "hooksOff"];
    case "partial":
    case "orphaned":
      return ["setup", "hooksOff"];
  }
}

/** The notice after the hooks are turned off. */
export const hooksOffNotice = (statuslineLeft: boolean): string =>
  `Hooks off. Sessions started from now show limited status.${statuslineLeft ? " Your status line was changed since setup, so it was left as it is." : ""}`;

/** Whether a Set up's diff starts open: an edited skill that is about to be replaced is shown, never replaced silently. */
export const startsOpen = (want: Want, skill: AgentSetup["skill"] | undefined): boolean => opensDiff(want.skill, skill);

/** Where a Set up is: the preview the core made (none when the file is refused), and what the card says about it. */
/** `distro` is set when the flow is for a WSL distro: the card then shows the core's words and no skill switch. */
export type InstallFlow = { preview: ConfigPreview | null; open: boolean; busy: boolean; error?: string; conflict?: boolean; want?: Want; distro?: string };

/** The first file the core refuses, as the sentence that says so. */
function refusal(p: ConfigPreview): string | undefined {
  for (const f of p.files) if (f.status.kind === "refused") return `Marshell will not edit ${f.path}: ${f.status.detail}`;
  return undefined;
}

/** A preview that cannot be applied and has nothing to change: the hooks are already as asked. */
const alreadyDone = (p: ConfigPreview) => !p.can_apply && p.files.every((f) => f.status.kind === "noChange");

/** Why a removal preview cannot be applied, in the core's words (a refused file named first); nothing when it can be. */
export function removalBlock(p: ConfigPreview): string | undefined {
  return p.can_apply ? undefined : (refusal(p) ?? p.reason);
}

/**
 * The setup as the card should read it for a WSL distro: nothing of the Windows files shows, the settings file is the one the preview
 * names, and there is no skill file. Whether the distro already has hooks is not read (that would boot it); the preview says what changes.
 */
function distroSetup(s: AgentSetup, flow: InstallFlow): AgentSetup {
  return {
    ...s,
    settings_path: flow.preview?.files[0]?.path ?? flow.distro ?? s.settings_path,
    hooks: "none",
    hooks_path: undefined,
    approve_installed: false,
    chained: false,
    has_own_statusline: false,
    needs_trust: false,
    skill: "unavailable",
    skill_problem: undefined,
  };
}

/** The consent card's input for the core's setup and preview. A refused file gets a headline that says nothing will change. */
export function consentProps(windows: AgentSetup, flow: InstallFlow) {
  const s = flow.distro === undefined ? windows : distroSetup(windows, flow);
  const want = flow.want ?? setupWant(s);
  const cli = s.agent as CliId;
  const on = onOf(want, cli === "codex" ? "codex" : "claude", flow.distro);
  const files = flow.preview?.files ?? [];
  const base = toDetection(s);
  // With a preview, whether hooks.json exists is what the core read: a file with a hash before it.
  const hooksFile = s.hooks_path === undefined || !flow.preview ? undefined : files.find((f) => f.path === s.hooks_path);
  const detection: Detection = base.hooks && flow.preview ? { ...base, hooks: { path: base.hooks.path, exists: hooksFile?.before_hash !== undefined } } : base;
  const chain = files.find((f) => /statusline-chain\.json$/.test(f.path));
  const unchanged = Boolean(flow.preview && alreadyDone(flow.preview));
  const installed = s.hooks !== "none" || s.approve_installed || s.chained;
  const done = unchanged && installed;
  const plan: ConsentPlan = {
    cli,
    on,
    configPath: s.settings_path,
    backupPath: s.snapshots_dir,
    removal: removalCopy(cli, on, s.hooks_path === undefined ? undefined : { hooksFile: fileName(s.hooks_path), created: true }, undefined, addsStatusLine(detection, on), skillUndo(on.skill, s.skill)),
    diff: flow.preview ? previewDiff(flow.preview) : "",
    deleted: flow.preview ? deletedFiles(flow.preview) : undefined,
    needsTrust: s.needs_trust,
    chainPath: chain?.path,
    // A Partial install's repair changes only what is missing or wrong (the diff shows which), so it isn't the whole install's count.
    headline: flow.preview ? (refusal(flow.preview) ?? (flow.distro !== undefined ? flow.preview.reason : done ? `Already set up. Nothing in ${s.settings_path} changes.` : unchanged ? "Nothing will change: status is off." : s.hooks === "partial" ? `Repair the hook entries Marshell added in ${s.settings_path}.` : undefined)) : flow.error ? `Marshell will not edit ${s.settings_path}, so nothing will change.` : `Checking ${s.settings_path}…`,
  };
  // Allow is for a preview the core can apply; `done` is the one that has nothing left to change, whose Skip becomes Done.
  const allow = Boolean(flow.preview?.can_apply) && !flow.conflict;
  return { scenario: { detection, plan }, on, offer: flow.distro !== undefined ? (cli === "codex" ? CODEX_OFFER : OFFER).filter((c) => c !== "skill") : s.hooks === "installed" ? (["approve", "skill"] as Capability[]) : cli === "codex" ? CODEX_OFFER : OFFER, open: flow.open, busy: flow.busy, error: flow.error, allow, done };
}
