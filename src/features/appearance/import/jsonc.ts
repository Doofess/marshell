/** Copies `text`, calling `visit` for every character outside a string. `visit` returns how many characters it consumed (0 to keep the one at `i`). */
function mapOutsideStrings(text: string, visit: (i: number, out: string[]) => number): string {
  const out: string[] = [];
  let i = 0;
  while (i < text.length) {
    const c = text[i]!;
    if (c === '"') {
      // Copy the whole string, honouring backslash escapes.
      let j = i + 1;
      while (j < text.length && text[j] !== '"') j += text[j] === "\\" ? 2 : 1;
      out.push(text.slice(i, j + 1));
      i = j + 1;
      continue;
    }
    const used = visit(i, out);
    if (used > 0) i += used;
    else {
      out.push(c);
      i++;
    }
  }
  return out.join("");
}

function stripComments(text: string): string {
  return mapOutsideStrings(text, (i, out) => {
    if (text.startsWith("//", i)) {
      const end = text.indexOf("\n", i);
      return (end < 0 ? text.length : end) - i;
    }
    if (text.startsWith("/*", i)) {
      // A space keeps the tokens either side of the comment apart.
      out.push(" ");
      const end = text.indexOf("*/", i + 2);
      return (end < 0 ? text.length : end + 2) - i;
    }
    return 0;
  });
}

function stripTrailingCommas(text: string): string {
  // Looks past whitespace for a closing bracket; the sticky flag matches at lastIndex only.
  const closing = /\s*[}\]]/y;
  return mapOutsideStrings(text, (i) => {
    if (text[i] !== ",") return 0;
    closing.lastIndex = i + 1;
    // Drop the comma only; the whitespace and the closing bracket are copied as usual.
    return closing.test(text) ? 1 : 0;
  });
}

/** Parses JSON as Windows Terminal and VS Code write it: a leading BOM, line and block comments and trailing commas are allowed. */
export function parseJsonc(text: string): unknown {
  const clean = stripTrailingCommas(stripComments(text.replace(/^﻿/, "")));
  try {
    return JSON.parse(clean);
  } catch (e) {
    throw new Error(`Not valid JSON: ${e instanceof Error ? e.message : String(e)}`);
  }
}
