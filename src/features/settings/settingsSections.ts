/** The sections of the Settings page, in the order the nav lists them. */
export type SettingsSection = "appearance" | "terminal" | "notifications" | "profiles" | "history" | "agents" | "accounts" | "claude" | "codex" | "antigravity" | "advanced";

export const SETTINGS_SECTIONS: Array<{ id: SettingsSection; label: string }> = [
  { id: "appearance", label: "Appearance" },
  { id: "terminal", label: "Terminal" },
  { id: "notifications", label: "Notifications" },
  { id: "profiles", label: "Shell profiles" },
  { id: "history", label: "History" },
  { id: "agents", label: "Agents" },
  { id: "accounts", label: "Accounts" },
  { id: "claude", label: "Claude Code" },
  { id: "codex", label: "Codex" },
  { id: "antigravity", label: "Antigravity" },
  { id: "advanced", label: "Advanced" },
];

/** The agent CLIs that have a section of their own. */
export type CompanyAgent = "claude" | "codex" | "antigravity";

const COMPANY: ReadonlySet<SettingsSection> = new Set<SettingsSection>(["claude", "codex", "antigravity"]);

/** The sections the nav lists: a company section only when its CLI was found, Accounts always. */
export function shownSections(found: ReadonlySet<CompanyAgent>): Array<{ id: SettingsSection; label: string }> {
  return SETTINGS_SECTIONS.filter((s) => !COMPANY.has(s.id) || found.has(s.id as CompanyAgent));
}
