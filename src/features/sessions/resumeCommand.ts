/**
 * The command Resume will run for a session, for display only. It mirrors `resume_args` in
 * crates/core/src/profiles/detect.rs, which is what the core really runs; keep the two together.
 * Antigravity has no verified continue flag, so it resumes as a fresh start. A plain shell has no agent,
 * so it starts again as its profile.
 */
export function resumeCommand(agent: string | null, profileName: string): string {
  switch (agent) {
    case null:
      return profileName;
    case "claude":
      return "claude --continue";
    case "codex":
      return "codex resume --last";
    case "antigravity":
      return "agy";
    default:
      // The core refuses an agent it does not know, with its own words; show the name until then.
      return agent;
  }
}
