// Shared helpers for the PreToolUse guard hooks. Plain Node, no dependencies.
// Contract: read the hook JSON on stdin; on a decision print one JSON object to stdout; always exit 0.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

export const IS_WIN = process.platform === "win32";
/** Path functions for the platform the hook runs on. */
export const P = IS_WIN ? path.win32 : path.posix;

/** The parsed hook input, or null for empty, non-JSON or non-object stdin. */
export function readInput() {
  let raw = "";
  try {
    raw = fs.readFileSync(0, "utf8");
  } catch {
    return null;
  }
  try {
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

function emit(permissionDecision, permissionDecisionReason) {
  const out = { hookSpecificOutput: { hookEventName: "PreToolUse", permissionDecision, permissionDecisionReason } };
  process.stdout.write(JSON.stringify(out));
}

/** A decision a rule returns: `{ decision: "deny" | "ask", reason }`. */
export const deny = (reason) => ({ decision: "deny", reason });
export const ask = (reason) => ({ decision: "ask", reason });

/**
 * Runs `rules(input)` and prints its decision. Anything that goes wrong, including a throw, leaves the tool call alone:
 * a bug in a guard must never block work.
 */
export function runHook(rules) {
  try {
    const input = readInput();
    if (!input) return;
    const result = rules(input);
    if (result) emit(result.decision, result.reason);
  } catch {
    // fail open
  }
}

const HOME_TOKEN = /%USERPROFILE%|\$\{(?:env:)?(?:HOME|USERPROFILE)\}|\$(?:env:)?(?:HOME|USERPROFILE)(?![A-Za-z0-9_])/gi;

/** Replaces ~, %USERPROFILE%, $HOME, ${HOME}, $env:USERPROFILE and $env:HOME with the home folder. */
export function expandHome(p) {
  const home = os.homedir();
  let s = String(p);
  if (s === "~" || /^~[\\/]/.test(s)) s = home + s.slice(1);
  return s.replace(HOME_TOKEN, () => home);
}

/** An absolute, normalized path. Handles /c/... and /mnt/c/... on Windows. Relative paths resolve against `base`. */
export function toAbs(p, base) {
  let s = expandHome(p).trim();
  if (IS_WIN) s = s.replace(/^\/mnt\/([a-zA-Z])(?=\/|$)/, "$1:").replace(/^\/([a-zA-Z])(?=\/|$)/, "$1:");
  return P.resolve(base || process.cwd(), s);
}

/**
 * `abs` with its longest existing prefix canonicalized by realpath, so 8.3 short names (C:\Users\HOMEOF~1) and
 * junctions compare equal to the real folder. The part that does not exist yet is appended unchanged. Falls back to
 * `abs` on any error.
 */
export function canon(abs) {
  try {
    let cur = abs;
    const tail = [];
    for (;;) {
      try {
        const real = fs.realpathSync.native(cur);
        return tail.length ? P.join(real, ...tail.reverse()) : real;
      } catch {
        // not there (yet): try the parent
      }
      const up = P.dirname(cur);
      if (up === cur) return abs;
      tail.push(P.basename(cur));
      cur = up;
    }
  } catch {
    return abs;
  }
}

/** The comparison key of the real home folder, canonicalized. */
export function homeKey() {
  return key(canon(P.resolve(os.homedir())));
}

/** A comparison key: forward slashes, and lower case on Windows. */
export function key(p) {
  const s = String(p).replace(/\\/g, "/");
  return IS_WIN ? s.toLowerCase() : s;
}

/** The nearest folder at or above `start` that holds a `.git` file or folder, or null. */
export function findGitRoot(start) {
  let dir = start;
  for (;;) {
    try {
      if (fs.existsSync(P.join(dir, ".git"))) return dir;
    } catch {
      // keep walking
    }
    const up = P.dirname(dir);
    if (up === dir) return null;
    dir = up;
  }
}

/** A git root is a checkout of this repo only if it has both crates/protocol and src-tauri. */
export function isMarshellRoot(root) {
  try {
    return fs.existsSync(P.join(root, "crates", "protocol")) && fs.existsSync(P.join(root, "src-tauri"));
  } catch {
    return false;
  }
}

/** `file` relative to `root` as a comparison key, or null when it is outside. */
export function relKey(root, file) {
  const rel = P.relative(root, file);
  if (!rel || rel === ".." || rel.startsWith("..\\") || rel.startsWith("../") || P.isAbsolute(rel)) return null;
  return key(rel);
}
