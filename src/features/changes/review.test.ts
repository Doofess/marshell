import { describe, expect, it } from "vitest";
import { reviewCommandOf, reviewLabel } from "./review";

describe("reviewCommandOf", () => {
  // The same table as `review_command` in crates/core/src/profiles/detect.rs; a test there pins the two together.
  it.each([
    ["claude", "/code-review"],
    ["codex", "/review"],
    ["copilot", "/review"],
    ["qwen", "/review"],
    ["antigravity", null],
    [null, null],
    ["something-else", null],
  ] as const)("gives %s the command %s", (agent, command) => {
    expect(reviewCommandOf(agent)).toBe(command);
  });
});

describe("reviewLabel", () => {
  it("names Claude Code's command", () => {
    expect(reviewLabel("claude", "Claude Code")).toBe("Ask Claude Code to review (types /code-review)");
  });
  it("names the /review command for Codex and Copilot", () => {
    expect(reviewLabel("codex", "Codex CLI")).toBe("Ask Codex CLI to review (types /review)");
    expect(reviewLabel("copilot", "Copilot CLI")).toBe("Ask Copilot CLI to review (types /review)");
  });
  it("says Qwen Code reviews uncommitted work only", () => {
    expect(reviewLabel("qwen", "Qwen Code")).toBe("Ask Qwen Code to review (types /review, uncommitted work only)");
  });
  it("says a CLI with no review command types a request", () => {
    expect(reviewLabel("antigravity", "Antigravity")).toBe("Ask Antigravity to review (no review command, so it types a request)");
    expect(reviewLabel("unknown", "Some CLI")).toBe("Ask Some CLI to review (no review command, so it types a request)");
  });
});
