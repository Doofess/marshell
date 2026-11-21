import type { HookInstall } from "../../generated/HookInstall";
import type { HookAgent } from "../../lib/api/agents";
import { agentLabel } from "../sessions/sessionActions";

/** What a session's agent does when its hooks are in. */
export const AUTO_OPEN_OK = "Opens its HTML files";

type Hooks = Partial<Record<HookAgent, HookInstall>>;

const HOOK_AGENTS: readonly string[] = ["claude", "codex", "antigravity", "qwen", "copilot"];
const isHookAgent = (agent: string): agent is HookAgent => HOOK_AGENTS.includes(agent);

/** Why the session's agent can't have its HTML files opened; null when it can, and for a session with no agent. */
export function autoOpenNote(agent: string | null, hooks: Hooks): string | null {
  if (agent === null) return null;
  if (!isHookAgent(agent)) return `Not in this session: ${agentLabel(agent)} reports no file writes.`;
  return hooks[agent] === "installed" ? null : `Not in this session: ${agentLabel(agent)}'s hooks aren't set up.`;
}

/** One row per CLI found: its name and what it does about its HTML files. */
export function autoOpenList(found: readonly string[], hooks: Hooks): { agent: string; label: string; text: string }[] {
  return found.map((agent) => ({
    agent,
    label: agentLabel(agent),
    text: !isHookAgent(agent) ? "Reports no file writes" : hooks[agent] === "installed" ? AUTO_OPEN_OK : "Its hooks aren't set up",
  }));
}
