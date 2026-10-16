import { baseName, makeTheme } from "./colors";
import type { ImportedTheme } from "./types";

/** A plist value: a dict of values, or the text of a <real>, <integer> or <string>. */
type Plist = string | { [key: string]: Plist };

// Tags and the text between them. The <?xml ?> and <!DOCTYPE> lines match the first branch and are skipped below.
const TOKEN = /<(\/?)([A-Za-z]+)[^>]*?(\/?)>|<[?!][^>]*>|([^<]+)/g;

type Token = { kind: "open" | "close" | "empty"; tag: string } | { kind: "text"; text: string };

function tokenize(text: string): Token[] {
  const tokens: Token[] = [];
  for (const m of text.matchAll(TOKEN)) {
    if (m[4] !== undefined) {
      if (m[4].trim() !== "") tokens.push({ kind: "text", text: m[4].trim() });
    } else if (m[2] !== undefined) {
      tokens.push({ kind: m[1] ? "close" : m[3] ? "empty" : "open", tag: m[2] });
    }
  }
  return tokens;
}

/** Reads one value at `pos`. Returns it and the position after it. */
function readValue(tokens: Token[], pos: number): [Plist, number] {
  const t = tokens[pos];
  if (!t || t.kind === "text" || t.kind === "close") throw new Error("This iTerm2 file is not a valid plist.");
  if (t.kind === "empty") return [t.tag, pos + 1];
  if (t.tag !== "dict") {
    // <real>, <integer> and <string> hold their text; anything else is read the same way and ignored by the caller.
    const next = tokens[pos + 1];
    if (next?.kind === "text") return [next.text, pos + 3];
    return ["", pos + 2];
  }
  const dict: { [key: string]: Plist } = {};
  let i = pos + 1;
  while (tokens[i] && !(tokens[i]!.kind === "close")) {
    const key = tokens[i]!;
    const keyText = tokens[i + 1];
    if (key.kind !== "open" || key.tag !== "key" || keyText?.kind !== "text") throw new Error("This iTerm2 file is not a valid plist.");
    const [value, next] = readValue(tokens, i + 3);
    dict[keyText.text] = value;
    i = next;
  }
  return [dict, i + 1];
}

function parsePlist(text: string): { [key: string]: Plist } {
  const tokens = tokenize(text);
  const start = tokens.findIndex((t) => t.kind === "open" && t.tag === "dict");
  if (!tokens.some((t) => t.kind === "open" && t.tag === "plist") || start < 0) {
    throw new Error("This file is not an iTerm2 colour plist. Export the scheme from iTerm2 as .itermcolors and import that.");
  }
  const [dict] = readValue(tokens, start);
  if (typeof dict === "string") throw new Error("This iTerm2 file is not a valid plist.");
  return dict;
}

/** An iTerm2 colour dict (components from 0 to 1) as #rrggbb. */
function colorOf(key: string, v: Plist | undefined): string {
  if (typeof v !== "object") throw new Error(`The theme is missing "${key}". Re-export it from iTerm2 and import again.`);
  const byte = (name: string) => {
    const c = Number(v[`${name} Component`]);
    if (!Number.isFinite(c)) throw new Error(`"${key}" has no usable ${name.toLowerCase()} value. Re-export it from iTerm2.`);
    return Math.round(Math.min(1, Math.max(0, c)) * 255).toString(16).padStart(2, "0");
  };
  return `#${byte("Red")}${byte("Green")}${byte("Blue")}`;
}

/** Reads an iTerm2 .itermcolors file. `fileName` gives the theme its name. */
export function parseItermColors(text: string, fileName = ""): ImportedTheme {
  const dict = parsePlist(text);
  const ansi = Array.from({ length: 16 }, (_, i) => colorOf(`Ansi ${i} Color`, dict[`Ansi ${i} Color`]));
  const foreground = colorOf("Foreground Color", dict["Foreground Color"]);
  return makeTheme("iterm2", baseName(fileName) || "iTerm2 theme", {
    foreground,
    background: colorOf("Background Color", dict["Background Color"]),
    cursor: dict["Cursor Color"] === undefined ? foreground : colorOf("Cursor Color", dict["Cursor Color"]),
    selection: dict["Selection Color"] === undefined ? "#808080" : colorOf("Selection Color", dict["Selection Color"]),
    ansi,
  });
}
