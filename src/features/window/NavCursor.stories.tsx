import type { Meta, StoryObj } from "@storybook/react-vite";
import { NavDemo } from "../../design/NavDemo";
import { ThemePair } from "../../design/ThemePair";
import type { Os } from "../../lib/keymap";
import { GRID_CONTEXT } from "../../lib/navigation/fixtures";
import { run } from "../../lib/navigation/machine";
import { INITIAL_NAV, type NavContext, type NavEvent, type NavState } from "../../lib/navigation/types";
import { ShortcutSheet } from "../shortcuts/ShortcutSheet";

const meta: Meta = { title: "Keyboard navigation", parameters: { layout: "fullscreen" } };
export default meta;

const DOWN: NavEvent = { type: "modifier-down" };
const arrow = (dir: "left" | "right" | "up" | "down"): NavEvent => ({ type: "arrow", dir });
const start = (selectedPane: number | null): NavState => ({ ...INITIAL_NAV, selectedPane });
const demo = (state: NavState, ctx: NavContext = GRID_CONTEXT, os: Os = "windows") => <NavDemo state={state} ctx={ctx} os={os} />;

export const NoneSelected: StoryObj = {
  render: () => (
    <ThemePair label="Nothing selected (Esc, or the select-none chord): no pane is lit, the accent is neutral, typing goes nowhere, and the status bar says how to start. Tab alone, with nothing focused, selects the first terminal" stack>
      {demo(INITIAL_NAV)}
    </ThemePair>
  ),
};

export const TabSelectsTheNextTerminal: StoryObj = {
  render: () => (
    <ThemePair label="Hold Ctrl+Shift and tap Tab: the next terminal on screen is selected at once (here from the first to the second). It wraps after the last, and it never lands on the sidebar" stack>
      {demo(run(start(0), [DOWN, { type: "tab" }], GRID_CONTEXT).state)}
    </ThemePair>
  ),
};

export const HeldOnASidebarRow: StoryObj = {
  render: () => (
    <ThemePair label="Hold Ctrl+Shift and press ← then ↓ ↓: the cursor walks the sidebar list. Releasing selects session 3, which is on screen, so its pane is selected" stack>
      {demo(run(start(0), [DOWN, arrow("left"), arrow("down"), arrow("down")], GRID_CONTEXT).state)}
    </ThemePair>
  ),
};

export const HeldOnASessionNotOnScreen: StoryObj = {
  render: () => (
    <ThemePair label="The cursor on session 5, which is only in the list. Releasing brings it into the focused pane (the first empty slot if none is focused) and selects it; the session that was there keeps running" stack>
      {demo(run(start(0), [DOWN, arrow("left"), arrow("down"), arrow("down"), arrow("down")], GRID_CONTEXT).state)}
    </ThemePair>
  ),
};

export const HeldAcrossPanes: StoryObj = {
  render: () => (
    <ThemePair label="Arrows move across the four panes by where they are on screen: from the first pane, → then ↓ reaches the fourth" stack>
      {demo(run(start(0), [DOWN, arrow("right"), arrow("down")], GRID_CONTEXT).state)}
    </ThemePair>
  ),
};

export const HeldOnTheTopBar: StoryObj = {
  render: () => (
    <ThemePair label="↑ from the first row of panes reaches the top bar; releasing activates the control (here the layout button) and leaves the selection where it was" stack>
      {demo(run(start(1), [DOWN, arrow("up")], GRID_CONTEXT).state)}
    </ThemePair>
  ),
};

export const MacOs: StoryObj = {
  render: () => (
    <ThemePair label="On macOS the same chord is ⌃⇧ (Cmd+Tab is the system's app switcher and never reaches an app)" stack>
      {demo(run(start(0), [DOWN, arrow("left")], GRID_CONTEXT).state, GRID_CONTEXT, "mac")}
    </ThemePair>
  ),
};

const sheet = (os: Os) => (
  <ThemePair label={`The cheat sheet on ${os === "mac" ? "macOS" : os === "linux" ? "Linux" : "Windows"}`} stack>
    <div style={{ padding: 16 }}>
      <ShortcutSheet os={os} />
    </div>
  </ThemePair>
);
export const ShortcutsWindows: StoryObj = { render: () => sheet("windows") };
export const ShortcutsMac: StoryObj = { render: () => sheet("mac") };
export const ShortcutsLinux: StoryObj = { render: () => sheet("linux") };
