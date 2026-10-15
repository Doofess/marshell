import "./ContextRing.css";

export type RingTone = "neutral" | "caution" | "error";

export function ringTone(pct: number): RingTone {
  if (pct >= 95) return "error";
  if (pct >= 80) return "caution";
  return "neutral";
}

const R = 6;
const C = 2 * Math.PI * R;

/**
 * Context-window use as a 16 px ring, in the vendor's colour. The level shows in the shape: the ring thickens from 80%
 * and gets a centre dot from 95%, so it reads without colour.
 *
 * Unknown hides it (docs/PLAN.md: unknown values are never guessed).
 */
export function ContextRing({ pct }: { pct: number | undefined }) {
  if (pct === undefined || !Number.isFinite(pct)) return null;
  const p = Math.min(100, Math.max(0, pct));
  const shown = Math.round(p);
  const tone = ringTone(p);
  return (
    <svg className="context-ring" data-tone={tone} role="img" aria-label={`Context ${shown}% used`} width={16} height={16} viewBox="0 0 16 16">
      <circle className="context-ring__track" cx="8" cy="8" r={R} fill="none" strokeWidth="2" />
      <circle
        className="context-ring__value"
        cx="8"
        cy="8"
        r={R}
        fill="none"
        strokeWidth={tone === "neutral" ? 2 : 3}
        strokeDasharray={`${(C * p) / 100} ${C}`}
        transform="rotate(-90 8 8)"
      />
      {tone === "error" && <circle className="context-ring__dot" cx="8" cy="8" r="1.5" />}
    </svg>
  );
}
