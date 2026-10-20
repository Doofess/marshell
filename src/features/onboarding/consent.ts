import type { Capability, CliId, Detection } from "./types";
export * from "./types";

export const CLI_NAME: Record<CliId, string> = { claude: "Claude Code", codex: "Codex", antigravity: "Antigravity CLI" };

const no = (reason: string) => ({ available: false, reason, entries: 0 });

/**
 * What each CLI can do and how many hook entries each capability adds (docs/PLAN.md section 2: the adapter's capabilities).
 * Antigravity (checked against agy 1.3.3 and its bundled hooks doc, docs/research/2026-10-15-local-install-check.md): status is
 * PostToolUse, PreInvocation, PostInvocation and Stop; approval is PreToolUse, whose decision can allow, deny or ask; the status
 * line is `statusLine` in ~/.gemini/antigravity-cli/settings.json.
 */
export const CAPABILITIES: Record<CliId, Record<Capability, { available: boolean; reason?: string; entries: number }>> = {
  claude: { status: { available: true, entries: 9 }, approve: { available: true, entries: 1 }, statusline: { available: true, entries: 0 } },
  codex: { status: { available: true, entries: 6 }, approve: { available: true, entries: 1 }, statusline: no("Codex has no status line to chain.") },
  antigravity: { status: { available: true, entries: 4 }, approve: { available: true, entries: 1 }, statusline: { available: true, entries: 0 } },
};

export function entryCount(cli: CliId, on: Record<Capability, boolean>): number {
  return (Object.keys(CAPABILITIES[cli]) as Capability[]).reduce((n, c) => n + (on[c] && CAPABILITIES[cli][c].available ? CAPABILITIES[cli][c].entries : 0), 0);
}

/** Fails closed: a file that does not parse, or cannot be written, is never offered for editing. A managed file does not stop the user-level edit. */
/** The last part of a path, with either slash (Windows, macOS, Linux). */
export const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;
/** The folder part of a path, with either slash. */
export const folderOf = (path: string) => path.slice(0, Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/")));

export function canEdit(d: Detection): { ok: boolean; reason?: string } {
  if (d.configState === "unparsable") return { ok: false, reason: "The file does not parse." };
  if (d.configState === "read-only") return { ok: false, reason: "The file is read-only." };
  return { ok: true };
}

/**
 * What turning Marshell's hooks off, or uninstalling, does to this CLI's file, given the switches: hook entries it added are
 * removed, and a chained status line is put back from the chain file (its command was replaced, not added, so "removes only
 * what it added" would not be true of it). Nothing else in the file is touched. When the hooks go in a file of their own that
 * Marshell creates, removal deletes that file too, but only if nothing else was put in it since.
 */
export function removalCopy(cli: CliId, on: Record<Capability, boolean>, hooks?: { hooksFile: string; created: boolean }, handedOver?: string, addedStatusLine = false): string {
  const entries = entryCount(cli, on) > 0;
  const chain = on.statusline && CAPABILITIES[cli].statusline.available && !addedStatusLine;
  const parts = [
    entries && (addedStatusLine ? "removes the hook entries and the status line it added" : "removes the hook entries it added"),
    !entries && addedStatusLine && "removes the status line it added",
    entries && hooks?.created && `deletes ${hooks.hooksFile} if nothing else is in it`,
    chain && "puts your own status line back from statusline-chain.json",
    handedOver && `turns ${handedOver} back on`,
  ].filter((p): p is string => typeof p === "string");
  if (parts.length === 0) return "Marshell changes nothing, so there is nothing to undo.";
  const list = parts.length > 2 ? `${parts.slice(0, -1).join(", ")}, and ${parts.at(-1)}` : parts.join(" and ");
  const twoFiles = entries && (chain || Boolean(handedOver)) && hooks !== undefined;
  return `Marshell ${list}; the rest of ${twoFiles ? "each" : "the"} file stays as it is.`;
}

const sentence = (text: string) => `${text.charAt(0).toUpperCase()}${text.slice(1)}.`;

/** Status is on and the user has no status line: install adds one (so context and plan data arrive) instead of chaining theirs. */
export const addsStatusLine = (d: Detection, on: Record<Capability, boolean>) => d.hasOwnStatusLine === false && on.status && CAPABILITIES[d.cli].statusline.available;

export function consentHeadline(d: Detection, on: Record<Capability, boolean>, handOver = false): string {
  if (d.configState === "unparsable") return `${d.configPath} does not parse (line ${d.parseError?.line ?? "–"}), so Marshell will not edit it.`;
  if (d.configState === "read-only") return `${d.configPath} is read-only, so Marshell will not edit it.`;
  const n = entryCount(d.cli, on);
  const added = addsStatusLine(d, on);
  const chain = on.statusline && CAPABILITIES[d.cli].statusline.available && d.hasOwnStatusLine !== false;
  const turnOff = handOver && d.skipsAll ? `turn off ${d.skipsAll.setting}` : undefined;
  if (n === 0 && !chain && !turnOff && !added) return `Change nothing. ${CLI_NAME[d.cli]} will show limited status.`;
  const add = `Add ${n} hook ${n === 1 ? "entry" : "entries"}`;
  if (turnOff) {
    // The settings file gets two kinds of change: say the file once, then what happens in it.
    const inSettings = chain ? `In ${d.configPath}, chain your status line and ${turnOff}.` : sentence(`${turnOff} in ${d.configPath}`);
    if (n === 0) return inSettings;
    const where = d.hooks ? `${d.hooks.path}${d.hooks.exists ? "" : " (a new file)"}` : d.configPath;
    return `${add} in ${where}. ${inSettings}`;
  }
  if (d.hooks) {
    // Two files: each named with what goes in it.
    const text = [n > 0 && `${add} in ${d.hooks.path}${d.hooks.exists ? "" : " (a new file)"}`, chain && `chain your status line in ${d.configPath}`]
      .filter((p): p is string => typeof p === "string")
      .join(" and ");
    return sentence(text);
  }
  const text = chain ? (n > 0 ? `${add} and chain your status line` : "Chain your status line") : added ? `${add} and a status line` : add;
  return `${text} in ${d.configPath}.`;
}
