import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { nextSessionNumber } from "../sidebar/sessionNumber";
import { hasPromptLine } from "../sidebar/promptLine";
import { SessionRow } from "../sidebar/SessionRow";
import { FIXTURES } from "../sidebar/fixtures";
import { APPROVALS } from "../approval/fixtures";
import { ApproveCard } from "../approval/ApproveCard";
import { AllSessionsMenu } from "./AllSessionsMenu";
import { MainWindow } from "./MainWindow";
import { Rail } from "./Rail";
import { StatusBar } from "../composer/StatusBar";
import { SCENARIO_PORTS, SCENARIO_ROWS, SCENARIO_ROWS_WITH_TOOLS } from "./scenario";
import { SessionHeader } from "./SessionHeader";
import { SplitView } from "./SplitView";

const row = (id: string) => FIXTURES.find((f) => f.id === id)!;
const docker = row("tool-docker");
const pwsh = row("tool-powershell");
const ssh = row("tool-ssh");
const claude = row("working");
const count = (s: string, re: RegExp) => (s.match(re) ?? []).length;

describe("session numbers: every surface that names a session shows one", () => {
  it("gives every fixture its own number", () => {
    const nums = FIXTURES.map((f) => f.number);
    expect(new Set(nums).size).toBe(nums.length);
    for (const n of nums) expect(Number.isInteger(n) && n >= 1).toBe(true);
  });
  it("leaves a gap in the scenario, and the next session would fill it", () => {
    const taken = SCENARIO_ROWS_WITH_TOOLS.map((r) => r.number);
    expect(taken).toEqual([1, 2, 3, 5, 6, 7, 8, 9]);
    expect(nextSessionNumber(taken)).toBe(4);
  });
  for (const density of ["comfortable", "expanded"] as const)
    it(`the sidebar row (${density}) shows it before the name and names itself "Session N"`, () => {
      const h = renderToStaticMarkup(<SessionRow row={claude} density={density} />);
      expect(h).toContain('class="session-number"');
      expect(h.indexOf("session-number")).toBeLessThan(h.indexOf("session-row__name"));
      expect(h).toMatch(/aria-label="Session 3, api-server/);
    });
  it("the rail item carries it on the mark and in its name", () => {
    const h = renderToStaticMarkup(<Rail rows={SCENARIO_ROWS} needsYou={2} />);
    expect(count(h, /class="rail__number"/g)).toBe(5);
    expect(h).toContain('aria-label="Session 3, api-server');
  });
  it("the session header shows it before the name", () => {
    const h = renderToStaticMarkup(<SessionHeader row={claude} />);
    expect(h.indexOf("session-number")).toBeLessThan(h.indexOf("session-header__name"));
    expect(h).toContain('aria-label="Session 3, api-server"');
  });
  it("every pane header shows it before the name", () => {
    const h = renderToStaticMarkup(<SplitView panes={[claude, docker]} size={{ w: 1400, h: 800 }} />);
    for (const section of h.split("<section").slice(1)) {
      const header = section.slice(section.indexOf("<header"), section.indexOf("</header>"));
      expect(header.indexOf("session-number")).toBeGreaterThan(-1);
      expect(header.indexOf("session-number")).toBeLessThan(header.indexOf("pane__name"));
    }
    expect(h).toContain('aria-label="Session 8, terminal: compose"');
  });
  it("the lane card (and so the drawer) shows it, so a request says which session it is from", () => {
    const h = renderToStaticMarkup(<ApproveCard request={APPROVALS.safeBash} />);
    expect(h).toContain('class="session-number"');
    expect(h).toContain('aria-label="Session 1, billing: Run a command');
    expect(h.indexOf("session-number")).toBeLessThan(h.indexOf("approve-card__name"));
  });
  it("the all-sessions menu shows it in each row", () => {
    const h = renderToStaticMarkup(<AllSessionsMenu rows={SCENARIO_ROWS_WITH_TOOLS} />);
    expect(count(h, /class="session-number"/g)).toBe(8);
    expect(h).toContain('aria-label="Session 9, prod-box');
  });
  it("the window shows it in the sidebar, the header and the panes at once", () => {
    const h = renderToStaticMarkup(<MainWindow width={1500} height={900} layout="split" rows={SCENARIO_ROWS_WITH_TOOLS} />);
    expect(count(h, /class="session-number"/g)).toBeGreaterThan(10);
  });
  it("teaches the phrase in the badge's tooltip", () => {
    expect(renderToStaticMarkup(<SessionRow row={claude} density="comfortable" />)).toContain("Say “session 6” to an agent");
  });
});

describe("manual CLIs: a plain terminal with its program's mark", () => {
  for (const r of [pwsh, docker, ssh])
    for (const density of ["comfortable", "expanded"] as const)
      it(`${r.id} (${density}) has no ring, model, mode, subagents or motion`, () => {
        const h = renderToStaticMarkup(<SessionRow row={r} density={density} />);
        expect(h).not.toContain("context-ring");
        expect(h).not.toContain("session-row__model");
        expect(h).not.toContain("subagents");
        expect(h).not.toContain("data-working");
        expect(h).not.toContain("session-row__pill");
        expect(h).toContain("data-tool");
      });
  it("running a command is a still glyph with its own words, not the pulsing working one", () => {
    const h = renderToStaticMarkup(<SessionRow row={docker} density="comfortable" />);
    expect(h).toContain('data-kind="running"');
    expect(h).not.toContain('data-kind="working"');
    expect(h).toContain("Running docker compose up");
  });
  it("an exited one says its exit code", () => {
    expect(renderToStaticMarkup(<SessionRow row={ssh} density="comfortable" />)).toContain("Exited · code 255");
  });
  it("is drawn by its own mark in the header too, with no context ring", () => {
    const h = renderToStaticMarkup(<SessionHeader row={docker} />);
    expect(h).toContain('aria-label="Docker"');
    expect(h).not.toContain("context-ring");
  });
  it("the status bar shows only what a plain terminal can know (no model, context, plan meters or MCP)", () => {
    const h = renderToStaticMarkup(<StatusBar row={docker} ports={SCENARIO_PORTS} />);
    expect(h).not.toMatch(/ctx|mcp|skills|5h|7d|Opus|\$/);
    expect(h).toContain("~2 changed");
    expect(h).toContain("6m");
  });
  it("hasPromptLine: agents have one, manual CLIs and empty panes do not", () => {
    expect(hasPromptLine(claude)).toBe(true);
    expect(hasPromptLine(docker)).toBe(false);
    expect(hasPromptLine(pwsh)).toBe(false);
    expect(hasPromptLine(null)).toBe(false);
  });
});

describe("manual CLIs have no prompt line: the terminal is where you type", () => {
  const forms = (h: string) => count(h, /<form class="composer"/g);
  it("in split view only the agent's pane carries one, and the plain pane's terminal takes its rows", () => {
    const h = renderToStaticMarkup(<SplitView panes={[claude, docker]} size={{ w: 1400, h: 800 }} />);
    expect(forms(h)).toBe(1);
    const sections = h.split("<section").slice(1);
    const plain = sections.find((s) => s.includes('data-agent="docker"'))!;
    const agent = sections.find((s) => s.includes('data-agent="claude"'))!;
    expect(plain).toContain('data-plain="true"');
    expect(plain).not.toContain("pane__composer");
    expect(plain).not.toContain("grid-template-rows:28px minmax(0, 1fr) 42px");
    expect(agent).toContain("grid-template-rows:28px minmax(0, 1fr) 42px");
  });
  it("a split of only manual CLIs has no prompt line anywhere", () => {
    expect(forms(renderToStaticMarkup(<SplitView panes={[pwsh, docker]} size={{ w: 1400, h: 800 }} />))).toBe(0);
  });
  it("a single manual CLI hides the full prompt line but keeps the status bar", () => {
    const h = renderToStaticMarkup(<MainWindow width={1400} height={800} rows={SCENARIO_ROWS_WITH_TOOLS} selectedId="tool-docker" />);
    expect(h).not.toContain("window__composer");
    expect(forms(h)).toBe(0);
    expect(h).toContain("window__status");
  });
  it("the window gives a plain terminal no plan meters, since they are the AI plan's", () => {
    const h = renderToStaticMarkup(<MainWindow width={1400} height={800} rows={SCENARIO_ROWS_WITH_TOOLS} selectedId="tool-docker" />);
    const bar = h.slice(h.indexOf("window__status"));
    expect(bar).not.toMatch(/5h|7d/);
    const agent = renderToStaticMarkup(<MainWindow width={1400} height={800} rows={SCENARIO_ROWS_WITH_TOOLS} selectedId="working" />);
    expect(agent.slice(agent.indexOf("window__status"))).toMatch(/5h/);
  });
  it("a single agent keeps its prompt line", () => {
    const h = renderToStaticMarkup(<MainWindow width={1400} height={800} rows={SCENARIO_ROWS_WITH_TOOLS} selectedId="working" />);
    expect(h).toContain("window__composer");
    expect(forms(h)).toBe(1);
  });
  it("the pane decides from hasPromptLine, in one place", () => {
    expect(readFileSync(new URL("./Pane.tsx", import.meta.url), "utf8")).toContain("hasPromptLine(row)");
  });
});
