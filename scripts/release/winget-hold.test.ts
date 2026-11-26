import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { isHeld, submitDecision } from "./winget-hold.mjs";

const TAG = "v0.1.0-alpha.3";
const HELD =
  "The repository variable WINGET_HOLD is set, so v0.1.0-alpha.3 wasn't submitted to winget-pkgs. To submit it, run this workflow with action 'submit' and tag 'v0.1.0-alpha.3'. To let tags submit again, delete the variable (docs/RELEASING.md).";
const NO_TOKEN =
  "No WINGET_TOKEN secret, so v0.1.0-alpha.3 wasn't submitted to winget-pkgs. Add the secret (docs/RELEASING.md), then run this workflow with action 'submit' and tag 'v0.1.0-alpha.3'.";

describe("isHeld", () => {
  it.each([
    [undefined, false],
    ["", false],
    ["false", false],
    ["true", true],
    ["True", true],
    ["FALSE", true],
    [" false ", true],
    ["  ", true],
    ["yes", true],
    ["1", true],
  ])("%j holds: %s", (value, held) => {
    expect(isHeld(value)).toBe(held);
  });
});

describe("submitDecision", () => {
  it("a tag submits while the hold is off", () => {
    expect(submitDecision({ event: "push", tag: TAG, hasToken: "true", hold: "" })).toEqual({ submit: true });
    expect(submitDecision({ event: "push", tag: TAG, hasToken: "true", hold: "false" })).toEqual({ submit: true });
    expect(submitDecision({ event: "push", tag: TAG, hasToken: "true" })).toEqual({ submit: true });
  });

  it("a tag is held while WINGET_HOLD is set", () => {
    expect(submitDecision({ event: "push", tag: TAG, hasToken: "true", hold: "true" })).toEqual({
      submit: false,
      notice: HELD,
    });
  });

  it("a manual submit ignores the hold", () => {
    expect(submitDecision({ event: "workflow_dispatch", tag: TAG, hasToken: "true", hold: "true" })).toEqual({
      submit: true,
    });
  });

  it("without the token the notice is the token's", () => {
    expect(submitDecision({ event: "push", tag: TAG, hasToken: "false", hold: "true" })).toEqual({
      submit: false,
      notice: NO_TOKEN,
    });
    expect(submitDecision({ event: "push", tag: TAG, hasToken: "", hold: "true" })).toEqual({
      submit: false,
      notice: NO_TOKEN,
    });
  });
});

describe("the CLI", () => {
  const script = fileURLToPath(new URL("./winget-hold.mjs", import.meta.url));

  // CI's runner sets GITHUB_OUTPUT for every step, so each run starts from an environment without the
  // script's own variables.
  function run(vars: Record<string, string>) {
    const env: Record<string, string | undefined> = { ...process.env };
    for (const name of ["GITHUB_OUTPUT", "WINGET_HOLD", "EVENT", "TAG", "HAS_TOKEN"]) delete env[name];
    return spawnSync(process.execPath, [script], { env: { ...env, ...vars }, encoding: "utf8" });
  }

  function withOutputFile(test: (file: string) => void) {
    const dir = mkdtempSync(join(tmpdir(), "winget-hold-"));
    try {
      test(join(dir, "output"));
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  }

  it("holds a tag and says so", () => {
    withOutputFile((file) => {
      const result = run({ EVENT: "push", TAG, HAS_TOKEN: "true", WINGET_HOLD: "true", GITHUB_OUTPUT: file });
      expect(result.status).toBe(0);
      expect(readFileSync(file, "utf8").split(/\r?\n/)).toContain("submit=false");
      expect(result.stdout.split(/\r?\n/)).toContain(`::notice::${HELD}`);
    });
  });

  it("lets a tag submit without the variable", () => {
    withOutputFile((file) => {
      const result = run({ EVENT: "push", TAG, HAS_TOKEN: "true", GITHUB_OUTPUT: file });
      expect(result.status).toBe(0);
      expect(readFileSync(file, "utf8").split(/\r?\n/)).toContain("submit=true");
      expect(result.stdout).not.toContain("::notice::");
    });
  });

  it("fails without GITHUB_OUTPUT", () => {
    const result = run({ EVENT: "push", TAG, HAS_TOKEN: "true" });
    expect(result.status).toBe(1);
  });

  it("fails without TAG and writes nothing", () => {
    withOutputFile((file) => {
      const result = run({ EVENT: "push", HAS_TOKEN: "true", GITHUB_OUTPUT: file });
      expect(result.status).toBe(1);
      expect(existsSync(file)).toBe(false);
    });
  });
});
