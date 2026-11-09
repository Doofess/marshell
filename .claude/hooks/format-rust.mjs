// PostToolUse (Edit, Write, MultiEdit): formats the Rust file just written with rustfmt, in the edition its crate
// declares, so CI's `cargo fmt --all --check` never fails on an agent's edit. Plain Node, no dependencies.
// It never blocks: a file outside the project, a file rustfmt can't parse (an edit in progress) and a missing rustfmt are
// all left alone, and the language server reports a parse error anyway.
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { readInput } from "./lib.mjs";

/**
 * The edition named by the nearest manifest above `file`. A crate's `edition.workspace = true` names none, so the walk
 * goes on to the workspace's `[workspace.package]`. Null when no manifest names one.
 */
export function editionFor(file) {
  let dir = path.dirname(file);
  for (;;) {
    try {
      const m = /^\s*edition\s*=\s*"(\d{4})"/m.exec(fs.readFileSync(path.join(dir, "Cargo.toml"), "utf8"));
      if (m) return m[1];
    } catch {
      // no manifest here
    }
    const up = path.dirname(dir);
    if (up === dir) return null;
    dir = up;
  }
}

/** Whether `file` lies inside `root` (case-insensitively on Windows, as `path.relative` compares there). */
export function inside(root, file) {
  const rel = path.relative(root, file);
  return rel !== "" && !rel.startsWith("..") && !path.isAbsolute(rel);
}

/** Formats `file` and returns the message for Claude when rustfmt changed it, or null. */
export function formatRust(input, projectDir) {
  const file = input?.tool_input?.file_path;
  if (typeof file !== "string" || !file.endsWith(".rs") || !projectDir) return null;
  const abs = path.resolve(input.cwd || projectDir, file);
  if (!inside(projectDir, abs) || !fs.existsSync(abs)) return null;
  const edition = editionFor(abs);
  if (!edition) return null;
  const before = fs.readFileSync(abs);
  try {
    // From the file's folder, so rustup picks that checkout's toolchain and rustfmt finds its rustfmt.toml.
    execFileSync("rustfmt", ["--edition", edition, abs], { cwd: path.dirname(abs), stdio: "ignore", timeout: 10_000 });
  } catch {
    return null;
  }
  if (before.equals(fs.readFileSync(abs))) return null;
  return `rustfmt reformatted ${abs}; read it again before editing it.`;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  try {
    const message = formatRust(readInput(), process.env.CLAUDE_PROJECT_DIR);
    if (message) {
      process.stdout.write(JSON.stringify({ hookSpecificOutput: { hookEventName: "PostToolUse", additionalContext: message } }));
    }
  } catch {
    // never block an edit
  }
}
