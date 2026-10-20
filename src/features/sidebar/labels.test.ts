import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { nameContainsLabel } from "../../lib/labelInName";
import { rowLabel, rowTitle } from "./labels";
import { SessionRow } from "./SessionRow";
import type { RowModel } from "./types";

const base: RowModel = {
  id: "1",
  number: 6,
  name: "api-server",
  project: "my-app", projectKey: "my-app",
  branch: "feat/auth-flow",
  agent: "claude",
  status: "needs-permission",
  phrase: "Wants to run `npm test`",
  waitingMs: 65_000,
  contextPct: 42,
};

describe("rowLabel", () => {
  it("starts with the session number, then the text on screen in order, so the name contains the visible label", () => {
    expect(rowLabel(base)).toBe("Session 6, api-server my-app · feat/auth-flow 1m Wants to run npm test. Claude, Needs you: permission. Context 42% used.");
  });
  it("puts the model and its separator where they are drawn, before the phrase", () => {
    expect(rowLabel({ ...base, model: "Opus 5.5" })).toContain("1m Opus 5.5 · Wants to run npm test.");
  });
  it("leaves the separator out when there is no phrase after the model", () => {
    const l = rowLabel({ ...base, status: "idle", phrase: "", waitingMs: undefined, model: "Opus 5.5" });
    expect(l).toContain("feat/auth-flow Opus 5.5. Claude Opus 5.5, Idle.");
    expect(l).not.toContain("·.");
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
    expect(rowLabel({ ...base, name: "認証サーバー \u{1F680}" }).startsWith("Session 6, 認証サーバー \u{1F680} my-app")).toBe(true);
  });
  it("reads the subagent count where a comfortable row draws it, after the phrase", () => {
    const l = rowLabel({ ...base, model: "Opus 5.5", phrase: "Thinking", muted: false, subagents: 2 });
    expect(l).toContain("Opus 5.5 · Thinking · 2 subagents");
    expect(rowLabel({ ...base, model: "Opus 5.5", phrase: "Thinking", muted: false, subagents: 2 }, "expanded").split("2 subagents").length - 1).toBe(1);
  });
  it("in the expanded row also starts with the extra lines shown there", () => {
    const l = rowLabel({ ...base, model: "Opus 5.5", effort: "high", subagents: 2, recap: { text: "Fixed the retry loop", ageMs: 120_000 } }, "expanded");
    expect(l).toContain("Opus 5.5 high 2 subagents Fixed the retry loop 2m ago");
  });
});

describe("rowLabel and the age", () => {
  it("says <1m while the age is under a minute, so a focused row is not re-announced every second", () => {
    const l = rowLabel({ ...base, waitingMs: 12_000 });
    expect(l).not.toContain("12s");
    expect(l).toContain("my-app · feat/auth-flow <1m Wants to run npm test.");
    expect(rowLabel({ ...base, waitingMs: 65_000 })).toContain("feat/auth-flow 1m Wants");
    expect(rowLabel({ ...base, status: "done-seen", waitingMs: undefined, ageMs: 59_999, phrase: "Done" })).toContain("<1m Done.");
  });
  it("says the recap's age as <1m ago while it is under a minute", () => {
    const l = rowLabel({ ...base, recap: { text: "Fixed it", ageMs: 7000 } }, "expanded");
    expect(l).toContain("Fixed it <1m ago");
    expect(rowLabel({ ...base, recap: { text: "Fixed it", ageMs: 120_000 } }, "expanded")).toContain("Fixed it 2m ago");
  });
  it("says the recap as the expanded row draws it, even when the phrase has it too (label in name)", () => {
    const r: RowModel = { ...base, status: "done-seen", waitingMs: undefined, ageMs: 120_000, phrase: "Done · Fixed it", recap: { text: "Fixed it", ageMs: 120_000 } };
    expect(rowLabel(r, "expanded")).toContain("Fixed it 2m ago");
    expect(nameContainsLabel(renderToStaticMarkup(createElement(SessionRow, { row: r, density: "expanded" })))).toBe(true);
  });
  it("adds the core's limited-status note as a trailing part", () => {
    expect(rowLabel({ ...base, statusNote: "Live status is off for Claude Code" })).toContain("Needs you: permission. Status limited: Live status is off for Claude Code.");
  });
});

describe("rowTitle", () => {
  it("has the untruncated text on separate lines", () => {
    expect(rowTitle(base)).toBe("Session 6\napi-server\nmy-app · feat/auth-flow\nClaude\nWants to run `npm test`");
  });
});

describe("manual CLIs", () => {
  const docker: RowModel = { id: "d", number: 8, name: "compose", project: "my-app", projectKey: "my-app", branch: "main", agent: "docker", status: "running", phrase: "Running docker compose up" };
  it("are named by their program and say what they are doing, with no context or model", () => {
    const l = rowLabel(docker);
    expect(l).toBe("Session 8, compose my-app · main Running docker compose up. Docker, Running a command.");
    expect(l).not.toContain("Context");
  });
});
