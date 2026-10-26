import { describe, expect, it } from "vitest";
import type { AgentCatalog } from "../../generated/AgentCatalog";
import type { CatalogAgent } from "../../generated/CatalogAgent";
import { commandById } from "../../lib/keymap";
import { catalogModes, decide, effectiveMode, launchArgs, MODES, nextOffered, offeredMode, offeredModes, switchKind, type Request } from "./sessionModes";

const edit: Request = { kind: "edit", inProject: true };
const editOutside: Request = { kind: "edit", inProject: false };
const command: Request = { kind: "command", inProject: true };

describe("the five modes", () => {
  it("are the same five for every agent CLI, strictest first", () => {
    expect(MODES).toEqual(["manual", "plan", "auto-edit", "full-auto", "bypass"]);
  });
  it("each has a palette command that names the mode and says in plain words what asks", () => {
    for (const mode of MODES) {
      const c = commandById(`mode-${mode}`);
      expect(c.group).toBe("Sessions");
      expect(c.label).toMatch(/^Mode: (Manual|Plan|Auto-edit|Full auto|Bypass), /);
    }
    expect(commandById("mode-bypass").label).toContain("risky");
  });
});

describe("launchArgs: with Marshell's hooks on, a session starts strict and the hook relaxes it", () => {
  it("Manual, Auto-edit and Bypass start each CLI in its strictest native mode, so every request reaches Marshell", () => {
    for (const mode of ["manual", "auto-edit", "bypass"] as const) {
      expect(launchArgs("claude", mode, true), mode).toEqual({ args: ["--permission-mode", "manual"], native: mode === "manual" });
      expect(launchArgs("codex", mode, true), mode).toEqual({ args: ["--sandbox", "read-only", "--ask-for-approval", "on-request"], native: mode === "manual" });
    }
  });
  it("Plan uses the CLI's own plan mode; Codex has no plan flag, so a read-only sandbox stands in", () => {
    expect(launchArgs("claude", "plan", true)).toEqual({ args: ["--permission-mode", "plan"], native: true });
    expect(launchArgs("antigravity", "plan", true)).toEqual({ args: ["--mode", "plan"], native: true });
    expect(launchArgs("codex", "plan", true)).toEqual({ args: ["--sandbox", "read-only", "--ask-for-approval", "on-request"], native: false });
    expect(launchArgs("codex", "plan", false)).toEqual({ args: ["--sandbox", "read-only", "--ask-for-approval", "on-request"], native: false });
  });
  it("Full auto starts the CLI in its own auto mode", () => {
    expect(launchArgs("claude", "full-auto", true)).toEqual({ args: ["--permission-mode", "auto"], native: true });
    expect(launchArgs("codex", "full-auto", true)).toEqual({ args: ["--approve-for-me"], native: true });
    expect(launchArgs("antigravity", "full-auto", true)).toEqual({ args: ["--mode", "auto"], native: true });
  });
});

describe("launchArgs: with the hooks off, each mode is the CLI's own flag, or not offered", () => {
  it("Claude", () => {
    expect(launchArgs("claude", "auto-edit", false)).toEqual({ args: ["--permission-mode", "acceptEdits"], native: true });
    expect(launchArgs("claude", "bypass", false)).toEqual({ args: ["--permission-mode", "bypassPermissions"], native: true });
  });
  it("Codex", () => {
    expect(launchArgs("codex", "auto-edit", false)).toEqual({ args: ["--sandbox", "workspace-write", "--ask-for-approval", "on-request"], native: true });
    expect(launchArgs("codex", "bypass", false)).toEqual({ args: ["--dangerously-bypass-approvals-and-sandbox"], native: true });
  });
  it("Antigravity", () => {
    expect(launchArgs("antigravity", "auto-edit", false)).toEqual({ args: ["--mode", "accept-edits"], native: true });
    expect(launchArgs("antigravity", "bypass", false)).toEqual({ args: ["--dangerously-skip-permissions"], native: true });
    expect(launchArgs("antigravity", "full-auto", false)).toEqual({ args: ["--mode", "auto"], native: true });
  });
});

