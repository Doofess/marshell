import { WORDMARK } from "./wordmarkPath";

const [, , W, H] = WORDMARK.viewBox.split(" ").map(Number) as [number, number, number, number];

/** The app's name next to its icon, as vector letters so it looks the same on every OS and in every theme. */
export function Wordmark({ height = 14, decorative = false }: { height?: number; decorative?: boolean }) {
  const a11y = decorative ? ({ "aria-hidden": true } as const) : ({ role: "img", "aria-label": "Marshell" } as const);
  return (
    <svg className="wordmark" width={Math.round(((height * W) / H) * 100) / 100} height={height} viewBox={WORDMARK.viewBox} {...a11y}>
      <path d={WORDMARK.d} fill="currentColor" />
    </svg>
  );
}
