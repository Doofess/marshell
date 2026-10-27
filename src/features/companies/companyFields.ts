import type { CompanyAgent } from "../settings/settingsSections";

export type CompanyField = { key: string; label: string; help: string };

/** The settings each company section names, in the order the core reads them (`fn curated` in crates/core/src/agents/settings_view.rs). */
export const COMPANY_FIELDS: Record<CompanyAgent, CompanyField[]> = {
  claude: [
    { key: "model", label: "Default model", help: "The model a new Claude Code session starts with." },
    { key: "effortLevel", label: "Effort", help: "How hard Claude Code thinks by default." },
    { key: "permissions.defaultMode", label: "Default permission mode", help: "How Claude Code asks before it acts." },
    { key: "cleanupPeriodDays", label: "Delete transcripts after (days)", help: "Claude deletes its own transcripts after this long. Marshell keeps its own copy." },
    { key: "autoUpdatesChannel", label: "Update channel", help: "Which releases Claude Code updates itself to." },
    { key: "theme", label: "Theme", help: "Claude Code's own colours in the terminal." },
  ],
  codex: [
    { key: "model", label: "Default model", help: "The model a new Codex session starts with." },
    { key: "model_reasoning_effort", label: "Reasoning effort", help: "How hard Codex reasons by default." },
    { key: "approval_policy", label: "Approval policy", help: "When Codex asks before it runs a command." },
    { key: "sandbox_mode", label: "Sandbox", help: "What Codex's commands may touch." },
    { key: "project_doc_max_bytes", label: "AGENTS.md size limit (bytes)", help: "Codex reads instruction files up to this size." },
    { key: "check_for_update_on_startup", label: "Check for updates at start", help: "Whether Codex looks for a new version when it starts." },
  ],
  antigravity: [
    { key: "model", label: "Default model", help: "The model a new Antigravity session starts with, by the name /model shows, such as Gemini 3.1 Pro." },
    { key: "toolPermission", label: "Tool permission", help: "When Antigravity asks before it uses a tool." },
    { key: "enableTerminalSandbox", label: "Terminal sandbox", help: "Whether Antigravity's commands run in a sandbox." },
    { key: "colorScheme", label: "Colour scheme", help: "Antigravity's own colours in the terminal." },
    { key: "verbosity", label: "Verbosity", help: "How much Antigravity says as it works." },
    { key: "enableTelemetry", label: "Telemetry", help: "Whether Antigravity sends usage data to Google." },
  ],
};

/** Labels for choice values whose raw spelling reads badly; any other value shows capitalised. */
export const CHOICE_LABELS: Record<string, string> = {
  acceptEdits: "Accept edits",
  bypassPermissions: "Bypass permissions",
  dontAsk: "Don't ask",
  "on-request": "On request",
  "read-only": "Read-only",
  "workspace-write": "Workspace write",
  "danger-full-access": "Full access",
  "dark-daltonized": "Dark, colour-blind friendly",
  "light-daltonized": "Light, colour-blind friendly",
  "dark-ansi": "Dark, terminal colours",
  "light-ansi": "Light, terminal colours",
  xhigh: "Extra high",
};
