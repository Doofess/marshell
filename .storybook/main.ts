import type { StorybookConfig } from "@storybook/react-vite";

const config: StorybookConfig = {
  stories: ["../src/**/*.stories.@(ts|tsx)"],
  addons: ["@storybook/addon-a11y", "@storybook/addon-themes"],
  framework: { name: "@storybook/react-vite", options: {} },
  // The app's vite.config pins port 1420 with strictPort for Tauri; Storybook serves on its own port.
  // Storybook sets its own build target, so the app's cssTarget (native light-dark() and oklch()) is repeated here.
  viteFinal: async (cfg) => ({
    ...cfg,
    server: { ...cfg.server, port: undefined, strictPort: false },
    build: { ...cfg.build, cssTarget: ["chrome123", "safari17.5"] },
  }),
};
export default config;
