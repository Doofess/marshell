import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "../../design/ThemePair";
import { RestartBar } from "./RestartBar";

const meta: Meta = { title: "Restart bar", parameters: { layout: "fullscreen" } };
export default meta;

const noop = () => {};

export const AdministratorBypass: StoryObj = {
  render: () => (
    <ThemePair label="Bypass for an agent running as administrator: Restart waits for the checkbox" stack>
      <RestartBar text="Restart 3 infra in Bypass? It ends and runs claude --continue --permission-mode bypassPermissions." needsAck onCancel={noop} onRestart={noop} />
    </ThemePair>
  ),
};
