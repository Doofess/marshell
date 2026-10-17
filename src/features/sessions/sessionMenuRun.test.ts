import { describe, expect, it } from "vitest";
import type { SessionInfo } from "../../generated/SessionInfo";
import type { RowModel } from "../sidebar/types";
import { menuFacts } from "./sessionMenuRun";

const info = (tab_id: string, running = true): SessionInfo =>
  ({ tab_id, state: running ? { kind: "running" } : { kind: "exited", code: 0 } }) as SessionInfo;
const row = (id: string, task = false) => ({ id, task }) as RowModel;
const ctx = (over: Partial<Parameters<typeof menuFacts>[1]> = {}) => ({
  infos: [info("x")],
  rows: [row("x")],
  panes: ["x", null] as (string | null)[],
  shown: 2,
  fit: 4,
  ...over,
});

describe("menuFacts", () => {
  it("a running session is not resumable", () => {
    expect(menuFacts("x", ctx()).resumable).toBe(false);
  });
  it("an exited one is", () => {
    expect(menuFacts("x", ctx({ infos: [info("x", false)] })).resumable).toBe(true);
  });
  it("task follows the row", () => {
    expect(menuFacts("x", ctx()).task).toBe(false);
    expect(menuFacts("x", ctx({ rows: [row("x", true)] })).task).toBe(true);
  });
  it("a session in a shown pane is inShownPane; one in a pane out of sight is not", () => {
    expect(menuFacts("x", ctx()).inShownPane).toBe(true);
    expect(menuFacts("x", ctx({ panes: [null, "x"], shown: 1 })).inShownPane).toBe(false);
  });
  it("four panes with no empty one leave no room", () => {
    expect(menuFacts("z", ctx({ panes: ["a", "b", "c", "d"], shown: 4, fit: 4 })).paneAvailable).toBe(false);
  });
  it("an empty shown pane is available", () => {
    expect(menuFacts("z", ctx({ panes: ["x", null] })).paneAvailable).toBe(true);
  });
  it("room for another pane is available", () => {
    expect(menuFacts("z", ctx({ panes: ["a"], shown: 1, fit: 4 })).paneAvailable).toBe(true);
  });
});
