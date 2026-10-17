import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { createElement } from "react";
import { describe, expect, it } from "vitest";
import { contrast } from "../../design/contrast";
import { parseTokens } from "../../design/cssTokens";
import { AGENT_IDS } from "./agents";
import { AgentMark } from "./AgentMark";
import { isTool, kindName } from "./kinds";
import { TOOL_GROUP, TOOL_GROUPS, TOOL_GROUP_NAMES, TOOL_IDS, TOOL_NAMES, TOOL_PATHS, isToolId, toolForProgram } from "./tools";

const read = (p: string) => readFileSync(new URL(p, import.meta.url), "utf8");
const colors = read("./agent-colors.css");
const tokens = parseTokens(read("../../styles/tokens.css"), ":root");
const THEMES = ["dark", "light"] as const;
const SURFACES = ["--bg-base", "--bg-raised", "--bg-overlay"] as const;
const signalToken = (id: string) => colors.match(new RegExp(String.raw`\[data-agent="${id}"\][^{]*\{[^}]*--agent-signal:\s*var\((--[\w-]+)\)`))![1]!;

describe("manual CLI marks", () => {
  it("covers shells, dev tools, remote and cloud tools, and a fallback", () => {
    for (const id of ["powershell", "bash", "zsh", "cmd", "gitbash", "wsl", "git", "docker", "kubectl", "node", "python", "cargo", "gh", "terraform", "ssh", "aws", "gcloud", "az", "supabase", "vercel", "program"]) expect(TOOL_IDS).toContain(id);
    expect(TOOL_GROUPS).toEqual(["shell", "dev", "remote", "fallback"]);
  });
  it("never reuses an AI agent's id", () => {
    for (const id of TOOL_IDS) expect(AGENT_IDS as readonly string[]).not.toContain(id);
  });
  for (const id of TOOL_IDS) {
    it(`draws ${id} with a name, a group and an outline, in a calm tone`, () => {
      expect(TOOL_NAMES[id].length).toBeGreaterThan(0);
      expect(TOOL_GROUP_NAMES[TOOL_GROUP[id]].length).toBeGreaterThan(0);
      expect(TOOL_PATHS[id].length).toBeGreaterThan(10);
      const html = renderToStaticMarkup(createElement(AgentMark, { agent: id }));
      expect(html).toContain(`data-agent="${id}"`);
      expect(html).toContain(`aria-label="${TOOL_NAMES[id]}"`);
      expect(html).toContain('fill="currentColor"');
      expect(isTool(id)).toBe(true);
      expect(kindName(id)).toBe(TOOL_NAMES[id]);
      expect(colors).toMatch(new RegExp(String.raw`\[data-agent="${id}"\][^{]*\{[^}]*--agent:\s*var\(--(tool|brand)-`));
    });
    it(`${id} stays still, even for a running command`, () => {
      const html = renderToStaticMarkup(createElement(AgentMark, { agent: id, working: true }));
      expect(html).not.toContain("data-working");
      expect(html).not.toContain("agent-mark__piece");
      expect(html).not.toContain("agent-mark__layer");
    });
    for (const theme of THEMES) {
      it(`the mark on the ${id} badge holds 3:1 and its focus ring holds 3:1 on every surface (${theme})`, () => {
        const c = tokens[signalToken(id)]![theme];
        expect(contrast(theme === "dark" ? "#000000" : "#ffffff", c)).toBeGreaterThanOrEqual(3);
        for (const s of SURFACES) expect(contrast(c, tokens[s]![theme])).toBeGreaterThanOrEqual(3);
      });
    }
  }
  it("tells AI agents from manual CLIs", () => {
    expect(isToolId("docker")).toBe(true);
    expect(isToolId("claude")).toBe(false);
    expect(isTool("claude")).toBe(false);
  });
});

describe("toolForProgram", () => {
  it.each([
    ["pwsh", "powershell"],
    ["powershell", "powershell"],
    ["bash", "bash"],
    ["zsh", "zsh"],
    ["cmd", "cmd"],
    ["wsl", "wsl"],
    ["git", "git"],
    ["docker", "docker"],
    ["docker-compose", "docker"],
    ["kubectl", "kubectl"],
    ["node", "node"],
    ["python", "python"],
    ["python3", "python"],
    ["py", "python"],
    ["cargo", "cargo"],
    ["gh", "gh"],
    ["terraform", "terraform"],
    ["ssh", "ssh"],
    ["aws", "aws"],
    ["gcloud", "gcloud"],
    ["az", "az"],
    ["supabase", "supabase"],
    ["vercel", "vercel"],
    ["fake-agent", "program"],
  ])("%s maps to %s", (program, tool) => {
    expect(toolForProgram(program)).toBe(tool);
  });
});
