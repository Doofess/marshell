import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "./ThemePair";
import "./specimen.css";

const meta: Meta = { title: "Design/Space and motion" };
export default meta;

export const Space: StoryObj = {
  render: () => (
    <ThemePair label="Space">
      {["--space-1", "--space-2", "--space-3", "--space-4", "--space-6", "--space-8"].map((s) => (
        <div className="type-row" key={s}>
          <span className="type-row__meta">{s}</span>
          <span style={{ display: "block", blockSize: 8, inlineSize: `var(${s})`, background: "var(--text-2)" }} />
        </div>
      ))}
    </ThemePair>
  ),
};

const durations = [
  "--dur-palette",
  "--dur-peek",
  "--dur-exit",
  "--dur-enter",
  "--dur-check",
  "--dur-drawer",
  "--dur-bounce",
  "--dur-flash",
  "--dur-pulse",
];

export const MotionTokens: StoryObj = {
  render: () => (
    <ThemePair label="Motion tokens">
      {durations.map((d) => (
        <div className="type-row" key={d}>
          <span className="type-row__meta">{d}</span>
          <code style={{ fontSize: "var(--text-12)" }}>
            {getComputedStyle(document.documentElement).getPropertyValue(d)}
          </code>
        </div>
      ))}
    </ThemePair>
  ),
};
