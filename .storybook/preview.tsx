import type { Decorator, Preview } from "@storybook/react-vite";
import { withThemeByDataAttribute } from "@storybook/addon-themes";
import "../src/styles/tokens.css";
import "../src/styles/accents.css";
import "../src/styles/base.css";
import "../src/styles/motion.css";

const ACCENTS = ["amber", "blue", "indigo", "violet", "magenta", "cyan", "teal", "slate"] as const;

const withAccentAndMotion: Decorator = (Story, ctx) => {
  const root = document.documentElement;
  root.dataset.accent = String(ctx.globals.accent ?? "amber");
  if (ctx.globals.motion === "reduce") root.dataset.motion = "reduce";
  else delete root.dataset.motion;
  return <Story />;
};

const preview: Preview = {
  globalTypes: {
    accent: {
      description: "Accent",
      toolbar: { title: "Accent", icon: "paintbrush", items: [...ACCENTS], dynamicTitle: true },
    },
    motion: {
      description: "Motion",
      toolbar: { title: "Motion", icon: "play", items: ["full", "reduce"], dynamicTitle: true },
    },
  },
  initialGlobals: { accent: "amber", motion: "full" },
  decorators: [
    withThemeByDataAttribute({
      themes: { dark: "dark", light: "light" },
      defaultTheme: "dark",
      attributeName: "data-theme",
    }),
    withAccentAndMotion,
  ],
  parameters: {
    layout: "padded",
    backgrounds: { disable: true },
    a11y: { test: "todo" },
  },
};
export default preview;
