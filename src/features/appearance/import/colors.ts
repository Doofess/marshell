import type { ImportedTheme, TerminalColors } from "./types";

export type ThemeSource = "windows-terminal" | "iterm2" | "vscode";

const ID_PREFIX: Record<ThemeSource, string> = { "windows-terminal": "wt", iterm2: "iterm2", vscode: "vscode" };

/** Turns #rgb, #rgba, #rrggbb and #rrggbbaa into #rrggbb. The alpha is dropped. Null for anything else. */
export function normalizeColor(v: string): string | null {
  const m = /^#([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i.exec(v.trim());
  if (!m) return null;
  let h = m[1]!.toLowerCase();
  if (h.length <= 4) h = [...h].map((c) => c + c).join("");
  return `#${h.slice(0, 6)}`;
}

/** Like normalizeColor, but throws a message that names the key when the value is missing or not a colour. */
export function requireColor(key: string, v: unknown): string {
  if (v === undefined) throw new Error(`The theme is missing "${key}". Add it as #rrggbb and import again.`);
  const hex = typeof v === "string" ? normalizeColor(v) : null;
  if (!hex) throw new Error(`"${key}" is not a colour Marshell can read. Use #rrggbb.`);
  return hex;
}

/** Lower case, anything that is not a letter or digit collapsed to "-", trimmed of "-". */
export function slug(name: string): string {
  return name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "") || "theme";
}

/** The file name without its directory or extension. */
export function baseName(fileName: string): string {
  return fileName.replace(/^.*[\\/]/, "").replace(/\.[^.]*$/, "");
}

/** Builds a theme with the id "<source>:<slug>". An id already in `used` gets "-2", "-3", and so on. */
export function makeTheme(source: ThemeSource, name: string, colors: TerminalColors, used: Set<string> = new Set()): ImportedTheme {
  const base = `${ID_PREFIX[source]}:${slug(name)}`;
  let id = base;
  for (let n = 2; used.has(id); n++) id = `${base}-${n}`;
  used.add(id);
  return { id, name, source, colors };
}
