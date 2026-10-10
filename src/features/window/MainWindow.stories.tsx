import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "../../design/ThemePair";
import { MainWindow, type MainWindowProps } from "./MainWindow";

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

export const FiveAgents: StoryObj = stacked("The 5-agent scenario: two need you, one working, one done, one error", { width: 1180, height: 800 });
export const SplitView: StoryObj = stacked("Split view: the active pane has the accent bar, the other header recedes, terminal text is never dimmed", { width: 1180, height: 800, layout: "split" });
export const FocusMode: StoryObj = stacked("Focus mode: sidebar and header gone, a quiet pill top right", { width: 1180, height: 800, layout: "focus" });
export const FocusModeHeaderRevealed: StoryObj = stacked("Focus mode with the pointer on the top edge: the header slides in", { width: 1180, height: 800, layout: "focus", headerPeek: true });
export const CollapsedRail: StoryObj = stacked("Collapsed rail (52 px): stripe, glyph and monogram, needs-you badge stack on top", { width: 1180, height: 800, layout: "rail" });
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
export const MacShortcuts: StoryObj = stacked("macOS shortcut labels (focus pill)", { width: 1180, height: 560, layout: "focus", os: "mac" });
