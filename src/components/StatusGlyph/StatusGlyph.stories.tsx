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
    <ThemePair label="Context ring (Gemini blue; thicker from 80%, a centre dot from 95%)">
      <div data-agent="gemini" style={{ display: "flex", gap: "var(--space-4)", alignItems: "center" }}>
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

/** Candidates for the needs-permission mark inside the vendor badge, next to the current exclamation mark. */
const PERMISSION_MARKS = [
  ["Exclamation (now)", <><path d="M8 4.5v4.25" fill="none" strokeWidth="1.75" strokeLinecap="round" /><circle cx="8" cy="11.4" r="1" /></>],
  ["Key", <><circle cx="6" cy="6.2" r="2.1" fill="none" strokeWidth="1.5" /><path d="M7.5 7.7L12 12.2M10.3 10.5l1.6-1.6" fill="none" strokeWidth="1.5" strokeLinecap="round" /></>],
  ["Padlock", <><rect x="4.9" y="7.4" width="6.2" height="5" rx="1.1" /><path d="M6.4 7.4V6a1.6 1.6 0 0 1 3.2 0v1.4" fill="none" strokeWidth="1.5" strokeLinecap="round" /></>],
  ["Shield", <path d="M8 3.4l3.6 1.4v3.1c0 2.3-1.5 3.8-3.6 4.6-2.1-.8-3.6-2.3-3.6-4.6V4.8L8 3.4z" fill="none" strokeWidth="1.5" strokeLinejoin="round" />],
  ["Held (pause)", <path d="M6.2 4.8v6.4M9.8 4.8v6.4" fill="none" strokeWidth="2" strokeLinecap="round" />],
] as const;

export const PermissionMarks: StoryObj = {
  render: () => (
    <ThemePair label="Needs permission: candidate marks, in two vendors' colours">
      <div style={{ display: "flex", gap: "var(--space-6)", alignItems: "flex-start", flexWrap: "wrap" }}>
        {PERMISSION_MARKS.map(([name, mark]) => (
          <figure key={name} style={{ margin: 0, display: "grid", justifyItems: "center", gap: "var(--space-2)" }}>
            {(["claude", "gemini"] as const).map((vendor) => (
              <span key={vendor} data-agent={vendor} style={{ display: "flex", gap: "var(--space-3)", alignItems: "center" }}>
                {([16, 12] as const).map((size) => (
                  <svg key={size} width={size} height={size} viewBox="0 0 16 16" role="img" aria-label={name}>
                    <rect x="1.5" y="1.5" width="13" height="13" rx="4" fill="var(--agent-signal, var(--accent))" />
                    <g fill="var(--agent-ink, var(--accent-ink))" stroke="var(--agent-ink, var(--accent-ink))">
                      {mark}
                    </g>
                  </svg>
                ))}
              </span>
            ))}
            <figcaption style={{ fontSize: "var(--text-11)", color: "var(--text-3)" }}>{name}</figcaption>
          </figure>
        ))}
      </div>
    </ThemePair>
  ),
};
