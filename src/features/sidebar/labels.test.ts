import { describe, expect, it } from "vitest";
import { rowLabel, rowTitle } from "./labels";
import type { RowModel } from "./types";

const base: RowModel = {
  id: "1",
  name: "api-server",
  project: "my-app",
  branch: "feat/auth-flow",
  agent: "claude",
  status: "needs-permission",
  phrase: "Wants to run `npm test`",
  waitingMs: 65_000,
  contextPct: 42,
};

describe("rowLabel", () => {
  it("starts with the text on screen, in order, so the name contains the visible label", () => {
    expect(rowLabel(base)).toBe("api-server my-app · feat/auth-flow 1m Wants to run npm test. Claude, Needs you: permission. Context 42% used.");
  });
  it("puts the model and its separator where they are drawn, before the phrase", () => {
    expect(rowLabel({ ...base, model: "Opus 5.5" })).toContain("1m Opus 5.5 · Wants to run npm test.");
  });
  it("adds what only the glyphs say (agent, state, elevated, mode, muted, context) after the visible text", () => {
    const l = rowLabel({ ...base, muted: true, elevated: true, mode: "full-auto" });
    expect(l).toContain("Administrator. Full auto. Muted.");
    expect(l.indexOf("Wants to run npm test")).toBeLessThan(l.indexOf("Administrator."));
    expect(l).toContain("npm test · muted.");
  });
  it("leaves out what is unknown", () => {
    const l = rowLabel({ ...base, branch: undefined, waitingMs: undefined, contextPct: undefined });
    expect(l).not.toContain("branch");
    expect(l).not.toContain("Context");
    expect(l).not.toContain("Waiting");
    expect(l).not.toContain("–");
  });
  it("shows the age of a finished session as the row does", () => {
    expect(rowLabel({ ...base, status: "done-seen", waitingMs: undefined, ageMs: 720_000, phrase: "Done" })).toContain("my-app · feat/auth-flow 12m Done.");
  });
  it("names the agent and the model when the CLI reports one", () => {
    expect(rowLabel({ ...base, model: "Opus 5.5" })).toContain("Claude Opus 5.5, Needs you");
    expect(rowTitle({ ...base, model: "Opus 5.5" })).toContain("\nClaude Opus 5.5\n");
  });
  it("keeps names that are not Latin", () => {
    expect(rowLabel({ ...base, name: "認証サーバー \u{1F680}" }).startsWith("認証サーバー \u{1F680} my-app")).toBe(true);
  });
  it("in the expanded row also starts with the extra lines shown there", () => {
    const l = rowLabel({ ...base, model: "Opus 5.5", effort: "high", subagents: 2, recap: { text: "Fixed the retry loop", ageMs: 120_000 } }, "expanded");
    expect(l).toContain("Opus 5.5 high 2 subagents Fixed the retry loop 2m ago");
  });
});

describe("rowTitle", () => {
  it("has the untruncated text on separate lines", () => {
    expect(rowTitle(base)).toBe("api-server\nmy-app · feat/auth-flow\nClaude\nWants to run `npm test`");
  });
});
