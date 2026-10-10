import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { ContextRing } from "../ContextRing/ContextRing";
import { ThemePair } from "../../design/ThemePair";
import { AUX_LABELS, GLYPHS, type AuxKind, type GlyphKind } from "./glyphs";
import { AuxGlyph, StatusGlyph } from "./StatusGlyph";
import "../../design/specimen.css";

const meta: Meta = { title: "Components/Status glyphs" };
export default meta;

const kinds = Object.keys(GLYPHS) as GlyphKind[];

export const Set: StoryObj = {
  render: () => (
    <ThemePair label="Status glyph set">
      <table className="matrix">
        <thead>
          <tr>
            <th>State</th>
            <th>16 px</th>
            <th>12 px</th>
            <th>Motion</th>
            <th>Reduced motion</th>
          </tr>
        </thead>
        <tbody>
          {kinds.map((k) => (
            <tr key={k}>
              <td>{GLYPHS[k].label}</td>
              <td>
                <StatusGlyph kind={k} />
              </td>
              <td>
                <StatusGlyph kind={k} size={12} />
              </td>
              <td>{GLYPHS[k].motion ?? "–"}</td>
              <td>{GLYPHS[k].reduced ?? "–"}</td>
            </tr>
          ))}
          {(Object.keys(AUX_LABELS) as AuxKind[]).map((k) => (
            <tr key={k}>
              <td>{AUX_LABELS[k]}</td>
              <td colSpan={2}>
                <AuxGlyph kind={k} />
              </td>
              <td>–</td>
              <td>–</td>
            </tr>
          ))}
        </tbody>
      </table>
    </ThemePair>
  ),
};

/** Remounts the one-shot animations so they can be watched again; set the toolbar Motion to "reduce" to compare. */
export const MotionReplay: StoryObj = {
  render: function Replay() {
    const [n, setN] = useState(0);
    return (
      <ThemePair label="Motion">
        <button type="button" onClick={() => setN(n + 1)}>
          Replay
        </button>
        <div key={n} style={{ display: "flex", gap: "var(--space-6)", marginBlockStart: "var(--space-3)", alignItems: "center" }}>
          {(["working", "needs-permission", "needs-question", "done-unseen", "error"] as GlyphKind[]).map((k) => (
            <figure key={k} style={{ margin: 0, display: "grid", justifyItems: "center", gap: "var(--space-1)" }}>
              <StatusGlyph kind={k} />
              <figcaption style={{ fontSize: "var(--text-11)", color: "var(--text-3)" }}>{GLYPHS[k].label}</figcaption>
            </figure>
          ))}
        </div>
      </ThemePair>
    );
  },
};

export const ContextRings: StoryObj = {
  render: () => (
    <ThemePair label="Context ring">
      <div style={{ display: "flex", gap: "var(--space-4)", alignItems: "center" }}>
        {[0, 12, 42, 79, 80, 94, 95, 100].map((p) => (
          <figure key={p} style={{ margin: 0, display: "grid", justifyItems: "center", gap: "var(--space-1)" }}>
            <ContextRing pct={p} />
            <figcaption style={{ fontSize: "var(--text-11)", color: "var(--text-3)", fontVariantNumeric: "tabular-nums" }}>{p}%</figcaption>
          </figure>
        ))}
      </div>
    </ThemePair>
  ),
};
