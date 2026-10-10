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

const IN_AND_OUT = [
  ["breathe", "Breathe: the whole burst shrinks and grows"],
  ["retract", "Retract: all rays pull in together"],
  ["alternate", "Alternate: odd rays in while even rays out"],
  ["wave", "Wave: in and out, one ray after another"],
  ["deep", "Deep wave: rays nearly vanish"],
] as const;

/** Claude's working loop as in-and-out motions, to choose from. */
export const ClaudeInAndOut: StoryObj = {
  render: () => (
    <ThemePair label="Claude: in-and-out options">
      <div style={grid}>
        {IN_AND_OUT.map(([loop, text]) => (
          <figure key={loop} style={{ ...cell, margin: 0 }}>
            <AgentMark agent="claude" size={20} working loop={loop} />
            <AgentMark agent="claude" size={16} working loop={loop} />
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
