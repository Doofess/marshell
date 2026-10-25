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

describe("resumeCommand for a manifest agent", () => {
  it("is the manifest's command and continue arguments", () => {
    expect(resumeCommand("grok", "PowerShell")).toBe("grok --continue");
    expect(resumeCommand("amp", "PowerShell")).toBe("amp threads continue");
    expect(resumeCommand("aider", "PowerShell")).toBe("aider --restore-chat-history");
    expect(resumeCommand("cursor", "PowerShell")).toBe("cursor-agent --continue");
  });
  it("follows with the flags of the mode it started in, as the core does", () => {
    expect(resumeCommand("grok", "pwsh", "bypass")).toBe("grok --continue --permission-mode bypassPermissions");
    expect(resumeCommand("cursor", "pwsh", "manual")).toBe("cursor-agent --continue");
    // A mode the manifest does not map adds nothing.
    expect(resumeCommand("cursor", "pwsh", "auto-edit")).toBe("cursor-agent --continue");
  });
  it("shows only the name for an agent it has no manifest for, such as one of the person's own", () => {
    expect(resumeCommand("my-agent", "pwsh", "plan")).toBe("my-agent");
  });
  it("is the same through a session", () => {
    expect(sessionResumeCommand({ agent: "grok", mode: null }, "pwsh")).toBe("grok --continue");
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
