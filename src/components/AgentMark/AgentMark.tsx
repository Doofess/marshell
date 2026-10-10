import { useId, type CSSProperties } from "react";
import { AGENT_NAMES, MARK_PATHS, isAgentId, type AgentId } from "./agents";
import "./agent-colors.css";
import "./AgentMark.css";

/**
 * Claude's burst and Gemini's star draw in more at the tips than at the centre. The mark is drawn once for each layer
 * below plus once whole, each copy clipped to a
 * disc and drawn in by its own amount: the whole mark by the most, the inner part by less, the centre by least. Radii
 * are in the 24-unit mark and the amounts are the scale each copy reaches at the deepest point of the breath. Each
 * copy's reach at that point (radius x scale) stays inside the copy beneath it, so no cut edge shows.
 */
export type Breath = { outer: number; layers: readonly { radius: number; scale: number }[] };
export const BREATHING: Partial<Record<AgentId, Breath>> = {
  claude: {
    outer: 0.5,
    layers: [
      { radius: 8, scale: 0.72 },
      { radius: 4.5, scale: 0.92 },
    ],
  },
  // Gemini's sides are smooth curves, so its steps between layers would show; more, finer layers keep them out of sight.
  gemini: {
    outer: 0.42,
    layers: [
      { radius: 9, scale: 0.47 },
      { radius: 6.5, scale: 0.52 },
      { radius: 4.5, scale: 0.57 },
      { radius: 3, scale: 0.63 },
    ],
  },
};

/**
 * The vendor mark for a session's CLI, in the agent's brand colour. Unknown agents get the generic terminal mark.
 * `working` lets motion.css animate it with the vendor's own loop. A working Claude or Gemini is drawn in layers (BREATHING)
 * so its tips draw in more than its centre; a working Antigravity is drawn as a bell and tentacles that move separately; a working Copilot stays still while
 * bolts of electricity crackle across it.
 */
/** Where Antigravity's arch splits into bell and tentacles, in the 24-unit mark. Its working loop is a jellyfish swimming up. */
const BELL_BOTTOM = 13.5;

/** Copilot's electricity: zigzag bolts across the face, in the 24-unit mark. Each travels as a short bright dash. */
const BOLTS = [
  "M3 8 L8 11 L6 13 L11 15 L9 18 L14 20.5",
  "M21 6 L16 9 L18 12 L13 14 L16 17 L12 21",
  "M12 3.5 L10 7 L14 9 L11 12 L15 14.5 L13 18",
  "M23 14 L18 15 L19 18 L15 18.5 L16 21",
];

export function AgentMark({ agent, size = 16, working = false }: { agent: AgentId; size?: 12 | 16 | 20; working?: boolean }) {
  const id: AgentId = isAgentId(agent) ? agent : "generic";
  const d = MARK_PATHS[id];
  const breath = BREATHING[id];
  const sparks = id === "copilot" && working;
  const jelly = id === "antigravity" && working;
  const uid = useId().replace(/[^a-zA-Z0-9]/g, "");
  return (
    <svg className="agent-mark" data-agent={id} data-working={working || undefined} role="img" aria-label={AGENT_NAMES[id]} width={size} height={size} viewBox="0 0 24 24">
      <path className="agent-mark__body" style={breath ? ({ "--s": breath.outer } as CSSProperties) : undefined} d={d} fill="currentColor" fillRule="evenodd" />
      {sparks && (
        <g className="agent-mark__sparks" aria-hidden="true">
          <clipPath id={`${uid}-shape`}>
            <path d={d} clipRule="evenodd" />
          </clipPath>
          <g clipPath={`url(#${uid}-shape)`} fill="none" strokeLinecap="round" strokeLinejoin="round">
            {BOLTS.map((points, i) => (
              <g key={i} style={{ "--i": i } as CSSProperties}>
                <path className="agent-mark__bolt" data-glow="" d={points} pathLength="1" strokeWidth="4" />
                <path className="agent-mark__bolt" d={points} pathLength="1" strokeWidth="1.6" />
              </g>
            ))}
          </g>
        </g>
      )}
      {jelly && (
        <g className="agent-mark__parts" aria-hidden="true">
          <clipPath id={`${uid}-bell`}>
            <rect x="-4" y="-4" width="32" height={BELL_BOTTOM + 4} />
          </clipPath>
          <clipPath id={`${uid}-tentacles`}>
            <rect x="-4" y={BELL_BOTTOM} width="32" height={28 - BELL_BOTTOM} />
          </clipPath>
          <g clipPath={`url(#${uid}-bell)`}>
            <path className="agent-mark__bell" d={d} fill="currentColor" fillRule="evenodd" />
          </g>
          <g clipPath={`url(#${uid}-tentacles)`}>
            <path className="agent-mark__tentacles" d={d} fill="currentColor" fillRule="evenodd" />
          </g>
        </g>
      )}
      {breath &&
        working &&
        breath.layers.map((layer, i) => (
          <g key={i} clipPath={`url(#${uid}-l${i})`} aria-hidden="true">
            <clipPath id={`${uid}-l${i}`}>
              <circle cx="12" cy="12" r={layer.radius} />
            </clipPath>
            <path className="agent-mark__layer" style={{ "--s": layer.scale } as CSSProperties} d={d} fill="currentColor" fillRule="evenodd" />
          </g>
        ))}
    </svg>
  );
}
