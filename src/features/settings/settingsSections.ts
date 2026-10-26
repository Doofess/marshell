/** The sections of the Settings page, in the order the nav lists them. */
export type SettingsSection = "appearance" | "terminal" | "notifications" | "profiles" | "history" | "agents" | "advanced";

export const SETTINGS_SECTIONS: Array<{ id: SettingsSection; label: string }> = [
  { id: "appearance", label: "Appearance" },
  { id: "terminal", label: "Terminal" },
  { id: "notifications", label: "Notifications" },
  { id: "profiles", label: "Shell profiles" },
  { id: "history", label: "History" },
  { id: "agents", label: "Agents" },
  { id: "advanced", label: "Advanced" },
];
