import { describe, expect, it } from "vitest";
import { resumeCommand, sessionResumeCommand } from "./resumeCommand";

describe("resumeCommand", () => {
  it("is the CLI's own continue command for each agent", () => {
    expect(resumeCommand("claude", "PowerShell")).toBe("claude --continue");
    expect(resumeCommand("codex", "PowerShell")).toBe("codex resume --last");
  });
  it("is agy --continue for Antigravity", () => {
    expect(resumeCommand("antigravity", "pwsh")).toBe("agy --continue");
  });
  it("appends the CLI's own flags for the mode the session started in", () => {
    expect(resumeCommand("claude", "pwsh", "bypass")).toBe("claude --continue --permission-mode bypassPermissions");
    expect(resumeCommand("codex", "pwsh", "auto-edit")).toBe("codex resume --last --sandbox workspace-write --ask-for-approval on-request");
    expect(resumeCommand("antigravity", "pwsh", "full-auto")).toBe("agy --continue --mode auto");
  });
  it("adds nothing for a plain shell, whatever the mode", () => {
    expect(resumeCommand(null, "Git Bash", "plan")).toBe("Git Bash");
  });
  it("is the profile's name for a plain shell", () => {
    expect(resumeCommand(null, "Git Bash")).toBe("Git Bash");
  });
});

describe("sessionResumeCommand", () => {
  // The core resumes with the saved mode, so an Ended Bypass tab's bar must say so.
  it("shows the mode the session was last started in", () => {
    expect(sessionResumeCommand({ agent: "claude", mode: "bypass" }, "pwsh")).toBe("claude --continue --permission-mode bypassPermissions");
  });
  it("shows no flags for a session without a mode", () => {
    expect(sessionResumeCommand({ agent: "codex", mode: null }, "pwsh")).toBe("codex resume --last");
  });
});
