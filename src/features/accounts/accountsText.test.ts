import { describe, expect, it } from "vitest";
import type { AccountStatus } from "../../generated/AccountStatus";
import { accountLine } from "./accountsText";

const NOW = 1_000_000_000;
const st = (o: Partial<AccountStatus>): AccountStatus => ({ agent: "claude", sign_in: "signed_in", email: null, plan: null, method: null, checked_ms: NOW, ...o });

describe("accountLine", () => {
  it("Claude with email and plan", () => {
    expect(accountLine(st({ email: "a@example.com", plan: "max" }), "Claude Code", NOW)).toBe("Signed in as a@example.com, Max, checked just now");
  });
  it("leaves out the plan or the email with its comma", () => {
    expect(accountLine(st({ email: "a@example.com" }), "Claude Code", NOW)).toBe("Signed in as a@example.com, checked just now");
    expect(accountLine(st({ plan: "pro" }), "Claude Code", NOW)).toBe("Signed in, Pro, checked just now");
    expect(accountLine(st({}), "Claude Code", NOW + 5 * 60_000)).toBe("Signed in, checked 5 min ago");
  });
  it("Codex with a method", () => {
    const c = { agent: "codex" };
    expect(accountLine(st({ ...c, method: "ChatGPT" }), "Codex", NOW)).toBe("Signed in with ChatGPT, checked just now");
    expect(accountLine(st({ ...c, method: "an API key" }), "Codex", NOW)).toBe("Signed in with an API key, checked just now");
    expect(accountLine(st(c), "Codex", NOW)).toBe("Signed in, checked just now");
  });
  it("signed out, unknown and no command", () => {
    expect(accountLine(st({ sign_in: "signed_out" }), "Codex", NOW)).toBe("Not signed in, checked just now");
    expect(accountLine(st({ sign_in: "unknown" }), "Codex", NOW)).toBe("Couldn't check the sign-in status, checked just now");
    expect(accountLine(st({ agent: "antigravity", sign_in: "no_command" }), "Antigravity", NOW)).toBe("Antigravity has no sign-in status command, so Marshell can't show one.");
  });
});
