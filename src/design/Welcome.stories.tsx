import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "./ThemePair";

const meta: Meta = { title: "Design/Welcome" };
export default meta;

export const Welcome: StoryObj = {
  render: () => (
    <ThemePair label="Marshell design system">
      <h1 style={{ margin: 0, fontSize: "var(--text-20)", fontWeight: 600 }}>Marshell design system</h1>
      <p style={{ color: "var(--text-2)" }}>
        Batch 1: tokens, status glyphs and sidebar rows. Use the toolbar to switch theme, accent and motion.
      </p>
    </ThemePair>
  ),
};
