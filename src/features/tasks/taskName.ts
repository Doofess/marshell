/** The longest task name, the same as the core's `MAX_NAME` in `crates/core/src/tasks.rs`. */
const MAX_NAME = 40;

/**
 * Text turned into a task name: the same rules as the core's `slug`, so the launcher can show the branch before the core
 * makes it. ASCII letters and digits are kept (lowercased), every other run becomes one "-", the ends are trimmed, and a
 * name over 40 characters is cut at the last "-" at or before 40 (or hard-cut when there is none). Text with no ASCII
 * letter or digit gives "": the core then picks `task-<n>`.
 */
export function slugify(text: string): string {
  let out = "";
  for (const ch of text) {
    // ASCII only, as in Rust: toLowerCase() on a non-ASCII letter (like "İ") must not sneak a letter in.
    if (/^[A-Za-z0-9]$/.test(ch)) out += ch.toLowerCase();
    else if (!out.endsWith("-")) out += "-";
  }
  out = out.replace(/^-+|-+$/g, "");
  if (out.length > MAX_NAME) {
    const cut = out.slice(0, MAX_NAME + 1).lastIndexOf("-");
    out = out.slice(0, cut === -1 ? MAX_NAME : cut);
  }
  return out.replace(/^-+|-+$/g, "");
}

/** The task name field: what the user typed, or until they type anything, the name taken from the prompt. */
export const taskNameFor = (edited: string | null, prompt: string): string => edited ?? slugify(prompt);

/** The line under the field. An empty name reads `task-…` because the core picks the number. */
export function branchHint(name: string): string {
  return `Branch marshell/${slugify(name) || "task-…"}`;
}
