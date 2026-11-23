// The same table as `review_command` in crates/core/src/profiles/detect.rs; a test there pins the two together.
const REVIEW_COMMANDS: Record<string, string> = {
  claude: "/code-review",
  codex: "/review",
  copilot: "/review",
  qwen: "/review",
};

/** The command that asks an agent CLI to review the changes, or null where it has none (Antigravity, a plain shell, an unknown CLI). */
export function reviewCommandOf(agent: string | null): string | null {
  return agent !== null && Object.hasOwn(REVIEW_COMMANDS, agent) ? REVIEW_COMMANDS[agent]! : null;
}

/** The menu row for the review action. `agentName` is the CLI's display name. */
export function reviewLabel(agent: string, agentName: string): string {
  const command = reviewCommandOf(agent);
  if (command === null) return `Ask ${agentName} to review (no review command, so it types a request)`;
  if (agent === "qwen") return `Ask ${agentName} to review (types ${command}, uncommitted work only)`;
  return `Ask ${agentName} to review (types ${command})`;
}
