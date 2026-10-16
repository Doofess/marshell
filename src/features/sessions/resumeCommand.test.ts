import { describe, expect, it } from "vitest";
import { resumeCommand } from "./resumeCommand";

describe("resumeCommand", () => {
  it("is the CLI's own continue command for each agent", () => {
    expect(resumeCommand("claude", "PowerShell")).toBe("claude --continue");
    expect(resumeCommand("codex", "PowerShell")).toBe("codex resume --last");
  });
  it("is a plain agy for Antigravity, which resumes as a fresh start", () => {
    expect(resumeCommand("antigravity", "PowerShell")).toBe("agy");
  });
  it("is the profile's name for a plain shell", () => {
    expect(resumeCommand(null, "Git Bash")).toBe("Git Bash");
  });
});
