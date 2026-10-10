import type { SettingField } from "./fields";

export const SETTINGS_NAV: Array<{ id: string; label: string }> = [
  { id: "general", label: "General" },
  { id: "notifications", label: "Notifications" },
  { id: "terminal", label: "Terminal" },
  { id: "claude", label: "Claude Code" },
  { id: "codex", label: "Codex" },
  { id: "gemini", label: "Gemini" },
  { id: "advanced", label: "Advanced" },
];

/** Twelve fields across four categories. Codex, Gemini and Advanced are empty in Phase 0 and show the empty state. */
export const SETTINGS_FIELDS: SettingField[] = [
  { category: "general", key: "theme", label: "Theme", help: "Follow the system, or always dark or light.", kind: "enum", options: ["follow-system", "dark", "light"], value: "follow-system", scope: "marshell", applies: "live" },
  { category: "general", key: "announce", label: "Screen reader announcements", help: "What the window says aloud. Only 'needs-you' is built for now: requests, questions, errors and the session you switch to.", kind: "enum", options: ["needs-you", "everything", "on-request"], value: "needs-you", scope: "marshell", applies: "live" },
  { category: "general", key: "startOnLogin", label: "Start Marshell when I sign in", help: "Opens the window minimised to the tray.", kind: "boolean", value: false, scope: "marshell", applies: "live" },
  { category: "notifications", key: "sounds", label: "Play sounds", help: "A soft chime when something needs you or finishes.", kind: "boolean", value: true, scope: "marshell", applies: "live" },
  { category: "notifications", key: "quietHours", label: "Quiet hours", help: "No sounds between these times, for example 22:00–07:00.", kind: "text", value: "", scope: "marshell", applies: "live" },
  { category: "notifications", key: "subagentSounds", label: "Sounds for subagents", help: "A short tick when a subagent finishes. Off by default.", kind: "boolean", value: false, scope: "marshell", applies: "live" },
  { category: "terminal", key: "fontSize", label: "Font size", help: "In pixels. The terminal uses your own mono font.", kind: "number", value: 13, scope: "marshell", applies: "live" },
  { category: "terminal", key: "terminalTheme", label: "Terminal theme", help: "Follow the app, or pin the terminal dark or light.", kind: "enum", options: ["follow-app", "dark", "light"], value: "follow-app", scope: "marshell", applies: "live" },
  { category: "terminal", key: "contrast", label: "Contrast protection", help: "Lifts dim text so it stays readable on the terminal's background.", kind: "boolean", value: true, scope: "marshell", applies: "live" },
  { category: "claude", key: "model", label: "Default model", help: "The model a new Claude Code session starts with.", kind: "enum", options: ["opus", "sonnet", "haiku"], value: "opus", scope: "user", applies: "new-sessions" },
  { category: "claude", key: "cleanupPeriodDays", label: "Delete transcripts after (days)", help: "Claude deletes its own transcripts after this long. Marshell keeps its own copy.", kind: "number", value: 30, scope: "user", applies: "new-sessions" },
  { category: "claude", key: "defaultMode", label: "Default permission mode", help: "How Claude Code asks before it acts.", kind: "enum", options: ["default", "acceptEdits", "plan"], value: "default", scope: "user", lockedBy: "managed", managedValue: "plan", applies: "restart" },
];

/** A fourteen-line Claude settings.json with the two keys the editor changes. */
export const SETTINGS_TEXT = `{
  "model": "opus",
  "cleanupPeriodDays": 30,
  "permissions": {
    "allow": [
      "Bash(npm test:*)",
      "Bash(git status)"
    ],
    "deny": [
      "Read(./.env)"
    ]
  },
  "includeCoAuthoredBy": true
}
`;

/** The same file with a missing value on line 3. */
export const SETTINGS_TEXT_BROKEN = `{
  "model": "opus",
  "cleanupPeriodDays": ,
  "permissions": {
    "allow": ["Bash(npm test:*)"]
  }
}
`;

/** A settings file whose hooks object makes a 200-line diff when nine entries are added. */
export const SETTINGS_TEXT_LONG = `{
  "model": "opus",
  "hooks": {
${Array.from({ length: 12 }, (_, i) => `    "Event${i}": [{ "hooks": [{ "type": "command", "command": "echo ${i}" }] }]`).join(",\n")}
  }
}
`;
