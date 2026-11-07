import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { ThemePair } from "../../design/ThemePair";
import { nextSessionNumber } from "../sidebar/sessionNumber";
import { DEFAULT_CHOICE, PROJECTS, REMEMBERED, TAKEN_NUMBERS } from "./fixtures";
import { Launcher, type LauncherView } from "./Launcher";
import { LAUNCH_AGENTS, LAUNCH_SHELLS, applyRemembered, startingText, type LauncherChoice } from "./launchChoice";
import { missingLine, type ShellOffer } from "./shellAgents";

const meta: Meta = { title: "Launcher", parameters: { layout: "fullscreen" } };
export default meta;

const frameStyle = (height: number) => ({ blockSize: height, position: "relative" as const, background: "var(--bg-base)", borderRadius: 8, overflow: "clip" as const });
const frame = (v: LauncherView, h: Partial<Parameters<typeof Launcher>[0]> = {}, height = 480) => (
  <div style={frameStyle(height)}>
    <Launcher view={v} {...h} />
  </div>
);
const NEXT = nextSessionNumber(TAKEN_NUMBERS);
const base: LauncherView = { choice: DEFAULT_CHOICE, projects: PROJECTS, query: null, activeIndex: 0, nextNumber: NEXT };
const agentView = (agent: string): LauncherView => ({ ...base, choice: { ...DEFAULT_CHOICE, agent } });
const shellOnly = (shell: LauncherChoice["shell"]): LauncherView => ({ ...base, choice: { ...DEFAULT_CHOICE, shell, agent: null } });

export const Empty: StoryObj = { render: () => <ThemePair label="Empty: pre-filled with the most recent folder; it says it becomes session 4" stack>{frame(base)}</ThemePair> };
export const Filtering: StoryObj = { render: () => <ThemePair label={'Filtering: "ap" in the folder chip'} stack>{frame({ ...base, query: "ap" })}</ThemePair> };
export const NoMatch: StoryObj = { render: () => <ThemePair label="No folder matches" stack>{frame({ ...base, query: "zzz" })}</ThemePair> };
export const TypedPathFound: StoryObj = {
  render: () => (
    <ThemePair label="A typed path that exists: Use <path> first, Browse… last" stack>
      {frame({ ...base, query: "C:\\dev\\my-app", pathCheck: { path: "C:\\dev\\my-app", state: "found", folder: { path: "C:\\dev\\my-app", name: "my-app", branch: "main" } } })}
    </ThemePair>
  ),
};
export const TypedPathMissing: StoryObj = {
  render: () => (
    <ThemePair label="A typed path with no folder: a disabled row, and Enter does nothing" stack>
      {frame({ ...base, query: "C:\\nope", pathCheck: { path: "C:\\nope", state: "missing" } })}
    </ThemePair>
  ),
};
export const RememberedForThisProject: StoryObj = {
  render: () => (
    <ThemePair label="Remembered for this project: Plan and Quick are pre-set" stack>
      {frame({ ...base, choice: applyRemembered(DEFAULT_CHOICE, REMEMBERED["my-app"]), remembered: REMEMBERED["my-app"] })}
    </ThemePair>
  ),
};
export const Shells: StoryObj = {
  render: () => <ThemePair label="The first chip opens the list of shells" stack>{frame({ ...base, shellsOpen: true }, {}, 640)}</ThemePair>,
};
export const AgentsInTheShell: StoryObj = {
  render: () => <ThemePair label="The second chip lists Just the shell, then the agents the shell can start" stack>{frame({ ...base, agentsOpen: true }, {}, 640)}</ThemePair>,
};
const UBUNTU: LauncherView = { ...base, choice: { ...DEFAULT_CHOICE, shell: "wsl" }, agentsOpen: true };
const UBUNTU_MISSING: { label: string; problem: "not_installed" | "not_driven_here" }[] = [
  { label: "Antigravity", problem: "not_installed" },
  { label: "GitHub Copilot", problem: "not_driven_here" },
];
const UBUNTU_OFFER: ShellOffer = { state: "ready", offered: LAUNCH_AGENTS.slice(0, 2), missing: UBUNTU_MISSING };
export const AgentMissingInTheShell: StoryObj = {
  render: () => <ThemePair label={missingLine("WSL", UBUNTU_MISSING) ?? ""} stack>{frame(UBUNTU, { offer: UBUNTU_OFFER }, 640)}</ThemePair>,
};
export const CheckingTheShell: StoryObj = {
  render: () => <ThemePair label="A shell is being asked which agents it has: Start waits" stack>{frame({ ...base, choice: { ...DEFAULT_CHOICE, shell: "wsl" } }, { offer: { state: "checking" } }, 400)}</ThemePair>,
};
export const ShellUnreached: StoryObj = {
  render: () => <ThemePair label="The shell could not be reached: only the shell is offered, and Start works" stack>{frame({ ...base, choice: { ...DEFAULT_CHOICE, shell: "wsl", agent: null } }, { offer: { state: "unreached" } }, 400)}</ThemePair>,
};
export const CommandPromptWithAnAgent: StoryObj = {
  render: () => <ThemePair label="Command Prompt can't pass a first prompt: a note replaces the prompt line" stack>{frame({ ...base, choice: { ...DEFAULT_CHOICE, shell: "cmd" } }, {}, 400)}</ThemePair>,
};
export const ShellPicked: StoryObj = {
  render: () => <ThemePair label="Just the shell is a plain terminal: no mode, pace or prompt line, and a worktree still works" stack>{frame(shellOnly("powershell"), {}, 400)}</ThemePair>,
};
export const AfterEnter: StoryObj = {
  render: () => (
    <ThemePair label="After Enter: the tab appears at once and never waits for the process (under 100 ms)">
      <p style={{ margin: 0, padding: "var(--space-3)", borderRadius: "var(--radius)", background: "var(--fill)", fontFamily: "var(--font-mono)", color: "var(--text-2)" }}>
        {startingText("claude", "my-app")}
      </p>
    </ThemePair>
  ),
};

