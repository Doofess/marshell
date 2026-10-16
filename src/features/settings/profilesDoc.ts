import type { Profile } from "../../generated/Profile";
import type { ProfilesDoc } from "../../generated/ProfilesDoc";

/**
 * The command field is one line of text; the profile keeps the argv. Words split on spaces and "…" keeps a part together
 * (a Windows path with a space). Backslashes are plain characters, so paths need no escaping. An argument holding a double
 * quote of its own cannot be written in this field.
 */
export function splitCommand(text: string): string[] {
  const argv: string[] = [];
  let current = "";
  // True once the word has any character, so "" is an empty argument and not nothing.
  let started = false;
  let quoted = false;
  for (const ch of text) {
    if (ch === '"') {
      quoted = !quoted;
      started = true;
    } else if (!quoted && /\s/.test(ch)) {
      if (started) argv.push(current);
      current = "";
      started = false;
    } else {
      current += ch;
      started = true;
    }
  }
  if (started) argv.push(current);
  return argv;
}

/** The argv as the command field shows it: an argument with a space (or none at all) is quoted. */
export const joinCommand = (argv: string[]): string => argv.map((a) => (a === "" || /[\s"]/.test(a) ? `"${a}"` : a)).join(" ");

/** Whether the command field can show this argv and give the same argv back. An argument with a double quote of its own cannot. */
export const commandRoundTrips = (argv: string[]): boolean => JSON.stringify(splitCommand(joinCommand(argv))) === JSON.stringify(argv);

/** Two profiles with the same values; the server's `detected` flag does not count. */
const sameValues = (a: Profile, b: Profile): boolean =>
  a.id === b.id &&
  a.name === b.name &&
  a.family === b.family &&
  a.cwd === b.cwd &&
  a.shell_integration === b.shell_integration &&
  (a.terminal_theme ?? null) === (b.terminal_theme ?? null) &&
  JSON.stringify(a.command) === JSON.stringify(b.command) &&
  JSON.stringify(a.env) === JSON.stringify(b.env);

export type ProfileKind = "custom" | "detected" | "edited";

/**
 * Where a profile comes from. The core sends an edited detected profile as an ordinary one, so the page compares with what
 * detection found: the same id with the same values is "detected", the same id with other values is "edited", the rest "custom".
 */
export function profileKind(p: Profile, detected: Profile[]): ProfileKind {
  const original = detected.find((d) => d.id === p.id);
  if (!original) return "custom";
  return sameValues(p, original) ? "detected" : "edited";
}

export type ProfilePatch = Partial<Pick<Profile, "name" | "command" | "cwd" | "env" | "shell_integration" | "terminal_theme">>;

export const updateProfile = (doc: ProfilesDoc, id: string, patch: ProfilePatch): ProfilesDoc => ({
  ...doc,
  profiles: doc.profiles.map((p) => (p.id === id ? { ...p, ...patch } : p)),
});

/** Adds a copy of `id` as a custom profile: the lowest free `custom-<n>` id and a name nobody else has. */
export function duplicateProfile(doc: ProfilesDoc, id: string): { doc: ProfilesDoc; id: string } {
  const source = doc.profiles.find((p) => p.id === id) ?? doc.profiles[0];
  if (!source) return { doc, id };
  let n = 1;
  while (doc.profiles.some((p) => p.id === `custom-${n}`)) n++;
  let name = `${source.name} copy`;
  for (let k = 2; doc.profiles.some((p) => p.name === name); k++) name = `${source.name} copy ${k}`;
  const copy: Profile = { ...source, id: `custom-${n}`, name, command: [...source.command], env: source.env.map(([k, v]) => [k, v]), detected: false };
  return { doc: { ...doc, profiles: [...doc.profiles, copy] }, id: copy.id };
}

/** A detected profile is never deleted: it would be back at the next detection. */
export function deleteProfile(doc: ProfilesDoc, id: string, detected: Profile[]): ProfilesDoc {
  if (detected.some((d) => d.id === id)) return doc;
  const profiles = doc.profiles.filter((p) => p.id !== id);
  return { profiles, default_id: doc.default_id === id ? (profiles[0]?.id ?? "") : doc.default_id };
}

export const makeDefault = (doc: ProfilesDoc, id: string): ProfilesDoc => ({ ...doc, default_id: id });

/** Puts a detected profile back to what detection found. Any other profile stays as it is. */
export function resetProfile(doc: ProfilesDoc, id: string, detected: Profile[]): ProfilesDoc {
  const original = detected.find((d) => d.id === id);
  return original ? { ...doc, profiles: doc.profiles.map((p) => (p.id === id ? { ...original } : p)) } : doc;
}

/** What goes to the core: environment rows still without a name are left out. */
export const forSave = (doc: ProfilesDoc): ProfilesDoc => ({
  ...doc,
  profiles: doc.profiles.map((p) => ({ ...p, env: p.env.filter(([k]) => k.trim() !== "").map(([k, v]): [string, string] => [k.trim(), v]) })),
});

/** Variable names that appear more than once (trimmed, ignoring letter case as Windows does), as first written. Rows without a name do not count. */
export function duplicateEnvKeys(env: Array<[string, string]>): string[] {
  const seen = new Set<string>();
  const dups: string[] = [];
  for (const [k] of env) {
    const name = k.trim();
    if (name === "") continue;
    const key = name.toLowerCase();
    if (seen.has(key)) {
      if (!dups.some((d) => d.toLowerCase() === key)) dups.push(env.map(([x]) => x.trim()).find((x) => x.toLowerCase() === key)!);
    } else seen.add(key);
  }
  return dups;
}

/** How many profiles were added, removed or changed, plus one when the default moved. Zero means nothing to save. */
export function changeCount(saved: ProfilesDoc, draft: ProfilesDoc): number {
  const before = new Map(saved.profiles.map((p) => [p.id, p]));
  const after = new Map(draft.profiles.map((p) => [p.id, p]));
  let n = 0;
  for (const [id, p] of after) {
    const old = before.get(id);
    if (!old || !sameValues(old, p)) n++;
  }
  for (const id of before.keys()) if (!after.has(id)) n++;
  return saved.default_id === draft.default_id ? n : n + 1;
}
