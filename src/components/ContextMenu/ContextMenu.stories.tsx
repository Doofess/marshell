import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "../../design/ThemePair";
import { ContextMenu, type MenuItem } from "./ContextMenu";

const meta: Meta = { title: "Context menu", parameters: { layout: "fullscreen" } };
export default meta;

const ITEMS: MenuItem[] = [
  { id: "rename", label: "Rename", shortcut: "F2" },
  { id: "resume", label: "Resume" },
  { id: "review", label: "Review changes", shortcut: "Ctrl+Shift+R" },
  { separator: true },
  { id: "close", label: "Close", shortcut: "Ctrl+Shift+W" },
];

const noop = () => {};

export const Default: StoryObj = {
  render: () => (
    <ThemePair label="Session row menu: Rename, Resume, Review changes, a rule, then Close; the first item is active" stack>
      {/* A transform makes this the containing block, so the menu's fixed position stays inside the frame. */}
      <div style={{ blockSize: 240, position: "relative", transform: "translate(0)", background: "var(--bg-base)" }}>
        <ContextMenu items={ITEMS} label="Session actions" anchor={{ x: 24, y: 24 }} onRun={noop} onClose={noop} />
      </div>
    </ThemePair>
  ),
};

export const AdministratorRowMenu: StoryObj = {
  render: () => (
    <ThemePair label="A session running as administrator: Pause stays listed, disabled, with the reason" stack>
      <div style={{ blockSize: 260, position: "relative", transform: "translate(0)", background: "var(--bg-base)" }}>
        <ContextMenu
          items={[{ id: "rename", label: "Rename", shortcut: "F2" }, { id: "pause", label: "Pause", disabled: "Pause isn't available for a session running as administrator." }, { separator: true }, { id: "close", label: "Close", shortcut: "Ctrl+Shift+W" }]}
          label="Session actions"
          anchor={{ x: 24, y: 24 }}
          onRun={noop}
          onClose={noop}
        />
      </div>
    </ThemePair>
  ),
};
