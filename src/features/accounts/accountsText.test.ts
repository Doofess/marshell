import { describe, expect, it } from "vitest";
import type { AccountStatus } from "../../generated/AccountStatus";
import { accountExtraLines, accountLine, signInResult } from "./accountsText";

const NOW = 1_000_000_000;
const st = (o: Partial<AccountStatus>): AccountStatus => ({ agent: "claude", sign_in: "signed_in", email: null, plan: null, method: null, last_account: null, token_env: null, checked_ms: NOW, ...o });

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

describe("Qwen Code's account", () => {
  const q = (o: Partial<AccountStatus> = {}) => st({ agent: "qwen", sign_in: "no_command", ...o });
  it("Qwen's line says it has no status command and how to sign in", () => {
    expect(accountLine(q(), "Qwen Code", NOW)).toBe("Qwen Code has no sign-in status command, so Marshell can't show one. Sign in with /auth in a Qwen Code session.");
    expect(accountExtraLines(q())).toEqual([]);
  });
  it("Qwen's configured type is named", () => {
    expect(accountExtraLines(q({ method: "Qwen OAuth" }))).toEqual(["Its settings choose Qwen OAuth."]);
    expect(accountExtraLines(q({ method: "an OpenAI-compatible API" }))).toEqual(["Its settings choose an OpenAI-compatible API."]);
  });
  it("an unknown type shows in backticks", () => {
    expect(accountExtraLines(q({ method: "`future`" }))).toEqual(["Its settings choose `future`."]);
  });
  it("Antigravity's line is unchanged and has no second line", () => {
    const a = st({ agent: "antigravity", sign_in: "no_command" });
    expect(accountLine(a, "Antigravity", NOW)).toBe("Antigravity has no sign-in status command, so Marshell can't show one.");
    expect(accountExtraLines(a)).toEqual([]);
  });
});

describe("Copilot CLI's account", () => {
  const c = (o: Partial<AccountStatus> = {}) => st({ agent: "copilot", sign_in: "no_command", ...o });
  it("Copilot's line says it has no status command", () => {
    expect(accountLine(c(), "Copilot CLI", NOW)).toBe("Copilot CLI has no sign-in status command, so Marshell can't show one.");
    expect(accountExtraLines(c())).toEqual([]);
  });
  it("Copilot's last account is named", () => {
    expect(accountExtraLines(c({ last_account: "octocat" }))).toEqual(["Copilot's own file names octocat as the last account signed in."]);
    expect(accountExtraLines(c({ last_account: "octocat on github.com" }))).toEqual(["Copilot's own file names octocat as the last account signed in."]);
    expect(accountExtraLines(c({ last_account: "octocat on https://github.com/" }))).toEqual(["Copilot's own file names octocat as the last account signed in."]);
    expect(accountExtraLines(c({ last_account: "octocat on ghe.example.com" }))).toEqual(["Copilot's own file names octocat on ghe.example.com as the last account signed in."]);
  });
  it("a set token variable is named after the account", () => {
    expect(accountExtraLines(c({ last_account: "octocat", token_env: "GH_TOKEN" }))).toEqual([
      "Copilot's own file names octocat as the last account signed in.",
      "GH_TOKEN is set, and Copilot CLI uses it before a stored sign-in.",
    ]);
    expect(accountExtraLines(c({ token_env: "COPILOT_GITHUB_TOKEN" }))).toEqual(["COPILOT_GITHUB_TOKEN is set, and Copilot CLI uses it before a stored sign-in."]);
  });
});

describe("signInResult", () => {
  it("names the CLI once and says what its status command reported", () => {
    expect(signInResult(st({ email: "a@example.com" }), "Claude Code", NOW)).toBe("Claude Code: Signed in as a@example.com, checked just now");
    expect(signInResult(st({ agent: "codex", sign_in: "signed_out" }), "Codex", NOW)).toBe("Codex: Not signed in, checked just now");
  });
  it("Copilot CLI: the line that already names it, then the last account", () => {
    const c = (o: Partial<AccountStatus> = {}) => st({ agent: "copilot", sign_in: "no_command", ...o });
    expect(signInResult(c({ last_account: "octocat" }), "Copilot CLI", NOW)).toBe(
      "Copilot CLI has no sign-in status command, so Marshell can't show one. Copilot's own file names octocat as the last account signed in.",
    );
    expect(signInResult(c(), "Copilot CLI", NOW)).toBe("Copilot CLI has no sign-in status command, so Marshell can't show one.");
  });
});
