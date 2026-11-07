import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "../../design/ThemePair";
import { MainWindow, type MainWindowProps } from "./MainWindow";
import { FIXTURES } from "../sidebar/fixtures";
import { SCENARIO_REQUESTS, SCENARIO_ROWS_WITH_TOOLS } from "./scenario";

const meta: Meta = { title: "Main window", parameters: { layout: "fullscreen" } };
export default meta;

const stacked = (label: string, props: MainWindowProps): StoryObj => ({
  render: () => (
    <ThemePair label={label} stack>
      <div style={{ overflowX: "auto" }}>
        <MainWindow {...props} />
      </div>
    </ThemePair>
  ),
});

// 880 tall so the lane (header, the 256 px target card and one 36 px row = 320) fits inside its 40% cap of the 840 px sidebar.
export const FiveAgents: StoryObj = stacked("The 5-agent scenario: two need you, one working, one done, one error", { width: 1180, height: 880 });
export const FocusMode: StoryObj = stacked("Focus mode: sidebar and header gone, a quiet pill top right", { width: 1180, height: 800, layout: "focus" });
export const FocusModeHeaderRevealed: StoryObj = stacked("Focus mode with the pointer on the top edge: the header slides in", { width: 1180, height: 800, layout: "focus", headerPeek: true });
export const CollapsedRail: StoryObj = stacked("Collapsed rail (52 px): stripe, vendor mark and glyph, needs-you badge stack on top", { width: 1180, height: 800, layout: "rail" });
export const AutoCollapseAt940: StoryObj = stacked("Below 960 px the sidebar collapses to the rail by itself", { width: 940, height: 620 });
export const MinimumWindow720: StoryObj = stacked("720 by 480, the minimum window", { width: 720, height: 480 });
export const DrawerPushes: StoryObj = stacked("Right drawer at 1700 px wide: pushes the terminal", { width: 1700, height: 800, drawer: true });
export const DrawerOverlays: StoryObj = stacked("Right drawer at 1180 px wide: overlays, the terminal would drop below 720", { width: 1180, height: 800, drawer: true });
export const LightTerminalInDarkApp: StoryObj = {
  render: () => (
    <ThemePair label="Terminal pinned light while the app is dark (and the reverse)" stack>
      <MainWindow width={1180} height={640} terminalSetting="light" />
    </ThemePair>
  ),
};
export const NoSessions: StoryObj = stacked("First run, or everything closed: no sessions yet, and the window says how to start one. The sidebar, header and bar stay where they are, so nothing jumps when the first session appears", { width: 1180, height: 640, rows: [], requests: [] });
export const MacShortcuts: StoryObj = stacked("macOS shortcut labels (focus pill)", { width: 1180, height: 560, layout: "focus", os: "mac" });

const twelve = Array.from({ length: 12 }, (_, i) => ({ ...SCENARIO_REQUESTS[0]!, id: `r${i}`, waitingMs: 60_000 + i * 1000 }));
export const SidebarWithShell: StoryObj = stacked("The sidebar: the session list scrolls (wheel, touch, keys) with no scrollbar, a soft fade at an edge says there is more, and under it a plain PowerShell for commands you still run by hand. It belongs to no agent, so it carries no vendor colour", { width: 1180, height: 880 });
export const ShellHidden: StoryObj = stacked("The same sidebar with the shell hidden by the user: just its header, and a button to show it again", { width: 1180, height: 880, shellCollapsed: true });
export const ShellFoldsInShortWindow: StoryObj = stacked("In a short window with twelve requests waiting, the lane takes its 40% and the shell folds to its header by itself, so the list is never squeezed under two sessions", { width: 1180, height: 520, requests: twelve });

const MIXED_IDS = ["working", "tool-docker", "tool-powershell", "needs-permission", "needs-question", "tool-ssh"];
const mixed = MIXED_IDS.map((id) => FIXTURES.find((f) => f.id === id)!);
export const ManualCliSelected: StoryObj = stacked("A manual CLI selected (docker compose up): a plain terminal, so there is no prompt line of ours (type in the terminal), no context ring and no plan meters. The sidebar lists it with its logo and number among the agents; the header shows its mark and what is known", { width: 1180, height: 880, rows: SCENARIO_ROWS_WITH_TOOLS, selectedId: "tool-docker" });
export const ManualCliRail: StoryObj = stacked("The rail with manual CLIs: each item has its program's mark and a small number, and a running command shows as a still glyph, never the moving mark", { width: 1180, height: 800, layout: "rail", rows: SCENARIO_ROWS_WITH_TOOLS, selectedId: "tool-powershell" });
export const SessionNumbersEverywhere: StoryObj = stacked("Session numbers on every surface: the sidebar rows, the session header and the lane card (\"1 · billing\"). Session 4 is free (it was closed), so the next session takes it", { width: 1700, height: 900, rows: SCENARIO_ROWS_WITH_TOOLS, selectedId: "working" });
