import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "../../design/ThemePair";
import { AppearanceView, type AppearanceHandlers, type AppearanceViewState } from "./AppearanceDialog";
import { DEFAULT_APPEARANCE } from "./rootState";

const meta: Meta = { title: "Settings/Appearance", parameters: { layout: "fullscreen" } };
export default meta;

const noop = () => {};
const handlers: AppearanceHandlers = { save: noop, onAccentText: noop, commitAccent: noop, onFontText: noop, commitFont: noop, onSizeText: noop, commitSize: noop, importFile: noop, onClose: noop };
const state = (over: Partial<AppearanceViewState> = {}): AppearanceViewState => ({
  appearance: DEFAULT_APPEARANCE,
  app: "dark",
  wtFiles: [],
  error: null,
  accent: { text: "", reason: null },
  font: "",
  size: { text: "13", reason: null },
  importError: null,
  ...over,
});
const show = (label: string, s: AppearanceViewState) => (
  <ThemePair label={label} stack>
    <div style={{ inlineSize: 760, maxInlineSize: "100%", blockSize: 1500 }}>
      <AppearanceView state={s} handlers={handlers} />
    </div>
  </ThemePair>
);

const WT_SETTINGS = {
  path: "C:\\Users\\me\\AppData\\Local\\Packages\\Microsoft.WindowsTerminal_8wekyb3d8bbwe\\LocalState\\settings.json",
  text: JSON.stringify({
    profiles: { defaults: { font: { face: "Cascadia Code", size: 11 } } },
    schemes: [
      { name: "Campbell", foreground: "#cccccc", background: "#0c0c0c", black: "#0c0c0c", red: "#c50f1f", green: "#13a10e", yellow: "#c19c00", blue: "#0037da", purple: "#881798", cyan: "#3a96dd", white: "#cccccc", brightBlack: "#767676", brightRed: "#e74856", brightGreen: "#16c60c", brightYellow: "#f9f1a5", brightBlue: "#3b78ff", brightPurple: "#b4009e", brightCyan: "#61d6d6", brightWhite: "#f2f2f2" },
      { name: "One Half Light", foreground: "#383a42", background: "#fafafa", black: "#383a42", red: "#e45649", green: "#50a14f", yellow: "#c18301", blue: "#0184bc", purple: "#a626a4", cyan: "#0997b3", white: "#fafafa", brightBlack: "#4f525e", brightRed: "#e06c75", brightGreen: "#98c379", brightYellow: "#e5c07b", brightBlue: "#61afef", brightPurple: "#c678dd", brightCyan: "#56b6c2", brightWhite: "#ffffff" },
    ],
  }),
};

export const Default: StoryObj = { render: () => show("The defaults: follow the system, amber, the bundled font, no Windows Terminal found", state()) };
export const CustomAccentRefused: StoryObj = {
  render: () => show("A custom accent that fails the check: the reason shows and amber stays", state({ accent: { text: "#ffff66", reason: "Too light for the light theme: 1.1:1 against the background, needs 3:1." } })),
};
export const WithWindowsTerminal: StoryObj = {
  render: () =>
    show(
      "Windows Terminal found: its font is used, and its schemes can be added (Campbell already is)",
      state({
        wtFiles: [WT_SETTINGS],
        appearance: {
          ...DEFAULT_APPEARANCE,
          terminal_theme: "wt:campbell",
          imported_themes: [{ id: "wt:campbell", name: "Campbell", source: "windows-terminal", colors: { foreground: "#cccccc", background: "#0c0c0c", cursor: "#ffffff", selection: "#3a3a3a", ansi: ["#0c0c0c", "#c50f1f", "#13a10e", "#c19c00", "#0037da", "#881798", "#3a96dd", "#cccccc", "#767676", "#e74856", "#16c60c", "#f9f1a5", "#3b78ff", "#b4009e", "#61d6d6", "#f2f2f2"] } }],
        },
      }),
    ),
};
