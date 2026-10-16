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

/** Reads the colour schemes of a Windows Terminal settings.json (`schemes[]`) or a single scheme object. */
export function parseWtSchemes(text: string): ImportedTheme[] {
  const data = parseJsonc(text);
  if (!isObject(data)) throw new Error("This file is not a Windows Terminal settings file or colour scheme.");
  const list = Array.isArray(data.schemes) ? data.schemes : data.schemes === undefined ? [data] : null;
  if (!list || list.length === 0) throw new Error("This Windows Terminal file has no colour schemes.");
  const used = new Set<string>();
  return list.map((s) => schemeToTheme(s, used));
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