/** Try it with the mouse or the keyboard: Tab between the chips, a letter picks a value, Enter opens or picks, arrows move, Esc closes. */
function Live() {
  const [choice, setChoice] = useState<LauncherChoice>(DEFAULT_CHOICE);
  const [query, setQuery] = useState<string | null>(null);
  const [shellsOpen, setShellsOpen] = useState(false);
  const [agentsOpen, setAgentsOpen] = useState(false);
  const [active, setActive] = useState(0);
  const open = (next: { query?: string | null; shells?: boolean; agents?: boolean }) => {
    if (next.query !== undefined) setQuery(next.query);
    setShellsOpen(next.shells === true);
    setAgentsOpen(next.agents === true);
    setActive(next.shells ? Math.max(0, LAUNCH_SHELLS.findIndex((s) => s.id === choice.shell)) : next.agents ? Math.max(0, LAUNCH_AGENTS.findIndex((p) => p.id === choice.agent) + 1) : 0);
  };
  return frame(
    { choice, projects: PROJECTS, query, activeIndex: active, shellsOpen, agentsOpen, remembered: choice.agent !== null ? REMEMBERED[choice.projectId] : undefined, nextNumber: NEXT },
    {
      onChange: (p) => setChoice((c) => ({ ...c, ...p })),
      onQuery: (q) => open({ query: q }),
      onShells: (o) => open({ shells: o, query: null }),
      onAgents: (o) => open({ agents: o, query: null }),
      onActive: setActive,
      onPick: (id) => {
        setChoice((c) => applyRemembered({ ...c, projectId: id }, REMEMBERED[id]));
        setQuery(null);
      },
    },
    640,
  );
}
export const Interactive: StoryObj = { render: () => <Live /> };


export const AdministratorOn: StoryObj = {
  render: () => <ThemePair label="Administrator on: a shield in the chip, and an agent starts in Manual with its Mode chip locked" stack>{frame({ ...base, elevated: true }, {}, 400)}</ThemePair>,
};
export const AdministratorOnForAShell: StoryObj = {
  render: () => <ThemePair label="Administrator on with no agent: no mode chip, so no second line" stack>{frame({ ...shellOnly("powershell"), elevated: true }, {}, 400)}</ThemePair>,
};
export const AdministratorOffWindows: StoryObj = {
  render: () => <ThemePair label="Not on Windows: the Administrator chip is disabled and says why" stack>{frame(base, { os: "mac" }, 400)}</ThemePair>,
};
export const WaitingForAdministratorApproval: StoryObj = {
  render: () => <ThemePair label="Start pressed while Windows asks: the launcher waits for approval" stack>{frame({ ...base, elevated: true }, { busy: true, pending: true }, 400)}</ThemePair>,
};


const skipPrograms = LAUNCH_AGENTS.map((p) => p.id === "antigravity" ? { ...p, skipsAll: { setting: "dangerouslySkipPermissions" } } : p);

export const OwnSettingBypass: StoryObj = {
  render: () => <ThemePair label="Antigravity's own setting makes Manual run as Bypass" stack>{frame(agentView("antigravity"), { agents: skipPrograms })}</ThemePair>,
};

function AdministratorOwnSetting() {
  const [bypassAcknowledged, setBypassAcknowledged] = useState(false);
  return <ThemePair label="Administrator Bypass requires the explicit confirmation" stack>{frame({ ...agentView("antigravity"), elevated: true, bypassAcknowledged }, { agents: skipPrograms, onBypassAck: setBypassAcknowledged })}</ThemePair>;
}

export const AdministratorOwnSettingBypass: StoryObj = { render: () => <AdministratorOwnSetting /> };
