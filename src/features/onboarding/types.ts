/** The CLIs with an adapter planned: Claude Code, Codex and Google's Antigravity (`agy`). Gemini CLI is dropped (docs/PLAN.md). */
export type CliId = "claude" | "codex" | "antigravity";
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
  /** Where the hook entries go when that is not `configPath` (Antigravity reads its hooks from ~/.gemini/config/hooks.json), and whether the file exists yet. */
  hooks?: { path: string; exists: boolean };
  /** A setting in `configPath` that skips every permission prompt (Antigravity's `dangerouslySkipPermissions: true`); while it is on, no mode below Bypass can work. */
  skipsAll?: { setting: string };
  /** The user has a status line of their own. `false` means there is none, so a status line is added rather than chained; unset means unknown. */
  hasOwnStatusLine?: boolean;
  /** The state of the marshell skill file and where it goes. Unset means unknown, which offers nothing. */
  skill?: "none" | "installed" | "outdated" | "edited";
  skillPath?: string;
  /** Marshell runs this CLI as a terminal session but has no adapter for it yet, so it has no settings to show or change here. */
  later?: boolean;
};

export type Capability = "status" | "approve" | "statusline" | "skill";
export type Availability = { available: boolean; reason?: string };

/** What the owner agrees to for one CLI: the switches, the exact diff, the backup and the way out. */
export type ConsentPlan = {
  cli: CliId;
  on: Record<Capability, boolean>;
  configPath: string;
  backupPath: string;
  removal: string;
  diff: string;
  /** Paths of files the change deletes (a preview with no `after_hash` and status Changes); the card names each as deleted above the diff. */
  deleted?: string[];
  needsTrust: boolean;
  /** Where the user's own status line command is kept when it is chained (shown on the card and in the diff). */
  chainPath?: string;
  /** Turn the detection's `skipsAll` setting off, so Marshell's session mode decides (docs/PLAN.md "Session modes"). */
  handOver?: boolean;
  /** Replaces the computed headline; only a stand-in change made long on purpose says so here. */
  headline?: string;
};
