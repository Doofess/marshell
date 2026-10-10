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

// 880 tall so the lane (header, the 256 px target card and one 36 px row = 320) fits inside its 40% cap of the 840 px sidebar.
export const FiveAgents: StoryObj = stacked("The 5-agent scenario: two need you, one working, one done, one error", { width: 1180, height: 880 });
export const SplitTwo: StoryObj = stacked("Split, two terminals: side by side in a wide window. The active pane has the accent bar, the other header recedes, terminal text is never dimmed", { width: 1500, height: 800, layout: "split" });
export const SplitTwoStacked: StoryObj = stacked("The same two in a 1180 wide window: side by side would leave 53 columns each, so the app stacks them (109 columns, 16 rows). Auto never starves a pane", { width: 1180, height: 800, layout: "split" });
export const SplitThree: StoryObj = stacked("Split, three: one main pane beside two stacked, picked by the app for this window. A pane that needs you keeps its tint at full strength", { width: 1500, height: 860, layout: "split", splitCount: 3 });
export const SplitFour: StoryObj = stacked("Split, four: a two by two grid. Each pane header shows its glyph, so the two that need you are visible without the sidebar", { width: 1700, height: 900, layout: "split", splitCount: 4, activePane: 2 });
export const SplitFourTooSmall: StoryObj = stacked("Asking for four in the 720 by 480 minimum shows one: four panes there would each be a sliver, so the app shows what is usable. The other three keep running, and the layout menu says so and turns the larger counts off", { width: 720, height: 480, layout: "split", splitCount: 4, layoutMenu: true });
export const SplitThreeCappedAtTwo: StoryObj = stacked("Asking for three in a 1180 by 800 window shows two, stacked: three would leave a pane under 60 columns or 14 rows. The header of the focused pane says one more is running", { width: 1180, height: 800, layout: "split", splitCount: 3 });
export const SplitNarrowWarning: StoryObj = stacked("A pane between 60 and 79 columns is allowed but says its size (73 by 18) and how to zoom: three panes in a 1500 wide window", { width: 1500, height: 860, layout: "split", splitCount: 3, arrangement: "columns" });
export const SplitStackedTall: StoryObj = stacked("A tall, narrow window stacks the panes instead, so none is starved of columns", { width: 760, height: 1000, layout: "split", splitCount: 3 });
export const SplitZoomed: StoryObj = stacked("Zoomed: one pane fills the view; the other three keep running and the header says so. The same button restores them", { width: 1500, height: 860, layout: "split", splitCount: 4, zoomedPane: 1 });
export const SplitEmptyPane: StoryObj = stacked("A new pane starts empty: choose a session that is not on screen, or start one", { width: 1500, height: 860, layout: "split", splitCount: 3, emptySlot: true, activePane: 2 });
export const SplitDividerMoved: StoryObj = stacked("A dragged divider (70/30): the app keeps the arrangement while the user's hand is on it", { width: 1500, height: 700, layout: "split", ratios: { col: 0.7 } });
export const LayoutMenuOpen: StoryObj = stacked("The layout menu: how many terminals you see at once (any number can run), how they are arranged, and the keys", { width: 1500, height: 760, layout: "split", splitCount: 3, layoutMenu: true });
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
