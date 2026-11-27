import type { Decorator, Preview } from "@storybook/react-vite";
import { withThemeByDataAttribute } from "@storybook/addon-themes";
import "../src/styles/tokens.css";
import "../src/styles/accents.css";
import "../src/styles/scrollbars.css";
import "../src/styles/base.css";
import "../src/styles/motion.css";
import { syncPageHidden } from "../src/design/visibility";

syncPageHidden(document);

const withMotion: Decorator = (Story, ctx) => {
  const root = document.documentElement;
  if (ctx.globals.motion === "reduce") root.dataset.motion = "reduce";
  else delete root.dataset.motion;
  return <Story />;
};

const preview: Preview = {
  globalTypes: {
    motion: {
      description: "Motion",
      toolbar: { title: "Motion", icon: "play", items: ["full", "reduce"], dynamicTitle: true },
    },
  },
  initialGlobals: { motion: "full" },
  decorators: [
    withThemeByDataAttribute({
      themes: { dark: "dark", light: "light" },
      defaultTheme: "dark",
      attributeName: "data-theme",
    }),
    withMotion,
  ],
  parameters: {
    layout: "padded",
    backgrounds: { disable: true },
    a11y: { test: "error" },
  },
};
export default preview;
