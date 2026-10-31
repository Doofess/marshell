import type { AgentFound } from "../../generated/AgentFound";
import type { AgentSetup } from "../../generated/AgentSetup";
import type { ApplyRequest } from "../../generated/ApplyRequest";
import type { ApplyResult } from "../../generated/ApplyResult";
import type { ConfigPreview } from "../../generated/ConfigPreview";
import type { Endpoint } from "../../generated/Endpoint";
import type { HookPlanRequest } from "../../generated/HookPlanRequest";
import type { RemovePreview } from "../../generated/RemovePreview";
import type { ConfigState, Detection } from "../../features/onboarding/types";
import { api, ApiError } from "./client";

/** The agents whose hooks Marshell sets up through `/v1/agents/{agent}/...`. */
export type HookAgent = "claude" | "codex" | "antigravity";

const base = (agent: string) => `/v1/agents/${encodeURIComponent(agent)}`;

/** What the core knows about one agent's settings file and the hooks in it. */
export const getSetup = (ep: Endpoint, agent: HookAgent) => api<AgentSetup>(ep, "GET", `${base(agent)}/setup`);

/** Which agent CLIs are on this machine: Claude, Codex and Antigravity, in that order. */
export const listAgents = (ep: Endpoint) => api<AgentFound[]>(ep, "GET", "/v1/agents");

/** The preview of the requested status, approval and status line hooks. A 422 ApiError says why the file is refused. */
export const planInstall = (ep: Endpoint, agent: HookAgent, want: HookPlanRequest) =>
  api<ConfigPreview>(ep, "POST", `${base(agent)}/hooks/plan`, want);

/** The preview of installing or updating the marshell skill file (an edited file shows its diff). */
export const planSkill = (ep: Endpoint, agent: HookAgent) => api<ConfigPreview>(ep, "POST", `${base(agent)}/skill/plan`);

/** The preview of removing the marshell skill file. */
export const planSkillRemove = (ep: Endpoint, agent: HookAgent) => api<ConfigPreview>(ep, "POST", `${base(agent)}/skill/remove-plan`);

/** The preview of turning the hooks off; with `distro`, the ones inside that WSL distro (a 400 or 422 ApiError says why not). */
export const planRemove = (ep: Endpoint, agent: HookAgent, distro?: string) =>
  api<RemovePreview>(ep, "POST", `${base(agent)}/hooks/remove-plan${distro === undefined ? "" : `?distro=${encodeURIComponent(distro)}`}`);

/** The 409 body's reason and the files it is about. A body that is not that JSON is read as a plain conflict. */
function conflictBody(text: string): { reason?: string; paths: string[] } {
  try {
    const b = JSON.parse(text) as { reason?: unknown; paths?: unknown };
    return { reason: typeof b.reason === "string" ? b.reason : undefined, paths: Array.isArray(b.paths) ? b.paths.filter((p): p is string => typeof p === "string") : [] };
  } catch {
    return { paths: [] };
  }
}

export type ApplyOutcome = { ok: true; result: ApplyResult } | { ok: false; conflict: true } | { ok: false; conflict: false; message: string };

/**
 * Applies a preview, guarded by the hash each file had when the preview was made. A 409 means nothing was written: a file changed since
 * (a conflict), or the core refuses a file (not a conflict, Reload cannot help). A 404 is a preview that expired, which Reload re-plans.
 */
export async function applyPreview(ep: Endpoint, preview: ConfigPreview): Promise<ApplyOutcome> {
  const body: ApplyRequest = {
    changeset_id: preview.changeset_id,
    expected_hashes: preview.files.map((f) => (f.before_hash === undefined ? { path: f.path } : { path: f.path, hash: f.before_hash })),
  };
  try {
    return { ok: true, result: await api<ApplyResult>(ep, "POST", "/v1/config/apply", body) };
  } catch (e) {
    if (e instanceof ApiError) {
      if (e.status === 404) return { ok: false, conflict: true };
      if (e.status !== 409) return { ok: false, conflict: false, message: e.message };
      const { reason, paths } = conflictBody(e.message);
      if (reason !== "refused") return { ok: false, conflict: true };
      return { ok: false, conflict: false, message: paths.length > 0 ? `Marshell can't change ${paths.join(", ")}, so nothing was written.` : "Nothing was written: there is nothing Marshell can change here." };
    }
    throw e;
  }
}

/** The files' diffs in order, one after the other; an empty diff (a refused file, or no change) adds nothing. */
export function previewDiff(p: ConfigPreview): string {
  return p.files.reduce((out, f) => {
    if (f.diff === "") return out;
    return out === "" || out.endsWith("\n") ? out + f.diff : `${out}\n${f.diff}`;
  }, "");
}

/** The files the preview deletes: changed, with no hash after. */
export function deletedFiles(p: ConfigPreview): string[] {
  return p.files.filter((f) => f.status.kind === "changes" && f.after_hash === undefined).map((f) => f.path);
}

const STATE: Record<AgentSetup["settings_state"], ConfigState> = { ok: "ok", missing: "missing", unparsable: "unparsable", readOnly: "read-only" };

/** The core's setup as the onboarding's `Detection`. `managed` and `skipsAll` have no source in `AgentSetup`, so they stay unset. */
export function toDetection(s: AgentSetup): Detection {
  return {
    cli: s.agent as Detection["cli"],
    installed: s.found,
    version: s.version,
    exePath: s.exe_path,
    configPath: s.settings_path,
    configState: STATE[s.settings_state],
    parseError: s.parse_error ? { line: s.parse_error.line, message: s.parse_error.message } : undefined,
    hasOwnStatusLine: s.has_own_statusline,
    skill: s.skill,
    skillPath: s.skill_path,
    skillProblem: s.skill_problem,
    // Antigravity's hooks go in a file of their own; without a preview, hooks.json is taken as there once Marshell's hooks are.
    ...(s.hooks_path === undefined ? {} : { hooks: { path: s.hooks_path, exists: s.hooks !== "none" } }),
  };
}

/**
 * Every row of the first-run screen: Claude's from its setup, Codex's and Antigravity's from their own setups when there are some, then the
 * rest from the found list, which gives no version and no settings and so stays a "later" row. `found` null (the list could not be read)
 * leaves Claude's row alone, and the others' too unless their setups were read. Claude's and unknown entries in `found` are ignored.
 */
export function toDetections(setup: AgentSetup, found: AgentFound[] | null, codex?: AgentSetup | null, antigravity?: AgentSetup | null): Detection[] {
  const later = (["codex", "antigravity"] as const).flatMap((cli): Detection[] => {
    if (cli === "codex" && codex) return [toDetection(codex)];
    if (cli === "antigravity" && antigravity) return [toDetection(antigravity)];
    const f = found?.find((x) => x.agent === cli);
    return f ? [{ cli, installed: f.found, exePath: f.exe_path, configPath: "", configState: "ok", later: true }] : [];
  });
  return [toDetection(setup), ...later];
}
