import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { ThemePair } from "../../design/ThemePair";
import { nextSessionNumber } from "../sidebar/sessionNumber";
import { DEFAULT_CHOICE, PROJECTS, REMEMBERED, TAKEN_NUMBERS } from "./fixtures";
import { Launcher, type LauncherView } from "./Launcher";
import { LAUNCH_PROGRAMS, applyRemembered, isAgentProgram, startingText, type LauncherChoice } from "./launchChoice";

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
const manual = (program: LauncherChoice["program"]): LauncherView => ({ ...base, choice: { ...DEFAULT_CHOICE, program } });

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
export const AgentsShellsAndTools: StoryObj = {
  render: () => <ThemePair label="The first chip opens a list of agents, shells and tools" stack>{frame({ ...base, programsOpen: true }, {}, 640)}</ThemePair>,
};
export const ShellPicked: StoryObj = {
  render: () => <ThemePair label="A shell is a plain terminal: no mode, pace or worktree, and the line asks for a first command" stack>{frame(manual("powershell"), {}, 400)}</ThemePair>,
};
export const SshPicked: StoryObj = {
  render: () => <ThemePair label="A tool asks what it needs: where to connect" stack>{frame(manual("ssh"), {}, 400)}</ThemePair>,
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
  const [programsOpen, setProgramsOpen] = useState(false);
  const [active, setActive] = useState(0);
  const open = (next: { query?: string | null; programs?: boolean }) => {
    if (next.query !== undefined) setQuery(next.query);
    if (next.programs !== undefined) setProgramsOpen(next.programs);
    setActive(next.programs ? Math.max(0, LAUNCH_PROGRAMS.findIndex((p) => p.id === choice.program)) : 0);
  };
  return frame(
    { choice, projects: PROJECTS, query, activeIndex: active, programsOpen, remembered: isAgentProgram(choice.program) ? REMEMBERED[choice.projectId] : undefined, nextNumber: NEXT },
    {
      onChange: (p) => setChoice((c) => ({ ...c, ...p })),
      onQuery: (q) => open({ query: q, programs: false }),
      onPrograms: (o) => open({ programs: o, query: null }),
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

