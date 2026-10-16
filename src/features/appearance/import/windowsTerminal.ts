import { makeTheme, requireColor } from "./colors";
import { parseJsonc } from "./jsonc";
import type { ImportedTheme } from "./types";

const ANSI_KEYS = [
  "black", "red", "green", "yellow", "blue", "purple", "cyan", "white",
  "brightBlack", "brightRed", "brightGreen", "brightYellow", "brightBlue", "brightPurple", "brightCyan", "brightWhite",
];

type Json = Record<string, unknown>;

function isObject(v: unknown): v is Json {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

/** A colour that may be left out; a value that is there but not a colour is an error. */
function optionalColor(key: string, v: unknown, fallback: string): string {
  return v === undefined ? fallback : requireColor(key, v);
}

function schemeToTheme(s: unknown, used: Set<string>): ImportedTheme {
  if (!isObject(s) || typeof s.name !== "string" || s.name.trim() === "") {
    throw new Error('A colour scheme has no "name". Give it one and import again.');
  }
  const ansi = ANSI_KEYS.map((key) => requireColor(key, s[key]));
  const foreground = requireColor("foreground", s.foreground);
  const colors = {
    foreground,
    background: requireColor("background", s.background),
    cursor: optionalColor("cursorColor", s.cursorColor, foreground),
    selection: optionalColor("selectionBackground", s.selectionBackground, "#808080"),
    ansi,
  };
  return makeTheme("windows-terminal", s.name.trim(), colors, used);
}

/** A scheme that could not be read, and why, worded to follow its name: "Foo" + "is missing brightPurple". */
export type SkippedScheme = { name: string; reason: string };

/** Thrown when a file has colour schemes but none of them can be read. The message is the first problem found. */
export class NoReadableSchemes extends Error {
  constructor(
    message: string,
    readonly skipped: SkippedScheme[],
  ) {
    super(message);
  }
}

function skipOf(s: unknown, err: Error): SkippedScheme {
  const name = isObject(s) && typeof s.name === "string" ? s.name.trim() : "";
  if (name === "") return { name: "A scheme", reason: "has no name" };
  const missing = /^The theme is missing "(.+?)"/.exec(err.message);
  if (missing) return { name, reason: `is missing ${missing[1]}` };
  const bad = /^"(.+?)" is not a colour/.exec(err.message);
  if (bad) return { name, reason: `has a ${bad[1]} that isn't a colour` };
  return { name, reason: err.message };
}

/** One line for the schemes that were left out, for example "Skipped 2 schemes: Foo is missing brightPurple; Bar has no name." */
export function skippedNote(skipped: SkippedScheme[]): string {
  return `Skipped ${skipped.length} ${skipped.length === 1 ? "scheme" : "schemes"}: ${skipped.map((s) => `${s.name} ${s.reason}`).join("; ")}.`;
}

/**
 * Reads the colour schemes of a Windows Terminal settings.json (`schemes[]`) or a single scheme object. A scheme that can't be
 * read is skipped on its own; throws only when there are schemes and none can be read.
 */
export function parseWtSchemes(text: string): { themes: ImportedTheme[]; skipped: SkippedScheme[] } {
  const data = parseJsonc(text);
  if (!isObject(data)) throw new Error("This file is not a Windows Terminal settings file or colour scheme.");
  const list = Array.isArray(data.schemes) ? data.schemes : data.schemes === undefined ? [data] : null;
  if (!list || list.length === 0) throw new Error("This Windows Terminal file has no colour schemes.");
  const used = new Set<string>();
  const themes: ImportedTheme[] = [];
  const skipped: SkippedScheme[] = [];
  let first: Error | null = null;
  for (const s of list) {
    try {
      themes.push(schemeToTheme(s, used));
    } catch (e) {
      const err = e instanceof Error ? e : new Error(String(e));
      first ??= err;
      skipped.push(skipOf(s, err));
    }
  }
  if (themes.length === 0) throw new NoReadableSchemes(first!.message, skipped);
  return { themes, skipped };
}

/** The schemes in several Windows Terminal files, and one note for those skipped. A file with no schemes at all adds nothing. */
export function schemesInFiles(texts: string[]): { themes: ImportedTheme[]; note: string | null } {
  const themes: ImportedTheme[] = [];
  const skipped: SkippedScheme[] = [];
  for (const text of texts) {
    try {
      const out = parseWtSchemes(text);
      themes.push(...out.themes);
      skipped.push(...out.skipped);
    } catch (e) {
      if (e instanceof NoReadableSchemes) skipped.push(...e.skipped);
    }
  }
  return { themes, note: skipped.length > 0 ? skippedNote(skipped) : null };
}

/** The default profile font of a Windows Terminal settings.json: the `font` object, else the older `fontFace` and `fontSize`. */
export function wtDefaultFont(text: string): { family?: string; size?: number } {
  const data = parseJsonc(text);
  const profiles = isObject(data) ? data.profiles : undefined;
  const defaults = isObject(profiles) ? profiles.defaults : undefined;
  if (!isObject(defaults)) return {};
  const font = isObject(defaults.font) ? defaults.font : {};
  const family = font.face ?? defaults.fontFace;
  const size = font.size ?? defaults.fontSize;
  const out: { family?: string; size?: number } = {};
  if (typeof family === "string" && family.trim() !== "") out.family = family;
  if (typeof size === "number" && Number.isFinite(size) && size > 0) out.size = size;
  return out;
}

/** True when a parsed JSON file looks like a Windows Terminal one. */
export function looksLikeWt(data: unknown): boolean {
  return isObject(data) && ("schemes" in data || "brightPurple" in data);
}

