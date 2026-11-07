import { describe, expect, it } from "vitest";
import type { ShellAgents } from "../../generated/ShellAgents";
import { LAUNCH_AGENTS, type LaunchProgram } from "./launchChoice";
import { effectiveAgent, firstAgent, missingLine, offerFor } from "./shellAgents";

const agent = (id: string) => LAUNCH_AGENTS.find((p) => p.id === id)!;
const deepseek: LaunchProgram = { ...agent("claude"), id: "preset:deepseek", label: "DeepSeek", preset: "deepseek", agent: "claude" };
const programs = [agent("claude"), agent("codex"), agent("antigravity"), agent("copilot"), deepseek];
const answer = (shell: string, problems: Record<string, ShellAgents["agents"][number]["problem"]>, reached = true): ShellAgents => ({
  profile_id: shell,
  reached,
  agents: Object.entries(problems).map(([id, problem]) => ({ id, problem })),
});

describe("offerFor", () => {
  it("an_unanswered_shell_is_checking", () => {
    expect(offerFor(programs, undefined, "ubuntu")).toEqual({ state: "checking" });
  });
  it("an_unreached_shell_offers_only_the_shell_and_can_start", () => {
    for (const a of ["failed", answer("ubuntu", {}, false)] as const) {
      const offer = offerFor(programs, a, "ubuntu");
      expect(offer).toEqual({ state: "unreached" });
      expect(effectiveAgent({ agent: "claude" }, offer)).toBeNull();
    }
  });
  it("a_preset_follows_its_agent", () => {
    const both = offerFor(programs, answer("ps", { claude: null, codex: "not_installed", antigravity: "not_installed", copilot: "not_installed" }), "ps");
    expect(both.state === "ready" && both.offered.map((p) => p.id)).toEqual(["claude", "preset:deepseek"]);
    const neither = offerFor(programs, answer("ps", { claude: "not_installed", codex: null, antigravity: null, copilot: null }), "ps");
    expect(neither.state === "ready" && neither.offered.map((p) => p.id)).toEqual(["codex", "antigravity", "copilot"]);
  });
  it("missing_agents_keep_catalog_order_and_exact_reasons", () => {
    const offer = offerFor(programs, answer("ubuntu", { claude: null, codex: null, antigravity: "not_installed", copilot: "not_driven_here" }), "ubuntu");
    expect(offer.state).toBe("ready");
    if (offer.state !== "ready") return;
    expect(offer.missing).toEqual([
      { label: "Antigravity", problem: "not_installed" },
      { label: "GitHub Copilot", problem: "not_driven_here" },
    ]);
    expect(missingLine("Ubuntu", offer.missing)).toBe("Not in Ubuntu: Antigravity (not installed), GitHub Copilot (Marshell runs it in WSL later)");
    expect(missingLine("Command Prompt", [{ label: "Amp", problem: "path_has_space" }])).toBe("Not in Command Prompt: Amp (its path has a space, which Command Prompt can't start)");
    expect(missingLine("Ubuntu", [])).toBeNull();
  });
  it("an_answer_for_another_shell_never_fills_this_one", () => {
    expect(offerFor(programs, answer("pwsh", { claude: null }), "ubuntu")).toEqual({ state: "checking" });
  });
});

describe("effectiveAgent", () => {
  it("a_remembered_agent_the_shell_lacks_becomes_just_the_shell", () => {
    const offer = offerFor(programs, answer("ubuntu", { claude: "not_installed", codex: null, antigravity: null, copilot: null }), "ubuntu");
    expect(effectiveAgent({ agent: "claude" }, offer)).toBeNull();
    expect(effectiveAgent({ agent: "codex" }, offer)).toBe("codex");
    expect(effectiveAgent({ agent: null }, offer)).toBeNull();
  });
  it("keeps the chosen agent while the shell is being asked", () => {
    expect(effectiveAgent({ agent: "claude" }, { state: "checking" })).toBe("claude");
  });
});

describe("firstAgent", () => {
  it("a_folder_with_nothing_remembered_starts_with_the_first_agent_the_shell_offers", () => {
    // No CLI is the default by its name: where Claude Code isn't installed, the first agent that is starts.
    const codexOnly = offerFor(programs, answer("pwsh", { claude: "not_installed", codex: null, antigravity: null, copilot: "not_installed" }), "pwsh");
    expect(firstAgent(codexOnly, [])).toBe("codex");
    expect(firstAgent(offerFor(programs, answer("pwsh", { claude: null, codex: null }), "pwsh"), [])).toBe("claude");
    // While the shell is being asked: the first agent found on this machine, so the chip keeps it when the answer agrees.
    expect(firstAgent({ state: "checking" }, ["codex", "antigravity"])).toBe("codex");
    expect(firstAgent({ state: "checking" }, [])).toBeNull();
    // No agent to offer: Just the shell.
    expect(firstAgent({ state: "unreached" }, ["codex"])).toBeNull();
    expect(firstAgent({ state: "ready", offered: [], missing: [] }, ["codex"])).toBeNull();
    // A preset is never the default by itself.
    expect(firstAgent({ state: "ready", offered: [deepseek], missing: [] }, [])).toBeNull();
  });
});
