import type { SessionKind } from "../../components/AgentMark/kinds";
import type { GlyphKind } from "../../components/StatusGlyph/glyphs";
import type { SessionRef } from "../launcher/filter";

/** What the palette needs to list a session: its stable number, where it runs, what it runs, and what it is doing. */
export type PaletteSession = SessionRef & { kind: SessionKind; status: GlyphKind };

/**
 * Open sessions in the stories. The numbers have gaps (4, 10 and 11 are free), as after closing a few: a number is kept
 * until its session closes, and 12 shows a session past the nine that have a Ctrl+digit shortcut. Three are manual CLIs.
 */
export const PALETTE_SESSIONS: PaletteSession[] = [
  { id: "s1", number: 1, name: "api-server", project: "my-app", branch: "feat/auth-flow", kind: "claude", status: "working" },
  { id: "s2", number: 2, name: "billing", project: "payments", branch: "fix/retry-backoff", kind: "claude", status: "needs-permission" },
  { id: "s3", number: 3, name: "infra", project: "terraform", branch: "chore/upgrade-aws-provider", kind: "codex", status: "needs-question" },
  { id: "s5", number: 5, name: "docs", project: "my-app", branch: "main", kind: "antigravity", status: "done-unseen" },
  { id: "s6", number: 6, name: "PowerShell", project: "my-app", branch: "main", kind: "powershell", status: "idle" },
  { id: "s7", number: 7, name: "docker compose up", project: "my-app", branch: "main", kind: "docker", status: "running" },
  { id: "s8", number: 8, name: "ssh prod-eu", project: "ops", kind: "ssh", status: "ended" },
  { id: "s9", number: 9, name: "scratch", project: "scratch", kind: "claude", status: "idle" },
  { id: "s12", number: 12, name: "load-test", project: "tools", branch: "main", kind: "python", status: "running" },
];
