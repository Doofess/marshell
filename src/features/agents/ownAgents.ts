import type { AgentManifest } from "../../generated/AgentManifest";
import type { OwnAgentProblem } from "../../generated/OwnAgentProblem";
import type { OwnAgentsSave } from "../../generated/OwnAgentsSave";
import type { PromptStyle } from "../../generated/PromptStyle";
import { ApiError } from "../../lib/api/client";

/** The five modes a manifest can map, in the chip's order (Don't ask is only ever reported). */
export const OWN_MODES = ["manual", "plan", "auto-edit", "full-auto", "bypass"] as const;
export type OwnMode = (typeof OWN_MODES)[number];
export const OWN_MODE_LABELS: Record<OwnMode, string> = { manual: "Manual", plan: "Plan", "auto-edit": "Auto-edit", "full-auto": "Full auto", bypass: "Bypass" };

export const CONFLICT_TEXT = "agents.json changed since you opened it. Reload to see the change.";

/** What the person types: text fields stay text until they are built into a manifest. */
export type OwnForm = {
  name: string;
  command: string;
  prompt: "none" | "positional" | "flag";
  promptFlag: string;
  continueFlags: string;
  modes: Record<OwnMode, string>;
  /** Manual with an empty flag list: the CLI's own default already asks before it runs tools. */
  manualNoFlags: boolean;
  exeNames: string;
};

export const blankForm = (): OwnForm => ({
  name: "",
  command: "",
  prompt: "none",
  promptFlag: "",
  continueFlags: "",
  modes: { manual: "", plan: "", "auto-edit": "", "full-auto": "", bypass: "" },
  manualNoFlags: false,
  exeNames: "",
});

/** Lowercase, runs of other characters become one dash, trimmed, at most 32 characters. */
export function deriveId(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+/, "")
    .slice(0, 32)
    .replace(/-+$/, "");
}

/** Splits on whitespace; a run in double or single quotes stays one flag, without its quotes. */
export function splitFlags(text: string): string[] {
  const out: string[] = [];
  const re = /"([^"]*)"|'([^']*)'|(\S+)/g;
  for (const m of text.matchAll(re)) out.push(m[1] ?? m[2] ?? m[3] ?? "");
  return out;
}

/** The inverse of `splitFlags`: a flag with a space (or none at all) is quoted. */
export const joinFlags = (flags: readonly string[]): string => flags.map((f) => (f === "" || /\s/.test(f) ? `"${f}"` : f)).join(" ");

/** Program names split on commas or spaces. */
export const splitNames = (text: string): string[] => text.split(/[,\s]+/).filter(Boolean);

export function buildManifest(form: OwnForm): AgentManifest {
  const name = form.name.trim();
  const prompt: PromptStyle = form.prompt === "flag" ? { style: "flag", flag: form.promptFlag.trim() } : { style: form.prompt };
  const cont = splitFlags(form.continueFlags);
  const modes: AgentManifest["modes"] = {};
  for (const mode of OWN_MODES) {
    const flags = splitFlags(form.modes[mode]);
    if (flags.length > 0) modes[mode] = flags;
    else if (mode === "manual" && form.manualNoFlags) modes.manual = [];
  }
  return {
    id: deriveId(name),
    name,
    cli: name,
    mark: "generic",
    commands: [form.command.trim()],
    reject_paths: [],
    version_shape: null,
    exe_names: splitNames(form.exeNames),
    packages: [],
    prompt,
    continue: cont.length > 0 ? cont : null,
    modes,
  };
}

export function formFromManifest(m: AgentManifest): OwnForm {
  const form = blankForm();
  form.name = m.name;
  form.command = m.commands[0] ?? "";
  form.prompt = m.prompt.style;
  form.promptFlag = m.prompt.style === "flag" ? m.prompt.flag : "";
  form.continueFlags = joinFlags(m.continue ?? []);
  for (const mode of OWN_MODES) form.modes[mode] = joinFlags(m.modes[mode] ?? []);
  form.manualNoFlags = m.modes.manual !== undefined && m.modes.manual.length === 0;
  form.exeNames = m.exe_names.join(", ");
  return form;
}

export type Change = { add: AgentManifest } | { replace: number; with: AgentManifest } | { remove: number };

/** The list to save: the one read, with one entry added, replaced or removed, and the version it was read at. */
export function buildSave(agents: readonly AgentManifest[], version: string, change: Change): OwnAgentsSave {
  if ("add" in change) return { agents: [...agents, change.add], version };
  if ("replace" in change) return { agents: agents.map((a, i) => (i === change.replace ? change.with : a)), version };
  return { agents: agents.filter((_, i) => i !== change.remove), version };
}

export type SaveOutcome =
  | { kind: "conflict"; text: string }
  | { kind: "refused"; problems: OwnAgentProblem[] }
  | { kind: "failed"; text: string };

/** What a failed save means: a 409 asks for a reload, a 422 carries the core's reasons, anything else is a plain failure. */
export function outcomeOf(e: unknown): SaveOutcome {
  if (e instanceof ApiError) {
    if (e.status === 409) return { kind: "conflict", text: CONFLICT_TEXT };
    if (e.status === 422) {
      try {
        const body: unknown = JSON.parse(e.message);
        if (Array.isArray(body)) return { kind: "refused", problems: body as OwnAgentProblem[] };
      } catch {
        // Not JSON: falls through to the plain failure.
      }
    }
  }
  return { kind: "failed", text: "Couldn't save agents.json. Try again." };
}

/** Problems that belong to the file, not to one entry: the file couldn't be read, so no entry exists at their index. */
export const fileProblems = (agents: readonly AgentManifest[], problems: readonly OwnAgentProblem[]): OwnAgentProblem[] => problems.filter((p) => p.index >= agents.length);

/**
 * An edit of a listed entry: the form's fields on top of the entry, so what the form does not show (a second command, packages,
 * reject paths, the version shape, the cli name, a mode it does not offer) is kept. Only the first command is the form's.
 */
export function mergeManifest(existing: AgentManifest, form: OwnForm): AgentManifest {
  const built = buildManifest(form);
  const command = built.commands[0] ?? "";
  const modes: AgentManifest["modes"] = { ...built.modes };
  for (const [mode, flags] of Object.entries(existing.modes)) {
    if (!(OWN_MODES as readonly string[]).includes(mode)) Object.assign(modes, { [mode]: flags });
  }
  return {
    ...existing,
    id: built.id,
    name: built.name,
    commands: command === (existing.commands[0] ?? "") ? existing.commands : [command, ...existing.commands.slice(1)],
    exe_names: built.exe_names,
    prompt: built.prompt,
    continue: built.continue,
    modes,
  };
}

/**
 * Sorts a refusal: the problem of the entry being saved (it belongs on the form) from the others (they are in the file already,
 * and are said once, naming the entry). `sentIndex` is the saved entry's place in `sent`, or null when none is (a removal).
 */
export function splitProblems(sent: readonly AgentManifest[], problems: readonly OwnAgentProblem[], sentIndex: number | null): { mine: OwnAgentProblem | undefined; others: string[] } {
  const mine = sentIndex === null ? undefined : problems.find((p) => p.index === sentIndex);
  const others = problems.filter((p) => p !== mine).map((p) => `Fix or remove “${sent[p.index]?.name || p.id || `entry ${p.index + 1}`}” first: ${p.reason}`);
  return { mine, others };
}
