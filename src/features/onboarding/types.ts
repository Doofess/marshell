export type CliId = "claude" | "codex" | "gemini";
export type ConfigState = "ok" | "missing" | "unparsable" | "read-only";

/** What detection found for one CLI. Phase 3 fills this from the core; Phase 0 uses fixtures. */
export type Detection = {
  cli: CliId;
  installed: boolean;
  version?: string;
  exePath?: string;
  configPath: string;
  configState: ConfigState;
  parseError?: { line: number; message: string };
  /** A managed or organisation settings file that applies on top of the user's; listed read-only, never edited. */
  managed?: { path: string };
};

export type Capability = "status" | "approve" | "statusline";
export type Availability = { available: boolean; reason?: string };

/** What the owner agrees to for one CLI: the switches, the exact diff, the backup and the way out. */
export type ConsentPlan = {
  cli: CliId;
  on: Record<Capability, boolean>;
  configPath: string;
  backupPath: string;
  removal: string;
  diff: string;
  needsTrust: boolean;
  /** Where the user's own status line command is kept when it is chained (shown on the card and in the diff). */
  chainPath?: string;
  /** Replaces the computed headline; only a stand-in change made long on purpose says so here. */
  headline?: string;
};
