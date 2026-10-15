import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "../../design/ThemePair";
import { Logo } from "./Logo";
import { Wordmark } from "./Wordmark";

const meta: Meta = { title: "App icon", parameters: { layout: "fullscreen" } };
export default meta;

const SIZES = [16, 22, 32, 64, 128];

export const Sizes: StoryObj = {
  render: () => (
    <ThemePair label="The app icon (Night shift) at the sizes it appears: bar, tray, taskbar, dock. The tile is the same in both themes" stack>
      <div style={{ display: "flex", alignItems: "flex-end", gap: 24, padding: 24 }}>
        {SIZES.map((n) => (
          <figure key={n} style={{ margin: 0, display: "grid", justifyItems: "center", gap: 8 }}>
            <Logo size={n} />
            <figcaption style={{ fontSize: 11 }}>{n} px</figcaption>
          </figure>
        ))}
      </div>
    </ThemePair>
  ),
};

export const Lockup: StoryObj = {
  render: () => (
    <ThemePair label="Icon and wordmark together, as in the window's top bar. The name is vector letters in Space Grotesk SemiBold, in the text colour" stack>
      <div style={{ display: "flex", alignItems: "center", gap: 48, padding: 24 }}>
        {[1, 1.6, 2.6].map((k) => (
          <div key={k} style={{ display: "flex", alignItems: "center", gap: 8 * k }}>
            <Logo size={Math.round(22 * k)} decorative />
            <Wordmark height={Math.round(13 * k)} decorative />
          </div>
        ))}
      </div>
    </ThemePair>
  ),
};