describe("decide: what Marshell's hook answers when a CLI asks", () => {
  it("a risky request always gets a card, in every mode, Bypass included (the guard rail)", () => {
    for (const mode of MODES) for (const cli of ["claude", "codex", "antigravity"] as const) expect(decide(cli, mode, command, true), `${cli} ${mode}`).toBe("ask");
  });
  it("Manual and Plan ask about everything", () => {
    for (const mode of ["manual", "plan"] as const) for (const r of [edit, command]) expect(decide("claude", mode, r, false)).toBe("ask");
  });
  it("Auto-edit allows edits inside the project and asks about the rest", () => {
    expect(decide("codex", "auto-edit", edit, false)).toBe("allow");
    expect(decide("codex", "auto-edit", editOutside, false)).toBe("ask");
    expect(decide("codex", "auto-edit", command, false)).toBe("ask");
  });
  it("Full auto: where the CLI's classifier already let the rest through, what reaches Marshell asks; without one, Marshell allows", () => {
    expect(decide("claude", "full-auto", command, false)).toBe("ask");
    expect(decide("codex", "full-auto", command, false)).toBe("ask");
    expect(decide("antigravity", "full-auto", command, false)).toBe("allow");
  });
  it("Bypass allows everything that is not risky", () => {
    for (const cli of ["claude", "codex", "antigravity"] as const) for (const r of [edit, editOutside, command]) expect(decide(cli, "bypass", r, false)).toBe("allow");
  });
});

describe("switchKind: changing mode mid-session", () => {
  it("is instant when the session's native start stays the same (only Marshell's answer changes)", () => {
    expect(switchKind("claude", "manual", "bypass", true)).toBe("instant");
  });
  it("needs the CLI's own mode when the native start differs (Plan, or a classifier)", () => {
    expect(switchKind("claude", "manual", "plan", true)).toBe("native");
    expect(switchKind("claude", "bypass", "full-auto", true)).toBe("native");
    expect(switchKind("codex", "plan", "manual", true)).toBe("instant");
  });
  it("is always native with the hooks off", () => {
    expect(switchKind("claude", "manual", "auto-edit", false)).toBe("native");
  });
  it("is native for Antigravity's Full auto, which is its own --mode auto", () => {
    expect(switchKind("antigravity", "manual", "full-auto", false)).toBe("native");
  });
});

describe("effectiveMode: a CLI setting that skips every prompt", () => {
  it("makes the session Bypass whatever mode was picked", () => {
    for (const mode of MODES) expect(effectiveMode(mode, true), mode).toBe("bypass");
  });
  it("leaves the mode alone otherwise", () => {
    for (const mode of MODES) expect(effectiveMode(mode, false), mode).toBe(mode);
  });
});

