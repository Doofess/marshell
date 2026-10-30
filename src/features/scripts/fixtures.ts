import type { RepoScripts } from "../../generated/RepoScripts";
import type { SessionInfo } from "../../generated/SessionInfo";

/** A repo's scripts for tests; every field can be replaced. */
export const scriptsFixture = (over: Partial<RepoScripts> = {}): RepoScripts => ({
  root: "C:/dev/app",
  file: "C:/dev/app/marshell.json",
  hash: "h1",
  setup: "pnpm install",
  run: "pnpm dev",
  teardown: null,
  trusted: false,
  problem: null,
  ...over,
});

/** A session as the core lists it, running a plain shell. */
export const infoFixture = (over: Partial<SessionInfo> = {}): SessionInfo => ({
  tab_id: "t1",
  number: 1,
  name: "dev",
  agent: null,
  profile_id: "default",
  cwd: "C:/dev/app",
  project: "C:/dev/app",
  mode: null,
  state: { kind: "running" },
  command: { kind: "unknown" } as SessionInfo["command"],
  paused: false,
  foreground: null,
  live: null,
  activity: null,
  status_note: null,
  muted: false,
  attention: null,
  stuck: null,
  ...over,
});
