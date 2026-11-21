import { describe, expect, it } from "vitest";
import { autoOpenList, autoOpenNote, AUTO_OPEN_OK } from "./autoOpen";

describe("autoOpenNote", () => {
  it("says nothing for a plain shell or a CLI whose hooks are in", () => {
    expect(autoOpenNote(null, {})).toBeNull();
    expect(autoOpenNote("claude", { claude: "installed" })).toBeNull();
  });
  it("says whose hooks aren't set up", () => {
    for (const state of ["none", "partial", "orphaned", undefined] as const) expect(autoOpenNote("codex", { codex: state })).toBe("Not in this session: Codex's hooks aren't set up.");
  });
  it("says an agent that reports no file writes", () => {
    expect(autoOpenNote("my-agent", {})).toBe("Not in this session: my-agent reports no file writes.");
  });
});

describe("autoOpenList", () => {
  it("lists each CLI found with what it does", () => {
    expect(AUTO_OPEN_OK).toBe("Opens its HTML files");
    expect(autoOpenList(["claude", "codex"], { claude: "installed" })).toEqual([
      { agent: "claude", label: "Claude Code", text: "Opens its HTML files" },
      { agent: "codex", label: "Codex", text: "Its hooks aren't set up" },
    ]);
  });
  it("says an agent that reports no file writes", () => {
    expect(autoOpenList(["my-agent"], {})).toEqual([{ agent: "my-agent", label: "my-agent", text: "Reports no file writes" }]);
  });
});
