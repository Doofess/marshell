import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { AccountStatus } from "../../generated/AccountStatus";
import { AccountsView } from "./AccountsSection";

const NOW = 1_000_000_000;
const st = (agent: string, sign_in: AccountStatus["sign_in"]): AccountStatus => ({ agent, sign_in, email: null, plan: null, method: null, checked_ms: NOW });
const none = () => {};
const view = (rows: Parameters<typeof AccountsView>[0]["rows"]) => renderToStaticMarkup(<AccountsView rows={rows} now={NOW} onSignIn={none} onCheckAgain={none} />);

describe("AccountsView", () => {
  it("offers Sign in for signed-out or unknown Claude Code and Codex only", () => {
    const h = view([
      { agent: "claude", status: st("claude", "signed_out") },
      { agent: "codex", status: st("codex", "unknown") },
      { agent: "antigravity", status: st("antigravity", "no_command") },
    ]);
    expect(h).toContain("Sign in to Claude Code");
    expect(h).toContain("Sign in to Codex");
    expect(h).not.toContain("Sign in to Antigravity");
    expect(h).toContain("<h3");
  });
  it("offers no Sign in when signed in", () => {
    const h = view([{ agent: "claude", status: st("claude", "signed_in") }]);
    expect(h).not.toContain("Sign in to");
    expect(h).toContain("Check again");
  });
  it("says so when no CLI is found, with no Check again", () => {
    const h = view([]);
    expect(h).toContain("No agent CLI is installed, so there is nothing to sign in to.");
    expect(h).not.toContain("Check again");
  });
  it("shows Checking… and disables Check again while checking", () => {
    const h = view([{ agent: "codex", status: "checking" }]);
    expect(h).toContain("Checking…");
    expect(h).toMatch(/<button[^>]*disabled[^>]*>Check again/);
  });
  it("shows the failure line", () => {
    expect(view([{ agent: "codex", status: "failed" }])).toContain("Couldn&#x27;t reach the Marshell core.");
  });
});
