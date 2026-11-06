import { describe, expect, it } from "vitest";
import { measureDoneAnnouncement, measureLine } from "./measure";
import { DOCTOR_REPORT } from "./fixtures";
import type { DoctorReport } from "./types";

const withIds = (...ids: string[]): DoctorReport => ({
  ...DOCTOR_REPORT,
  items: ids.map((id) => ({ id, agent: "claude", group: "broken", title: "t", detail: "d", fixable: false, open_agents: false })),
});

describe("measureLine", () => {
  it("says the servers haven't been measured", () => {
    expect(measureLine(undefined, 1000)).toBe("MCP servers haven't been measured. Measuring starts each local server for a moment.");
  });
  it("counts seconds, then minutes from 60 s", () => {
    expect(measureLine(1_000, 11_000)).toBe("MCP servers were measured 10 s ago.");
    expect(measureLine(0, 59_000)).toBe("MCP servers were measured 59 s ago.");
    expect(measureLine(0, 150_000)).toBe("MCP servers were measured 2 min ago.");
  });
});

describe("measureDoneAnnouncement", () => {
  it("says none, one or n servers didn't start, counting only mcpfail items", () => {
    expect(measureDoneAnnouncement(withIds("mcpcost.claude", "hooks.claude"))).toBe("No MCP server failed to start.");
    expect(measureDoneAnnouncement(withIds("mcpfail.claude.a", "mcpcost.claude"))).toBe("1 MCP server didn't start.");
    expect(measureDoneAnnouncement(withIds("mcpfail.claude.a", "mcpfail.codex.b"))).toBe("2 MCP servers didn't start.");
  });
});
