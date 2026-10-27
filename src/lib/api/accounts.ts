import type { ApplyResult } from "../../generated/ApplyResult";
import type { SettingsPlan } from "../../generated/SettingsPlan";
import type { SettingsPlanRequest } from "../../generated/SettingsPlanRequest";
import type { SettingsUndoRequest } from "../../generated/SettingsUndoRequest";
import type { AccountStatus } from "../../generated/AccountStatus";
import type { AgentInstall } from "../../generated/AgentInstall";
import type { AgentSettingsView } from "../../generated/AgentSettingsView";
import type { Endpoint } from "../../generated/Endpoint";
import { api } from "./client";
import type { HookAgent } from "./agents";

const base = (agent: string) => `/v1/agents/${encodeURIComponent(agent)}`;

/** One agent CLI's install: found, its version, how it was installed and the command that updates it. `recheck` reads it again instead of the core's cache. */
export const getInstall = (ep: Endpoint, agent: HookAgent, recheck = false) => api<AgentInstall>(ep, "GET", `${base(agent)}/install?recheck=${recheck}`);

/** Whether the CLI is signed in, by its own status command. */
export const getAccount = (ep: Endpoint, agent: HookAgent) => api<AccountStatus>(ep, "GET", `${base(agent)}/account`);

/** The CLI's user settings for a read-only view, with secrets masked. */
export const getSettingsView = (ep: Endpoint, agent: HookAgent) => api<AgentSettingsView>(ep, "GET", `${base(agent)}/settings-view`);

/** Plans a settings change against the file as the view showed it. A 409 ApiError means the file changed since; a 422 carries the core's sentence as its message. */
export const planSettings = (ep: Endpoint, agent: "claude" | "codex" | "antigravity", req: SettingsPlanRequest) => api<SettingsPlan>(ep, "POST", `${base(agent)}/settings/plan`, req);

/** Puts the settings file back as a snapshot had it. */
export const undoSettings = (ep: Endpoint, agent: "claude" | "codex" | "antigravity", snapshotId: string) =>
  api<ApplyResult>(ep, "POST", `${base(agent)}/settings/undo`, { snapshot_id: snapshotId } satisfies SettingsUndoRequest);
