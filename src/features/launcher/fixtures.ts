import type { Project } from "./filter";
import type { LauncherChoice, Remembered } from "./launchChoice";

const DAY = 86_400_000;

export const PROJECTS: Project[] = [
  { id: "my-app", name: "my-app", branch: "main", path: "C:/dev/my-app", lastUsedMs: 9 * DAY },
  { id: "payments", name: "payments", branch: "fix/retry-backoff", path: "C:/dev/payments", lastUsedMs: 8 * DAY },
  { id: "marketing", name: "marketing", branch: "feat/pricing-page", path: "C:/dev/marketing", lastUsedMs: 7 * DAY },
  { id: "terraform", name: "terraform", branch: "chore/upgrade-aws-provider", path: "C:/dev/terraform", lastUsedMs: 6 * DAY },
  { id: "scratch", name: "scratch", path: "C:/Users/Home Office/scratch", lastUsedMs: 5 * DAY },
  { id: "tools", name: "tools", branch: "main", path: "C:/dev/tools", lastUsedMs: 4 * DAY },
];

export const DEFAULT_CHOICE: LauncherChoice = { program: "claude", projectId: "my-app", mode: "Manual", pace: "Balanced", worktree: false };
export const REMEMBERED: Record<string, Remembered> = { "my-app": { mode: "Plan", pace: "Quick" } };

/** The numbers sessions hold in the launcher stories: 4 is free, as after closing a session, so a new one becomes session 4. */
export const TAKEN_NUMBERS = [1, 2, 3, 5, 6, 7, 8, 9];
