import type { Meta, StoryObj } from "@storybook/react-vite";
import { ThemePair } from "../../design/ThemePair";
import { ContextRing } from "../ContextRing/ContextRing";
import { AuxGlyph, StatusGlyph } from "../StatusGlyph/StatusGlyph";
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

/** Needs-you, error, done, caution and the context ring all take the vendor's colour, so a list stays one family of colours. */
export const StatusInVendorColours: StoryObj = {
  render: () => (
    <ThemePair label="Status glyphs and context ring, in each vendor's colour">
      <div style={{ ...grid, gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))" }}>
        {AGENT_IDS.map((id) => (
          <figure key={id} data-agent={id} style={{ ...cell, margin: 0 }}>
            <span style={{ display: "flex", gap: "var(--space-2)", alignItems: "center" }}>
              <StatusGlyph kind="needs-permission" />
              <StatusGlyph kind="needs-question" />
              <StatusGlyph kind="error" />
              <StatusGlyph kind="done-unseen" />
              <StatusGlyph kind="done-seen" />
            </span>
            <span style={{ display: "flex", gap: "var(--space-2)", alignItems: "center" }}>
              <AuxGlyph kind="caution" />
              <ContextRing pct={42} />
              <ContextRing pct={85} />
              <ContextRing pct={97} />
            </span>
            <figcaption>{AGENT_NAMES[id]}</figcaption>
          </figure>
        ))}
      </div>
    </ThemePair>
  ),
};
