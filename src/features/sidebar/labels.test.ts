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
  it("reads name, state, phrase, place and wait", () => {
    expect(rowLabel(base)).toBe(
      "api-server, Needs you: permission. Wants to run npm test. my-app, branch feat/auth-flow. Waiting 1m. Context 42% used.",
    );
  });
  it("mentions muted, elevated and the caution mode", () => {
    expect(rowLabel({ ...base, muted: true, elevated: true, mode: "full-auto" })).toContain("Administrator. Full auto. Muted.");
  });
  it("leaves out what is unknown", () => {
    const l = rowLabel({ ...base, branch: undefined, waitingMs: undefined, contextPct: undefined });
    expect(l).not.toContain("branch");
    expect(l).not.toContain("Context");
    expect(l).not.toContain("Waiting");
  });
  it("keeps names that are not Latin", () => {
    expect(rowLabel({ ...base, name: "認証サーバー 🚀" })).toContain("認証サーバー 🚀,");
  });
});

describe("rowTitle", () => {
  it("has the untruncated text on separate lines", () => {
    expect(rowTitle(base)).toBe("api-server\nmy-app · feat/auth-flow\nWants to run `npm test`");
  });
});
