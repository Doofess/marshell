import "./Logo.css";

/**
 * The app icon ("Night shift", assets/icon/app-night-shift.svg): two marshalling wands on a dark tile, the pale
 * hands below and the signal-orange tips above. Drawn inline so it can sit in a button and be themed from Logo.css.
 */
export function Logo({ size = 24, decorative = false }: { size?: number; decorative?: boolean }) {
  const a11y = decorative ? ({ "aria-hidden": true } as const) : ({ role: "img", "aria-label": "Marshell" } as const);
  return (
    <svg className="logo" width={size} height={size} viewBox="0 0 1024 1024" {...a11y}>
      <rect width="1024" height="1024" rx="230" style={{ fill: "var(--logo-tile)" }} />
      <g strokeWidth="124" strokeLinecap="round" fill="none">
        <g style={{ stroke: "var(--logo-hand)" }}>
          <line x1="420" y1="792" x2="366" y2="624" />
          <line x1="604" y1="792" x2="658" y2="624" />
        </g>
        <g style={{ stroke: "var(--logo-signal)" }}>
          <line x1="366" y1="624" x2="240" y2="232" />
          <line x1="658" y1="624" x2="784" y2="232" />
        </g>
      </g>
    </svg>
  );
}
