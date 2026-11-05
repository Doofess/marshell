import { describe, expect, it } from "vitest";
import { cliDoneAnnouncement, cliLine } from "./cliDoctors";
import type { DoctorItem, DoctorReport } from "./types";

const item = (id: string, group: DoctorItem["group"]): DoctorItem => ({ id, agent: "codex", group, title: id, detail: "", fixable: false, open_agents: false });
const report = (items: DoctorItem[]): DoctorReport => ({ checked_at_ms: 0, duration_ms: 0, items });
const TAIL = "Codex's doctor contacts OpenAI.";

describe("cliLine", () => {
  it("says the CLI doctors haven't run before a run", () => {
    expect(cliLine(undefined, 5_000)).toBe(`CLI doctors haven't run. ${TAIL}`);
  });
  it("counts seconds under a minute", () => {
    expect(cliLine(1_000, 13_000)).toBe(`CLI doctors ran 12 s ago. ${TAIL}`);
  });
  it("counts whole minutes, rounded down, from 60 s", () => {
    expect(cliLine(0, 125_000)).toBe(`CLI doctors ran 2 min ago. ${TAIL}`);
    expect(cliLine(0, 60_000)).toBe(`CLI doctors ran 1 min ago. ${TAIL}`);
  });
});

describe("cliDoneAnnouncement", () => {
  it("finds nothing when no Broken or Waste cli item exists", () => {
    expect(cliDoneAnnouncement(report([]))).toBe("The CLI doctors found nothing.");
    expect(cliDoneAnnouncement(report([item("cli.codex.x", "info"), item("hooks.claude", "broken")]))).toBe("The CLI doctors found nothing.");
  });
  it("counts one issue", () => {
    expect(cliDoneAnnouncement(report([item("cli.codex.x", "broken"), item("cli.claude.y", "info")]))).toBe("The CLI doctors found 1 issue.");
  });
  it("counts Broken and Waste cli items", () => {
    expect(cliDoneAnnouncement(report([item("cli.a", "broken"), item("cli.b", "waste"), item("cli.c", "broken"), item("cli.d", "info"), item("hooks.x", "waste")]))).toBe("The CLI doctors found 3 issues.");
  });
});
