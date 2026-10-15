import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "./ThemePair";
import "./specimen.css";

const meta: Meta = { title: "Design/Type" };
export default meta;

const scale: Array<[string, number, string]> = [
  ["--text-28", 600, "3 sessions need you"],
  ["--text-20", 600, "No sessions yet. Start one with Ctrl+Shift+T."],
  ["--text-15", 500, "api-server has waited 1 min to run a command"],
  ["--text-13", 500, "Wants to run npm test"],
  ["--text-13", 400, "Marshell adds 9 entries to your Claude user settings so it can see status and approvals."],
  ["--text-12", 400, "Editing src/auth.ts · 2 subagents"],
  ["--text-11", 400, "84k / 200k · 312k in · 48k out · $1.20"],
];

export const Scale: StoryObj = {
  render: () => (
    <ThemePair label="Type scale">
      {scale.map(([size, weight, text]) => (
        <div className="type-row" key={size + weight + text}>
          <span className="type-row__meta">
            {size.replace("--text-", "")} / {weight}
          </span>
          <span style={{ fontSize: `var(${size})`, fontWeight: weight }}>{text}</span>
        </div>
      ))}
    </ThemePair>
  ),
};

export const TabularFigures: StoryObj = {
  render: () => (
    <ThemePair label="Tabular figures">
      {["1m 05s", "11m 40s", "1h 02m", "84k / 200k", "1.2M / 1M", "$1.20", "$1,234.56"].map((s) => (
        <div key={s} style={{ fontVariantNumeric: "tabular-nums", fontSize: "var(--text-11)", textAlign: "end", inlineSize: 120 }}>
          {s}
        </div>
      ))}
    </ThemePair>
  ),
};

export const Mono: StoryObj = {
  render: () => (
    <ThemePair label="Mono">
      <pre style={{ margin: 0, fontFamily: "var(--font-mono)", fontSize: "var(--text-12)", whiteSpace: "pre-wrap" }}>
        {"rm -rf ./dist && npm run build -- --mode production\nBash(npm test:*)"}
      </pre>
    </ThemePair>
  ),
};
