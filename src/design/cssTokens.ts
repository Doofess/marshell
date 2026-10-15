export type ThemeValue = { light: string; dark: string };

/** Splits on commas that are not inside parentheses. */
function splitTopLevel(s: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let start = 0;
  for (let i = 0; i < s.length; i++) {
    const ch = s[i];
    if (ch === "(") depth++;
    else if (ch === ")") depth--;
    else if (ch === "," && depth === 0) {
      parts.push(s.slice(start, i).trim());
      start = i + 1;
    }
  }
  parts.push(s.slice(start).trim());
  return parts;
}

/**
 * Reads the custom properties of the first rule whose selector list contains `selector`.
 * Only for Marshell's own token files: flat rules, no nesting, no comments inside values.
 */
export function parseTokens(css: string, selector: string): Record<string, ThemeValue> {
  const noComments = css.replace(/\/\*[\s\S]*?\*\//g, "");
  const rule = /([^{}]+)\{([^{}]*)\}/g;
  for (const m of noComments.matchAll(rule)) {
    const selectors = splitTopLevel(m[1]!.trim()).map((s) => s.trim());
    if (!selectors.includes(selector)) continue;
    const out: Record<string, ThemeValue> = {};
    for (const decl of m[2]!.split(";")) {
      const i = decl.indexOf(":");
      if (i < 0) continue;
      const name = decl.slice(0, i).trim();
      const value = decl.slice(i + 1).trim();
      if (!name.startsWith("--") || value.includes("var(")) continue;
      const ld = /^light-dark\(([\s\S]*)\)$/.exec(value);
      if (ld) {
        const [light, dark] = splitTopLevel(ld[1]!);
        out[name] = { light: light!, dark: dark! };
      } else {
        out[name] = { light: value, dark: value };
      }
    }
    return out;
  }
  return {};
}
