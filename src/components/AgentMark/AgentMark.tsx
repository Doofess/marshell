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
    outer: 0.5,
    layers: [
      { radius: 9, scale: 0.62 },
      { radius: 6.5, scale: 0.75 },
      { radius: 4.5, scale: 0.87 },
      { radius: 3, scale: 0.95 },
    ],
  },
};

/**
 * The vendor mark for a session's CLI, in the agent's brand colour. Unknown agents get the generic terminal mark.
 * `working` lets motion.css animate it with the vendor's own loop. A working Claude or Gemini is drawn in layers (BREATHING)
 * so its tips draw in more than its centre.
 */
export function AgentMark({ agent, size = 16, working = false }: { agent: AgentId; size?: 12 | 16 | 20; working?: boolean }) {
  const id: AgentId = isAgentId(agent) ? agent : "generic";
  const d = MARK_PATHS[id];
  const breath = BREATHING[id];
  const uid = useId().replace(/[^a-zA-Z0-9]/g, "");
  return (
    <svg className="agent-mark" data-agent={id} data-working={working || undefined} role="img" aria-label={AGENT_NAMES[id]} width={size} height={size} viewBox="0 0 24 24">
      <path className="agent-mark__body" style={breath ? ({ "--s": breath.outer } as CSSProperties) : undefined} d={d} fill="currentColor" fillRule="evenodd" />
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
