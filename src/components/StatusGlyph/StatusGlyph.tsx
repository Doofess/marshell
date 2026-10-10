import type { ReactNode } from "react";
import { AUX_LABELS, GLYPHS, type AuxKind, type GlyphKind } from "./glyphs";
import "./StatusGlyph.css";

// All shapes are drawn on a 16-unit grid; colour comes from CSS (StatusGlyph.css) via currentColor and tokens.
const shapes: Record<GlyphKind, ReactNode> = {
  idle: <circle cx="8" cy="8" r="5.25" fill="none" stroke="currentColor" strokeWidth="1.5" />,
  working: (
    <>
      <circle className="g-working-arc" cx="8" cy="8" r="6.5" fill="none" stroke="currentColor" strokeWidth="1.25" pathLength="4" strokeDasharray="3 1" />
      <circle className="g-working-dot" cx="8" cy="8" r="4" fill="currentColor" />
    </>
  ),
  "needs-permission": (
    <>
      <rect className="g-badge" x="1.5" y="1.5" width="13" height="13" rx="4" fill="var(--accent)" />
      <path className="g-mark g-bang" d="M8 4.5v4.25" fill="none" stroke="var(--accent-ink)" strokeWidth="1.75" strokeLinecap="round" />
      <circle className="g-mark g-mark--fill" cx="8" cy="11.4" r="1" fill="var(--accent-ink)" />
    </>
  ),
  "needs-question": (
    <>
      <rect className="g-badge" x="1.5" y="1.5" width="13" height="13" rx="4" fill="var(--accent)" />
      <path
        className="g-mark"
        d="M6.25 6.25a1.75 1.75 0 1 1 2.6 1.53c-.5.28-.85.62-.85 1.22v.25"
        fill="none"
        stroke="var(--accent-ink)"
        strokeWidth="1.75"
        strokeLinecap="round"
      />
      <circle className="g-mark g-mark--fill" cx="8" cy="11.4" r="1" fill="var(--accent-ink)" />
    </>
  ),
  "done-unseen": (
    <path className="g-check" d="M3.75 8.25l2.75 2.75L12.25 5.25" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" pathLength="1" />
  ),
  "done-seen": (
    <path d="M3.75 8.25l2.75 2.75L12.25 5.25" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" />
  ),
  error: (
    <>
      <circle className="g-error-disc" cx="8" cy="8" r="6.5" fill="currentColor" />
      <path className="g-cut" d="M5.9 5.9l4.2 4.2M10.1 5.9l-4.2 4.2" fill="none" stroke="var(--bg-base)" strokeWidth="1.6" strokeLinecap="round" />
    </>
  ),
  stuck: <circle cx="8" cy="8" r="5.25" fill="none" stroke="currentColor" strokeWidth="1.5" strokeDasharray="2.2 2" />,
  ended: <rect x="3.25" y="3.25" width="9.5" height="9.5" rx="1.5" fill="none" stroke="currentColor" strokeWidth="1.5" />,
  limited: (
    <circle cx="8" cy="8" r="5.25" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeDasharray="0.01 2.75" />
  ),
};

export function StatusGlyph({ kind, size = 16, label }: { kind: GlyphKind; size?: 12 | 16; label?: string }) {
  return (
    <svg
      className="status-glyph"
      data-kind={kind}
      role="img"
      aria-label={label ?? GLYPHS[kind].label}
      width={size}
      height={size}
      viewBox="0 0 16 16"
    >
      {shapes[kind]}
    </svg>
  );
}

const aux: Record<AuxKind, ReactNode> = {
  // Bell with a slash.
  muted: (
    <g fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M4.5 11.5h7l-1-1.5V7a2.5 2.5 0 0 0-4-2" />
      <path d="M5.5 7v3l-1 1.5M7 13.25h2M2.5 2.5l11 11" />
    </g>
  ),
  // Shield.
  elevated: (
    <path d="M8 1.75l5 2v4c0 3.1-2.1 5.4-5 6.5-2.9-1.1-5-3.4-5-6.5v-4l5-2z" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" />
  ),
  // Warning triangle.
  caution: (
    <>
      <path d="M8 2l6.25 11H1.75L8 2z" fill="currentColor" />
      <path className="g-cut" d="M8 6.25v3" stroke="var(--bg-base)" strokeWidth="1.5" strokeLinecap="round" />
      <circle className="g-cut g-cut--fill" cx="8" cy="11.1" r="0.85" fill="var(--bg-base)" />
    </>
  ),
};

export function AuxGlyph({ kind, label }: { kind: AuxKind; label?: string }) {
  return (
    <svg className="aux-glyph" data-kind={kind} role="img" aria-label={label ?? AUX_LABELS[kind]} width={12} height={12} viewBox="0 0 16 16">
      {aux[kind]}
    </svg>
  );
}
