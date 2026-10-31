import type { Alias } from "../../generated/Alias";
import type { AliasInfo } from "../../generated/AliasInfo";
import type { ProjectInfo } from "../../generated/ProjectInfo";

/** One alias row as edited. `exists` is what the core found when it was read; editing the path forgets it. */
export type Row = { name: string; path: string; exists?: boolean };

const MAX_ALIASES = 100;
const MAX_NAME_CHARS = 40;

export const rowsOf = (aliases: AliasInfo[]): Row[] => aliases.map((a) => ({ name: a.name, path: a.path, exists: a.exists }));

export const addRow = (rows: Row[]): Row[] => [...rows, { name: "", path: "" }];

export const removeRow = (rows: Row[], index: number): Row[] => rows.filter((_, i) => i !== index);

export function patchRow(rows: Row[], index: number, patch: { name?: string; path?: string }): Row[] {
  return rows.map((r, i) => {
    if (i !== index) return r;
    if (patch.path === undefined) return { ...r, ...patch };
    const { exists: _forgotten, ...rest } = r;
    return { ...rest, ...patch };
  });
}

/** What Save sends: names and paths trimmed. The core trims too; this keeps the checks below on the same text. */
export const forSave = (rows: Row[]): Alias[] => rows.map((r) => ({ name: r.name.trim(), path: r.path.trim() }));

/**
 * The first refusal the core would give for this list, in the core's order, so the person hears it without a round trip.
 * "Isn't inside the project" is left to the core, which knows the project's root.
 */
export function firstProblem(rows: Row[]): string | null {
  if (rows.length > MAX_ALIASES) return `A project can have at most ${MAX_ALIASES} aliases.`;
  const seen = new Set<string>();
  for (const { name, path } of forSave(rows)) {
    if (name === "") return "An alias needs a name.";
    if (path === "") return "An alias needs a path.";
    if ([...name].length > MAX_NAME_CHARS) return `A name can be at most ${MAX_NAME_CHARS} characters.`;
    const key = name.toLowerCase();
    if (seen.has(key)) return `Two aliases are named "${name}".`;
    seen.add(key);
  }
  return null;
}

const norm = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();
const holds = (project: string, cwd: string) => {
  const a = norm(project);
  const b = norm(cwd);
  return b === a || b.startsWith(`${a}/`);
};
const leaf = (p: string) => p.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || p;

/**
 * The picker's options and the one preselected. The routed cwd's project is the listed project holding it (the longest match);
 * when none does, the cwd itself is offered first, so the palette command always lands on the focused session's folder.
 * With no cwd the first project is selected.
 */
export function pickProject(projects: ProjectInfo[], cwd: string | undefined): { options: ProjectInfo[]; selected: string | null } {
  if (cwd === undefined) return { options: projects, selected: projects[0]?.path ?? null };
  const holder = projects.filter((p) => holds(p.path, cwd)).sort((a, b) => b.path.length - a.path.length)[0];
  if (holder) return { options: projects, selected: holder.path };
  return { options: [{ path: cwd, name: leaf(cwd), branch: null, last_used_ms: 0 }, ...projects], selected: cwd };
}
