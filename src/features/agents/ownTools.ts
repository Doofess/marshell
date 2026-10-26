import type { OwnToolProblem } from "../../generated/OwnToolProblem";
import type { OwnToolsSave } from "../../generated/OwnToolsSave";
import type { PromptKind } from "../../generated/PromptKind";
import type { ToolProfile } from "../../generated/ToolProfile";
import { ApiError } from "../../lib/api/client";
import { deriveId } from "./ownAgents";

export const CONFLICT_TEXT = "tools.json changed since you opened it. Reload to see it.";

/** The id made from a name: lowercase, other characters become one dash. */
export const slug = deriveId;

export type PresetDraft = {
  /** The preset's id; blank for a new one, which gets a slug of its name. */
  id: string;
  name: string;
  program: string;
  /** One argument per line. */
  args: string;
  /** Labels by field id. Kept when an args edit drops a placeholder, so a label comes back with it. */
  labels: Record<string, string>;
};
export type PatternDraft = { pattern: string; phrase: string; kind: PromptKind };
/** What the person types. The fields the form doesn't show are carried in `hidden`. */
export type ToolDraft = {
  name: string;
  /** Comma-separated program names. */
  commands: string;
  presets: PresetDraft[];
  needsYou: PatternDraft[];
  ports: string[];
  hidden: Pick<ToolProfile, "scripts" | "launcher" | "quiet_ports">;
};

export const blankDraft = (): ToolDraft => ({ name: "", commands: "", presets: [], needsYou: [], ports: [], hidden: { scripts: [], launcher: false, quiet_ports: [] } });
export const blankPreset = (): PresetDraft => ({ id: "", name: "", program: "", args: "", labels: {} });
export const blankPattern = (): PatternDraft => ({ pattern: "", phrase: "", kind: "text" });

/** The field ids named by whole `{id}` arguments in the args text, in order, once each. */
export function placeholders(args: string): string[] {
  const out: string[] = [];
  for (const line of args.split("\n")) {
    const m = /^\{([A-Za-z0-9_-]+)\}$/.exec(line.trim());
    if (m?.[1] && !out.includes(m[1])) out.push(m[1]);
  }
  return out;
}

const argsOf = (text: string): string[] =>
  text
    .split("\n")
    .map((l) => l.replace(/\r$/, ""))
    .filter((l) => l.trim() !== "");

export function draftOf(p: ToolProfile): ToolDraft {
  return {
    name: p.name,
    commands: p.commands.join(", "),
    presets: p.presets.map((x) => ({ id: x.id, name: x.name, program: x.program, args: x.args.join("\n"), labels: Object.fromEntries(x.fields.map((f) => [f.id, f.label])) })),
    needsYou: p.needs_you.map((n) => ({ pattern: n.pattern, phrase: n.phrase, kind: n.kind })),
    ports: p.ports.map((x) => x.pattern),
    hidden: { scripts: p.scripts, launcher: p.launcher, quiet_ports: p.quiet_ports },
  };
}

const sameDraft = (a: ToolDraft, b: ToolDraft) => JSON.stringify(a) === JSON.stringify(b);

/**
 * The profile for a draft. An edit that changed nothing returns `existing` itself, so an entry the form can't show
 * exactly (a stale field, an empty argument) goes back as it was shown.
 */
export function profileOf(d: ToolDraft, existing?: ToolProfile): ToolProfile {
  if (existing && sameDraft(d, draftOf(existing))) return existing;
  const name = d.name.trim();
  return {
    id: slug(name),
    name,
    commands: d.commands.split(",").map((c) => c.trim()).filter(Boolean),
    scripts: d.hidden.scripts,
    launcher: d.hidden.launcher,
    needs_you: d.needsYou.map((n) => ({ pattern: n.pattern, phrase: n.phrase.trim(), kind: n.kind })),
    ports: d.ports.filter((p) => p.trim() !== "").map((pattern) => ({ pattern })),
    quiet_ports: d.hidden.quiet_ports,
    presets: d.presets.map((p) => ({
      id: p.id || slug(p.name),
      name: p.name.trim(),
      program: p.program.trim(),
      args: argsOf(p.args),
      fields: placeholders(p.args).map((id) => ({ id, label: p.labels[id]?.trim() || id })),
    })),
  };
}

export type SaveOutcome = { kind: "conflict"; text: string } | { kind: "refused"; problems: OwnToolProblem[] } | { kind: "failed"; text: string };

/** What a failed save means: a 409 asks for a reload, a 422 carries the core's reasons. */
export function outcomeOf(e: unknown): SaveOutcome {
  if (e instanceof ApiError) {
    if (e.status === 409) return { kind: "conflict", text: CONFLICT_TEXT };
    if (e.status === 422) {
      try {
        const body: unknown = JSON.parse(e.message);
        if (Array.isArray(body)) return { kind: "refused", problems: body as OwnToolProblem[] };
      } catch {
        // Not JSON: a plain failure.
      }
    }
  }
  return { kind: "failed", text: "Couldn't save tools.json. Try again." };
}

export type Change = { add: ToolProfile } | { replace: number; with: ToolProfile } | { remove: number };

export function buildSave(tools: readonly ToolProfile[], version: string, change: Change): OwnToolsSave {
  if ("add" in change) return { tools: [...tools, change.add], version };
  if ("replace" in change) return { tools: tools.map((t, i) => (i === change.replace ? change.with : t)), version };
  return { tools: tools.filter((_, i) => i !== change.remove), version };
}

/** What a refusal is about, as the shape of the change: indexes are those of the list sent. */
export type ChangeKind = { add: true } | { replace: number } | { remove: number };

/**
 * Sorts a 422: problems of entries already listed go back to their place in the list shown (a removal shifts the later ones),
 * and the one about the entry being added goes on its form.
 */
export function refusalOf(problems: readonly OwnToolProblem[], change: ChangeKind, shownLength: number): { entries: OwnToolProblem[]; form: string | undefined } {
  const entries: OwnToolProblem[] = [];
  let form: string | undefined;
  for (const p of problems) {
    if ("add" in change && p.index >= shownLength) form ??= p.reason;
    else if ("remove" in change) entries.push(p.index >= change.remove ? { ...p, index: p.index + 1 } : p);
    else entries.push(p);
  }
  return { entries, form };
}

/** Esc on a question cancels it and keeps the page from also closing. */
export function escapeCancels(e: { key: string; preventDefault: () => void }, cancel: () => void): void {
  if (e.key !== "Escape") return;
  e.preventDefault();
  cancel();
}

export const savedText = (name: string): string => `Saved ${name}`;
export const removedText = (name: string): string => `${name} removed. The previous file is saved as tools.json.bak.`;
