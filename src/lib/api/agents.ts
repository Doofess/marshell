import type { AgentSetup } from "../../generated/AgentSetup";
import type { ApplyRequest } from "../../generated/ApplyRequest";
import type { ApplyResult } from "../../generated/ApplyResult";
import type { ConfigPreview } from "../../generated/ConfigPreview";
import type { Endpoint } from "../../generated/Endpoint";
import type { RemovePreview } from "../../generated/RemovePreview";
import type { ConfigState, Detection } from "../../features/onboarding/types";
import { api, ApiError } from "./client";

const base = (agent: string) => `/v1/agents/${encodeURIComponent(agent)}`;

/** What the core knows about one agent's settings file and the hooks in it. */
export const getSetup = (ep: Endpoint, agent: "claude") => api<AgentSetup>(ep, "GET", `${base(agent)}/setup`);

/** The preview of installing the hooks (`status`) and the status line (`statusline`). A 422 ApiError says why the file is refused. */
export const planInstall = (ep: Endpoint, agent: "claude", want: { status: boolean; statusline: boolean }) =>
  api<ConfigPreview>(ep, "POST", `${base(agent)}/hooks/plan`, want);

export const planRemove = (ep: Endpoint, agent: "claude") => api<RemovePreview>(ep, "POST", `${base(agent)}/hooks/remove-plan`);

export type ApplyOutcome = { ok: true; result: ApplyResult } | { ok: false; conflict: true } | { ok: false; conflict: false; message: string };

/** Applies a preview, guarded by the hash each file had when the preview was made. A 409 means a file changed since: nothing was written. */
export async function applyPreview(ep: Endpoint, preview: ConfigPreview): Promise<ApplyOutcome> {
  const body: ApplyRequest = {
    changeset_id: preview.changeset_id,
    expected_hashes: preview.files.map((f) => (f.before_hash === undefined ? { path: f.path } : { path: f.path, hash: f.before_hash })),
  };
  try {
    return { ok: true, result: await api<ApplyResult>(ep, "POST", "/v1/config/apply", body) };
  } catch (e) {
    if (e instanceof ApiError) return e.status === 409 ? { ok: false, conflict: true } : { ok: false, conflict: false, message: e.message };
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
  };
}
