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
 * so its tips draw in more than its centre.
 */
/** Antigravity options being compared (temporary). */
const ANTIGRAVITY_TRIAL: Breath = {
  outer: 0.6,
  layers: [
    { radius: 12, scale: 0.7 },
    { radius: 8, scale: 0.8 },
    { radius: 5, scale: 0.9 },
  ],
};
/** Where the arch splits into dome and feet, in the 24-unit mark. */
const FEET_TOP = 13.5;

export function AgentMark({ agent, size = 16, working = false, option }: { agent: AgentId; size?: 12 | 16 | 20; working?: boolean; option?: string }) {
  const id: AgentId = isAgentId(agent) ? agent : "generic";
  const d = MARK_PATHS[id];
  const breath = BREATHING[id] ?? (id === "antigravity" && option === "breath" ? ANTIGRAVITY_TRIAL : undefined);
  const legs = id === "antigravity" && option === "legs" && working;
  const uid = useId().replace(/[^a-zA-Z0-9]/g, "");
  return (
    <svg className="agent-mark" data-agent={id} data-working={working || undefined} data-option={option} role="img" aria-label={AGENT_NAMES[id]} width={size} height={size} viewBox="0 0 24 24">
      <path className="agent-mark__body" style={breath ? ({ "--s": breath.outer } as CSSProperties) : undefined} d={d} fill="currentColor" fillRule="evenodd" />
      {legs && (
        <>
          <clipPath id={`${uid}-dome`}>
            <rect x="-2" y="-2" width="28" height={FEET_TOP + 2} />
          </clipPath>
          <clipPath id={`${uid}-feet`}>
            <rect x="-2" y={FEET_TOP} width="28" height={26 - FEET_TOP} />
          </clipPath>
          <g clipPath={`url(#${uid}-dome)`} aria-hidden="true">
            <path d={d} fill="currentColor" fillRule="evenodd" />
          </g>
          <g clipPath={`url(#${uid}-feet)`} aria-hidden="true">
            <path className="agent-mark__feet" d={d} fill="currentColor" fillRule="evenodd" />
          </g>
        </>
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
