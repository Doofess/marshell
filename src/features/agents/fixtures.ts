import type { AgentSetup } from "../../generated/AgentSetup";
import type { ConfigPreview } from "../../generated/ConfigPreview";
import type { FilePreview } from "../../generated/FilePreview";

/** Test input only: what the core sends for a Claude Code with a settings file, as `AgentSetup`. */
export const SETUP: AgentSetup = {
  agent: "claude",
  found: true,
  version: "2.1.3",
  exe_path: "C:/bin/claude.exe",
  settings_path: "C:/h/.claude/settings.json",
  settings_state: "ok",
  hooks: "none",
  approve_installed: false,
  has_own_statusline: true,
  chained: false,
  needs_trust: false,
  bridge_path: "C:/m/bin/marshell-hook.exe",
  snapshots_dir: "C:/m/snapshots",
};

/** Test input only: what the core sends for a Codex with no hooks of Marshell's yet. */
export const CODEX_SETUP: AgentSetup = {
  ...SETUP,
  agent: "codex",
  version: "0.163.0",
  exe_path: "C:/bin/codex.exe",
  settings_path: "C:/h/.codex/hooks.json",
  settings_state: "missing",
  has_own_statusline: false,
  needs_trust: true,
};

/** Test input only: what the core sends for an Antigravity with no hooks of Marshell's yet (hooks.json is missing; the status line is in settings.json). */
export const AGY_SETUP: AgentSetup = {
  ...SETUP,
  agent: "antigravity",
  version: "1.4.7",
  exe_path: "C:/bin/agy.exe",
  settings_path: "C:/h/.gemini/antigravity-cli/settings.json",
  hooks_path: "C:/h/.gemini/config/hooks.json",
};

const file = (path: string, diff: string): FilePreview => ({ path, scope: "user", applies: "newSessions", before_hash: "h0", diff, status: { kind: "changes" } }) as FilePreview;

export const DIFF = '--- a/C:/h/.claude/settings.json\n+++ b/C:/h/.claude/settings.json\n@@ -1,1 +1,2 @@\n {\n+  "hooks": {}\n';
export const PREVIEW: ConfigPreview = {
  changeset_id: "c1",
  title: "Set up Claude Code hooks",
  reason: "r",
  files: [file("C:/h/.claude/settings.json", DIFF), file("C:/m/agents/claude/statusline-chain.json", "--- /dev/null\n+++ b/C:/m/agents/claude/statusline-chain.json\n@@ -0,0 +1,1 @@\n+null\n")],
  stale: false,
  can_apply: true,
};
