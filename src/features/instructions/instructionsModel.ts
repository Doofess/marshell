import type { CopyFix } from "../../generated/CopyFix";
import type { CopyRow } from "../../generated/CopyRow";
import type { DocSave } from "../../generated/DocSave";
import type { DocScope } from "../../generated/DocScope";
import type { LibraryDoc } from "../../generated/LibraryDoc";
import { CLI_NAME } from "../onboarding/consent";
import type { CliId } from "../onboarding/types";

/** The CLIs in Marshell's order. */
const CLIS: CliId[] = ["claude", "codex", "antigravity", "qwen", "copilot"];
const inOrder = (clis: string[]) => [...CLIS.filter((c) => clis.includes(c)), ...clis.filter((c) => !(CLIS as string[]).includes(c))];

/** What the editor holds for one doc. */
export type Draft = {
  scope: DocScope;
  name: string;
  oldName: string | null;
  description: string;
  clis: string[];
  globsText: string;
  enabled: boolean;
  body: string;
  hash: string | null;
};

export const draftOf = (doc: LibraryDoc): Draft => ({
  scope: doc.scope,
  name: doc.name,
  oldName: doc.name,
  description: doc.description,
  clis: [...doc.clis],
  globsText: doc.globs.join("\n"),
  enabled: doc.enabled,
  body: doc.body,
  hash: doc.hash,
});

/** A new doc: a user doc is for the CLIs found on this PC, a project doc for all five (decision 10). */
export const newDraft = (scope: DocScope, found: string[]): Draft => ({
  scope,
  name: "",
  oldName: null,
  description: "",
  clis: scope === "project" ? [...CLIS] : inOrder(found),
  globsText: "",
  enabled: true,
  body: "",
  hash: null,
});

const sameList = (a: string[], b: string[]) => a.length === b.length && a.every((x, i) => x === b[i]);

/** Whether the draft differs from the doc it was made from; a new draft (no doc) is dirty once anything is typed. */
export function isDirty(d: Draft, doc: LibraryDoc | null): boolean {
  if (doc === null) return d.name.trim() !== "" || d.description.trim() !== "" || d.body.trim() !== "" || d.globsText.trim() !== "";
  const o = draftOf(doc);
  return d.name !== o.name || d.description !== o.description || !sameList(inOrder(d.clis), inOrder(o.clis)) || d.globsText !== o.globsText || d.enabled !== o.enabled || d.body !== o.body;
}

const globsOf = (text: string) =>
  text
    .split("\n")
    .map((g) => g.trim())
    .filter((g) => g !== "");

export function toSave(d: Draft, project: string | null): DocSave {
  const s: DocSave = { scope: d.scope, name: d.name, description: d.description, clis: inOrder(d.clis), globs: globsOf(d.globsText), enabled: d.enabled, body: d.body };
  if (project !== null) s.project = project;
  if (d.oldName !== null) s.old_name = d.oldName;
  if (d.hash !== null) s.expected_hash = d.hash;
  return s;
}

/** In a project doc Copilot CLI reads Claude Code's copy, so it can't be unticked while Claude Code is ticked. */
export const copilotLocked = (d: Draft): boolean => d.scope === "project" && d.clis.includes("claude") && d.clis.includes("copilot");

/** Ticks or unticks a CLI (decision 51). */
export function toggleCli(d: Draft, cli: string): Draft {
  if (cli === "copilot" && copilotLocked(d)) return d;
  if (d.clis.includes(cli)) return { ...d, clis: d.clis.filter((c) => c !== cli) };
  const next = [...d.clis, cli];
  if (cli === "claude" && d.scope === "project" && !next.includes("copilot")) next.push("copilot");
  return { ...d, clis: inOrder(next) };
}

/** The bar over the page; null when there is nothing to say. */
export function barText(outOfDate: number, changed: number): string | null {
  const out = outOfDate > 0 ? (outOfDate === 1 ? "1 copy is out of date" : `${outOfDate} copies are out of date`) : null;
  const chg = changed > 0 ? (changed === 1 ? "1 copy was changed outside Marshell; its row has the fixes." : `${changed} copies were changed outside Marshell; their rows have the fixes.`) : null;
  if (out && chg) return `${out}. ${chg}`;
  return out ?? chg;
}

const STATE: Record<CopyRow["state"], string> = {
  in_sync: "Up to date",
  out_of_date: "Out of date",
  missing: "No copy yet",
  changed_outside: "Changed outside Marshell",
  extra: "Review removes this copy",
  taken: "Not Marshell's file",
  no_copy: "No copy",
  waiting: "Waiting for the distro",
  refused: "Marshell can't write this copy",
};
export const stateText = (row: CopyRow): string => STATE[row.state];

/** The CLI's name, with " in <distro>" for a distro row; a Waiting row has only the distro. */
export function rowLabel(row: CopyRow): string {
  if (row.cli === undefined) return row.distro ?? "";
  const name = CLI_NAME[row.cli as CliId] ?? row.cli;
  return row.distro === undefined ? name : `${name} in ${row.distro}`;
}

/** Whether the row offers the two fixes; the core sets `fixes` only where it can plan them. */
export const copyFixes = (row: CopyRow, orphan: boolean): boolean => !orphan && row.fixes === true;

export function copyFixOf(doc: LibraryDoc, row: CopyRow, project: string | null, takeCopy: boolean): CopyFix {
  const f: CopyFix = { scope: doc.scope, name: doc.name, cli: row.cli ?? "", take_copy: takeCopy };
  if (project !== null) f.project = project;
  if (row.distro !== undefined) f.distro = row.distro;
  return f;
}
