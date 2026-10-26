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
