import type { Capability, CliId, Detection } from "./types";
export * from "./types";

export const CLI_NAME: Record<CliId, string> = { claude: "Claude Code", codex: "Codex", antigravity: "Antigravity CLI" };

const no = (reason: string) => ({ available: false, reason, entries: 0 });

/**
 * What each CLI can do and how many hook entries each capability adds (docs/PLAN.md section 2: the adapter's capabilities).
 * Antigravity: only its status line is verified (`statusLine` in ~/.gemini/antigravity-cli/settings.json pushes model, context,
 * quota and whether a tool waits for confirmation). Its hook events and any permission hook are verified when the adapter is
 * built, so the card says so instead of promising entries it cannot count (docs/research/2026-10-10-cli-data-sources.md).
 */
export const CAPABILITIES: Record<CliId, Record<Capability, { available: boolean; reason?: string; entries: number }>> = {
  claude: { status: { available: true, entries: 8 }, approve: { available: true, entries: 1 }, statusline: { available: true, entries: 0 } },
  codex: { status: { available: true, entries: 6 }, approve: { available: true, entries: 1 }, statusline: no("Codex has no status line to chain.") },
  antigravity: {
    status: no("Antigravity's status and context come through its status line (the switch below). Its hook events are verified when the adapter is built."),
    approve: no("Antigravity has no verified approval hook, so Marshell focuses the tab instead (verified when the adapter is built)."),
    statusline: { available: true, entries: 0 },
  },
};

export function entryCount(cli: CliId, on: Record<Capability, boolean>): number {
  return (Object.keys(CAPABILITIES[cli]) as Capability[]).reduce((n, c) => n + (on[c] && CAPABILITIES[cli][c].available ? CAPABILITIES[cli][c].entries : 0), 0);
}

/** Fails closed: a file that does not parse, or cannot be written, is never offered for editing. A managed file does not stop the user-level edit. */
export function canEdit(d: Detection): { ok: boolean; reason?: string } {
  if (d.configState === "unparsable") return { ok: false, reason: "The file does not parse." };
  if (d.configState === "read-only") return { ok: false, reason: "The file is read-only." };
  return { ok: true };
}

/**
 * What turning Marshell's hooks off, or uninstalling, does to this CLI's file, given the switches: hook entries it added are
 * removed, and a chained status line is put back from the chain file (its command was replaced, not added, so "removes only
 * what it added" would not be true of it). Nothing else in the file is touched.
 */
export function removalCopy(cli: CliId, on: Record<Capability, boolean>): string {
  const parts = [
    entryCount(cli, on) > 0 && "removes the hook entries it added",
    on.statusline && CAPABILITIES[cli].statusline.available && "puts your own status line back from statusline-chain.json",
  ].filter((p): p is string => typeof p === "string");
  if (parts.length === 0) return "Marshell changes nothing, so there is nothing to undo.";
  return `Marshell ${parts.join(" and ")}; the rest of the file stays as it is.`;
}

export function consentHeadline(d: Detection, on: Record<Capability, boolean>): string {
  if (d.configState === "unparsable") return `${d.configPath} does not parse (line ${d.parseError?.line ?? "–"}), so Marshell will not edit it.`;
  if (d.configState === "read-only") return `${d.configPath} is read-only, so Marshell will not edit it.`;
  const n = entryCount(d.cli, on);
  const chain = on.statusline && CAPABILITIES[d.cli].statusline.available;
  if (n === 0 && !chain) return `Change nothing. ${CLI_NAME[d.cli]} will show limited status.`;
  const add = `Add ${n} hook ${n === 1 ? "entry" : "entries"}`;
  const text = chain ? (n > 0 ? `${add} and chain your status line` : "Chain your status line") : add;
  return `${text} in ${d.configPath}.`;
}
