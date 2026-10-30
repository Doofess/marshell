import { describe, expect, it } from "vitest";
import { confirmScripts, runControl, scriptLines } from "./confirmScripts";
import { infoFixture, scriptsFixture } from "./fixtures";

describe("confirmScripts", () => {
  it("asks for an untrusted file with any script", () => {
    const c = confirmScripts(scriptsFixture({ setup: null, run: "pnpm dev" }));
    expect(c).toMatchObject({ kind: "ask", allowLabel: "Allow and create", cancelLabel: "Create without scripts" });
  });
  it("adds the setup line for a trusted file with a setup", () => {
    expect(confirmScripts(scriptsFixture({ trusted: true }))).toMatchObject({ kind: "setup", script: "pnpm install" });
  });
  it("shows nothing for a trusted file with no setup, a file with nothing in it, or a failed read", () => {
    expect(confirmScripts(scriptsFixture({ trusted: true, setup: null }))).toBeUndefined();
    expect(confirmScripts(scriptsFixture({ setup: null, run: null, hash: null }))).toBeUndefined();
    expect(confirmScripts(null)).toBeUndefined();
  });
  it("shows a problem as one line and never asks", () => {
    expect(confirmScripts(scriptsFixture({ problem: "bad json" }))).toMatchObject({ kind: "problem", text: "bad json" });
  });
});

describe("scriptLines", () => {
  it("lists only the present scripts, setup then run then teardown", () => {
    expect(scriptLines(scriptsFixture({ teardown: "rm -rf dist" }))).toEqual(["setup: pnpm install", "run: pnpm dev", "teardown: rm -rf dist"]);
    expect(scriptLines(scriptsFixture({ setup: null }))).toEqual(["run: pnpm dev"]);
  });
});

describe("runControl", () => {
  it("is Run when the repo has a run script", () => {
    expect(runControl(scriptsFixture(), [infoFixture()])).toEqual({ kind: "run", script: "pnpm dev" });
  });
  it("is null without a run script, with a problem, or before the scripts are read", () => {
    expect(runControl(scriptsFixture({ run: null }), [])).toBeNull();
    expect(runControl(scriptsFixture({ problem: "bad" }), [])).toBeNull();
    expect(runControl(null, [])).toBeNull();
  });
  it("is Stop when a run session for that root is running, comparing paths without case or slash kind", () => {
    const run = infoFixture({ tab_id: "r1", script: "run", cwd: "c:\\DEV\\app" });
    expect(runControl(scriptsFixture(), [infoFixture(), run])).toEqual({ kind: "stop", script: "pnpm dev", tabId: "r1" });
  });
  it("is Run when the run session is for another root, is the setup tab, or has exited", () => {
    expect(runControl(scriptsFixture(), [infoFixture({ script: "run", cwd: "C:/dev/other" })])).toMatchObject({ kind: "run" });
    expect(runControl(scriptsFixture(), [infoFixture({ script: "setup" })])).toMatchObject({ kind: "run" });
    expect(runControl(scriptsFixture(), [infoFixture({ script: "run", state: { kind: "exited", code: 1 } })])).toMatchObject({ kind: "run" });
  });
});
