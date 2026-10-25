import { describe, expect, it } from "vitest";
import { ApiError } from "../../lib/api/client";
import { blankForm, buildManifest, buildSave, deriveId, formFromManifest, joinFlags, outcomeOf, splitFlags, splitNames } from "./ownAgents";

describe("deriveId", () => {
  it("lowercases and dashes the name", () => {
    expect(deriveId("My Agent")).toBe("my-agent");
    expect(deriveId("  --Foo__Bar!! ")).toBe("foo-bar");
  });
  it("stops at 32 characters without a trailing dash", () => {
    const id = deriveId("a".repeat(31) + " b c");
    expect(id.length).toBeLessThanOrEqual(32);
    expect(id.endsWith("-")).toBe(false);
  });
});

describe("splitFlags", () => {
  it("splits on spaces and keeps a quoted run together", () => {
    expect(splitFlags(`--resume "my session" -c`)).toEqual(["--resume", "my session", "-c"]);
    expect(splitFlags("   ")).toEqual([]);
  });
  it("round-trips through joinFlags", () => {
    expect(splitFlags(joinFlags(["--a", "b c"]))).toEqual(["--a", "b c"]);
  });
  it("splits program names on commas or spaces", () => {
    expect(splitNames("foo, bar baz,,")).toEqual(["foo", "bar", "baz"]);
  });
});

describe("buildManifest", () => {
  const form = { ...blankForm(), name: "My Agent", command: "C:\\tools\\my agent.exe", exeNames: "my-agent, myagent" };
  it("builds the manifest with its defaults", () => {
    expect(buildManifest(form)).toEqual({
      id: "my-agent",
      name: "My Agent",
      cli: "My Agent",
      mark: "generic",
      commands: ["C:\\tools\\my agent.exe"],
      reject_paths: [],
      version_shape: null,
      exe_names: ["my-agent", "myagent"],
      packages: [],
      prompt: { style: "none" },
      continue: null,
      modes: {},
    });
  });
  it("maps the prompt styles", () => {
    expect(buildManifest({ ...form, prompt: "positional" }).prompt).toEqual({ style: "positional" });
    expect(buildManifest({ ...form, prompt: "flag", promptFlag: "-p" }).prompt).toEqual({ style: "flag", flag: "-p" });
  });
  it("keeps only the modes with flags, and manual [] when it has none", () => {
    const m = buildManifest({ ...form, continueFlags: "--continue", modes: { ...form.modes, plan: "--plan", bypass: " " }, manualNoFlags: true });
    expect(m.continue).toEqual(["--continue"]);
    expect(m.modes).toEqual({ manual: [], plan: ["--plan"] });
  });
  it("takes typed manual flags over the no-flags option", () => {
    expect(buildManifest({ ...form, modes: { ...form.modes, manual: "--ask" }, manualNoFlags: true }).modes).toEqual({ manual: ["--ask"] });
  });
  it("round-trips an entry through the form", () => {
    const m = buildManifest({ ...form, prompt: "flag", promptFlag: "-p", continueFlags: "-c", modes: { ...form.modes, plan: "--plan x" }, manualNoFlags: true });
    expect(buildManifest(formFromManifest(m))).toEqual(m);
  });
});

describe("buildSave", () => {
  const a = buildManifest({ ...blankForm(), name: "A", command: "a" });
  const b = buildManifest({ ...blankForm(), name: "B", command: "b" });
  it("adds, replaces and removes by index, with the version read", () => {
    expect(buildSave([a], "v1", { add: b })).toEqual({ agents: [a, b], version: "v1" });
    expect(buildSave([a, b], "v1", { replace: 0, with: { ...a, name: "A2" } }).agents[0]?.name).toBe("A2");
    expect(buildSave([a, b], "v1", { remove: 0 })).toEqual({ agents: [b], version: "v1" });
  });
});

describe("outcomeOf", () => {
  it("maps a 409 to the reload copy", () => {
    expect(outcomeOf(new ApiError(409, "changed"))).toEqual({ kind: "conflict", text: "agents.json changed since you opened it. Reload to see the change." });
  });
  it("maps a 422 to reasons by index", () => {
    const body = JSON.stringify([{ index: 1, id: "b", reason: "Its command is empty." }]);
    expect(outcomeOf(new ApiError(422, body))).toEqual({ kind: "refused", problems: [{ index: 1, id: "b", reason: "Its command is empty." }] });
  });
  it("maps anything else to a plain failure", () => {
    expect(outcomeOf(new ApiError(500, "x")).kind).toBe("failed");
    expect(outcomeOf(new ApiError(422, "not json")).kind).toBe("failed");
    expect(outcomeOf(new Error("offline")).kind).toBe("failed");
  });
});