describe("a manifest agent: only the modes its manifest maps", () => {
  const cursor = ["manual", "plan", "bypass"] as const;
  it("catalogModes keeps the catalog's modes in MODES order and drops Don't ask", () => {
    expect(catalogModes(["bypass", "dont-ask", "manual", "plan", "full-auto", "auto-edit"])).toEqual(MODES);
    expect(catalogModes(["bypass", "manual"])).toEqual(["manual", "bypass"]);
    expect(catalogModes([])).toEqual([]);
  });
  it("switchKind is native for an offered mode and unavailable for an unmapped one, hooks on or off", () => {
    for (const hooksOn of [false, true]) {
      expect(switchKind("grok", "manual", "plan", hooksOn, [...MODES])).toBe("native");
      expect(switchKind("cursor", "manual", "bypass", hooksOn, [...cursor])).toBe("native");
      expect(switchKind("cursor", "manual", "auto-edit", hooksOn, [...cursor])).toBe("unavailable");
      expect(switchKind("opencode", "manual", "plan", hooksOn, [])).toBe("unavailable");
      expect(switchKind("grok", "manual", "plan", hooksOn)).toBe("unavailable");
    }
  });
  it("launchArgs answers null for an id that is not an adapter instead of throwing", () => {
    expect(launchArgs("grok", "plan", false)).toBeNull();
    expect(launchArgs("grok", "plan", true)).toBeNull();
  });
  it("keeps the adapters' answers as they were", () => {
    expect(switchKind("claude", "manual", "plan", true)).toBe("native");
    expect(switchKind("codex", "manual", "auto-edit", true)).toBe("instant");
    expect(switchKind("codex", "manual", "full-auto", true)).toBe("native");
    expect(switchKind("claude", "auto-edit", "bypass", true)).toBe("instant");
  });
  it("offeredMode keeps an offered mode, else falls to the first offered in MODES order, else none", () => {
    expect(offeredMode(undefined, "auto-edit")).toBe("auto-edit");
    expect(offeredMode([...cursor], "plan")).toBe("plan");
    expect(offeredMode([...cursor], "auto-edit")).toBe("manual");
    expect(offeredMode(["bypass"], "manual")).toBe("bypass");
    expect(offeredMode([], "manual")).toBeUndefined();
  });
  it("nextOffered cycles only the offered modes", () => {
    expect(nextOffered([...cursor], "manual")).toBe("plan");
    expect(nextOffered([...cursor], "plan")).toBe("bypass");
    expect(nextOffered([...cursor], "bypass")).toBe("manual");
    expect(nextOffered([...cursor], "auto-edit")).toBe("manual");
    expect(nextOffered(undefined, "bypass")).toBe("manual");
  });
});

describe("offeredModes", () => {
  const entry = (id: string, modes: string[], adapter = false) => ({ id, adapter, modes }) as unknown as CatalogAgent;
  const catalog = {
    agents: [entry("claude", ["manual", "plan", "auto-edit", "full-auto", "bypass"], true), entry("cursor", ["manual", "plan", "bypass"]), entry("opencode", [])],
    presets: [],
  } as unknown as AgentCatalog;
  it("an adapter offers the five, with or without the catalog", () => {
    expect(offeredModes("claude", catalog)).toEqual(MODES);
    expect(offeredModes("codex", null)).toEqual(MODES);
  });
  it("a manifest agent offers what its catalog entry maps, in MODES order", () => {
    expect(offeredModes("cursor", catalog)).toEqual(["manual", "plan", "bypass"]);
    expect(offeredModes("opencode", catalog)).toEqual([]);
  });
  it("a manifest agent offers nothing when the catalog is missing or doesn't list it", () => {
    expect(offeredModes("cursor", null)).toEqual([]);
    expect(offeredModes("grok", catalog)).toEqual([]);
    expect(offeredModes(null, catalog)).toEqual([]);
  });
});

describe("Antigravity never starts or switches in a Marshell-held strict mode (slice 6d decision 7)", () => {
  it("antigravity_launches_natively_with_hooks_on", () => {
    for (const hooksOn of [true, false]) {
      expect(launchArgs("antigravity", "manual", hooksOn), `manual ${hooksOn}`).toEqual({ args: [], native: true });
      expect(launchArgs("antigravity", "plan", hooksOn), `plan ${hooksOn}`).toEqual({ args: ["--mode", "plan"], native: true });
      expect(launchArgs("antigravity", "auto-edit", hooksOn), `auto-edit ${hooksOn}`).toEqual({ args: ["--mode", "accept-edits"], native: true });
      expect(launchArgs("antigravity", "full-auto", hooksOn), `full-auto ${hooksOn}`).toEqual({ args: ["--mode", "auto"], native: true });
      expect(launchArgs("antigravity", "bypass", hooksOn), `bypass ${hooksOn}`).toEqual({ args: ["--dangerously-skip-permissions"], native: true });
    }
  });
  it("every_antigravity_mode_switch_is_native", () => {
    for (const hooksOn of [true, false]) for (const from of MODES) for (const to of MODES) expect(switchKind("antigravity", from, to, hooksOn), `${from} to ${to} ${hooksOn}`).toBe("native");
  });
});
