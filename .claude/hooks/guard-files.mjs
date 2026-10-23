// PreToolUse guard for Edit, Write, MultiEdit and NotebookEdit: rules F1 to F4 of SCOPE.md's file protections.
import { P, ask, canon, deny, findGitRoot, homeKey, isMarshellRoot, key, relKey, runHook, toAbs } from "./lib.mjs";

const FILE_TOOLS = new Set(["Edit", "Write", "MultiEdit", "NotebookEdit"]);

const F1 = "src/generated is written by ts-rs. Change crates/protocol and run cargo test -p marshell-protocol.";
const F2 = "This is an owner file. Propose the change to the user instead.";
const F3 = "Capabilities, the CSP and the API guard change only with the user's word (SCOPE.md).";
const F4 = "Real CLI configuration and Marshell data are off limits while developing (SCOPE.md).";

const OWNER_FILES = new Set(["scope.md", "docs/brief.md", "license", "third_party_notices.md", "security.md"]);
const ASK_FILES = new Set(["src-tauri/tauri.conf.json", "src-tauri/tauri.windows.conf.json", "crates/core/src/server/guard.rs"]);

/**
 * F4: ~/.claude.json, or under ~/.codex, ~/.marshell or ~/.claude, except Claude's per-project memory, its settings.json (the advisor
 * switch) and plan mode's plan files.
 */
function realConfig(fileKey) {
  const home = homeKey();
  if (fileKey === `${home}/.claude.json`) return deny(F4);
  for (const dir of [".codex", ".marshell", ".claude"]) {
    const base = `${home}/${dir}`;
    if (fileKey !== base && !fileKey.startsWith(`${base}/`)) continue;
    if (dir === ".claude") {
      const rest = fileKey.slice(base.length + 1);
      if (rest === "settings.json" || /^projects\/[^/]+\/memory\/.+/.test(rest) || /^plans\/[^/]+\.md$/.test(rest)) {
        return null;
      }
    }
    return deny(F4);
  }
  return null;
}

/** F1 to F3, on a path relative to a checkout of this repo. */
function checkoutRule(rel) {
  if (rel.startsWith("src/generated/")) return deny(F1);
  if (OWNER_FILES.has(rel)) return deny(F2);
  if (rel.startsWith("src-tauri/capabilities/") || ASK_FILES.has(rel)) return ask(F3);
  return null;
}

runHook((input) => {
  if (!FILE_TOOLS.has(input.tool_name)) return null;
  const toolInput = input.tool_input;
  if (!toolInput || typeof toolInput !== "object") return null;
  const raw = toolInput.file_path ?? toolInput.notebook_path;
  if (typeof raw !== "string" || !raw) return null;

  const file = canon(toAbs(raw, typeof input.cwd === "string" ? input.cwd : undefined));
  const config = realConfig(key(file));
  if (config) return config;

  const root = findGitRoot(P.dirname(file));
  if (!root || !isMarshellRoot(root)) return null;
  const rel = relKey(root, file);
  return rel ? checkoutRule(rel) : null;
});
