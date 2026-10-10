import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "../../design/ThemePair";
import { StatusGlyph } from "../StatusGlyph/StatusGlyph";
import { AGENT_IDS, AGENT_NAMES } from "./agents";
import { AgentMark } from "./AgentMark";

const meta: Meta = { title: "Agent marks" };
export default meta;

const grid = { display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(92px, 1fr))", gap: "var(--space-4)" } as const;
const cell = { display: "grid", justifyItems: "center", gap: "var(--space-2)", fontSize: "var(--text-11)", color: "var(--text-2)", textAlign: "center" } as const;

/** Every CLI the app knows, in its own colour, at the row size and larger. */
export const Marks: StoryObj = {
  render: () => (
    <ThemePair label="Vendor marks">
      <div style={grid}>
        {AGENT_IDS.map((id) => (
          <figure key={id} style={{ ...cell, margin: 0 }}>
            <AgentMark agent={id} size={20} />
            <AgentMark agent={id} size={16} />
            <figcaption>{AGENT_NAMES[id]}</figcaption>
          </figure>
        ))}
      </div>
    </ThemePair>
  ),
};

/** While a session works, its mark moves with its own loop. With reduced motion the mark stays still. */
export const WorkingLoops: StoryObj = {
  render: () => (
    <ThemePair label="Working: one loop per vendor">
      <div style={grid}>
        {AGENT_IDS.map((id) => (
          <figure key={id} style={{ ...cell, margin: 0 }}>
            <AgentMark agent={id} size={20} working />
            <AgentMark agent={id} size={16} working />
            <figcaption>{AGENT_NAMES[id]}</figcaption>
          </figure>
        ))}
      </div>
    </ThemePair>
  ),
};

const GENERIC_OPTIONS = [
  ["blink", "Blink: only the underscore blinks"],
  ["typing", "Typing: the underscore is typed out a step at a time, then starts again"],
  ["prompt", "Prompt: the chevron pulses while the underscore blinks"],
  ["frame", "Frame: the terminal frame breathes, the prompt holds still"],
] as const;

/** The custom agent's working loop, with the parts of its mark moving separately. */
export const CustomAgentOptions: StoryObj = {
  render: () => (
    <ThemePair label="Custom agent: options">
      <div style={grid}>
        {GENERIC_OPTIONS.map(([option, text]) => (
          <figure key={option} style={{ ...cell, margin: 0 }}>
            <AgentMark agent="generic" size={20} working option={option} />
            <AgentMark agent="generic" size={16} working option={option} />
            <figcaption>{text}</figcaption>
          </figure>
        ))}
      </div>
    </ThemePair>
  ),
};

/** The needs-you and error glyphs take the vendor's colour, so a list stays one family of colours. */
export const NeedsYouInVendorColours: StoryObj = {
  render: () => (
    <ThemePair label="Needs you and error, in each vendor's colour">
      <div style={grid}>
        {AGENT_IDS.map((id) => (
          <figure key={id} data-agent={id} style={{ ...cell, margin: 0 }}>
            <span style={{ display: "flex", gap: "var(--space-2)", alignItems: "center" }}>
              <StatusGlyph kind="needs-permission" />
              <StatusGlyph kind="needs-question" />
              <StatusGlyph kind="error" />
            </span>
            <figcaption>{AGENT_NAMES[id]}</figcaption>
          </figure>
        ))}
      </div>
    </ThemePair>
  ),
};
