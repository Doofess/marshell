import type { AgentInstall } from "../../generated/AgentInstall";
import type { AgentSettingsView } from "../../generated/AgentSettingsView";
import type { CompanyAgent } from "../settings/settingsSections";

export const versionLine = (i: AgentInstall) => `Version ${i.version ?? "–"}`;

export const newerLine = (i: AgentInstall) => (i.newer_version ? `Version ${i.newer_version} is available.` : null);

/** The commands of the manual-update sentence, apart so the markup can set them in `<code>`. */
export const MANUAL_UPDATE: Record<CompanyAgent, { update: string; npm: string | null }> = {
  claude: { update: "claude update", npm: "npm install -g @anthropic-ai/claude-code@latest" },
  codex: { update: "codex update", npm: "npm install -g @openai/codex@latest" },
  antigravity: { update: "agy update", npm: null },
};

/** What to say when the install type is unknown and so there is no Update button. */
export function manualUpdateLine(agent: CompanyAgent, name: string): string {
  const m = MANUAL_UPDATE[agent];
  return `Marshell can't tell how ${name} was installed. Update it with ${m.update}${m.npm ? `, or ${m.npm} for an npm install` : ""}.`;
}

export function runningNote(name: string, count: number): string | null {
  if (count <= 0) return null;
  return count === 1 ? `${name} is running in 1 tab. The update may fail while it runs.` : `${name} is running in ${count} tabs. The update may fail while they run.`;
}

export function fileLine(view: AgentSettingsView): string | null {
  switch (view.state.kind) {
    case "missing":
      return `${view.path} doesn't exist yet, so every setting has its default.`;
    case "unparsable":
      return `${view.path} doesn't parse (line ${view.state.line}), so Marshell shows nothing from it.`;
    case "unreadable":
      return `Marshell can't read ${view.path}.`;
    default:
      return null;
  }
}
