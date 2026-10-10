import { useId, type CSSProperties } from "react";
import { AGENT_NAMES, AMP_PIECES, CURSOR_CUBE, CURSOR_POINTER, GENERIC_PIECES, MARK_PATHS, isAgentId, type AgentId } from "./agents";
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
 * so its tips draw in more than its centre; a working Antigravity is drawn as a bell and tentacles that move separately; a working custom agent keeps its frame and prompt still while only the underscore blinks; a working Copilot stays still while
 * parts of it flash dark, each on its own tempo, like a current.
 */
/** Where Antigravity's arch splits into bell and tentacles, in the 24-unit mark. Its working loop is a jellyfish swimming up. */
const BELL_BOTTOM = 13.5;

/**
 * Copilot's electricity: parts of the face (left ear, left goggle, nose bridge, right goggle, right ear, lower face). Each
 * is a box in the 24-unit mark, clipped to the shape of the mark, and flashes on its own tempo: `tempo` stretches the
 * loop (so the parts drift in and out of step and the rhythm never repeats) and `phase` is how far into its loop it starts.
 */
const ZONES = [
  { x: -1, y: 8, width: 6.2, height: 15, tempo: 0.62, phase: 0.1 },
  { x: 5, y: 2, width: 6.3, height: 9, tempo: 1.1, phase: 0.55 },
  { x: 10, y: 2, width: 4, height: 10, tempo: 0.85, phase: 0.3 },
  { x: 12.7, y: 2, width: 6.3, height: 9, tempo: 1.45, phase: 0.8 },
  { x: 18.8, y: 8, width: 6.2, height: 15, tempo: 0.72, phase: 0.45 },
  { x: 5, y: 11, width: 14, height: 12, tempo: 1.25, phase: 0.02 },
] as const;

export function AgentMark({ agent, size = 16, working = false }: { agent: AgentId; size?: 12 | 16 | 20; working?: boolean }) {
  const id: AgentId = isAgentId(agent) ? agent : "generic";
  const d = MARK_PATHS[id];
  const breath = BREATHING[id];
  const pointer = id === "cursor" && working;
  const pieces = working ? (id === "amp" ? AMP_PIECES : id === "generic" ? GENERIC_PIECES : undefined) : undefined;
  const charge = id === "copilot" && working;
  const jelly = id === "antigravity" && working;
  const uid = useId().replace(/[^a-zA-Z0-9]/g, "");
  return (
    <svg className="agent-mark" data-agent={id} data-working={working || undefined} role="img" aria-label={AGENT_NAMES[id]} width={size} height={size} viewBox="0 0 24 24">
      <path className="agent-mark__body" style={breath ? ({ "--s": breath.outer } as CSSProperties) : undefined} d={d} fill="currentColor" fillRule="evenodd" />
      {pieces && (
        <g className="agent-mark__pieces" aria-hidden="true">
          {pieces.map((piece, i) => (
            <path key={i} className="agent-mark__piece" style={{ "--i": i } as CSSProperties} d={piece} fill="currentColor" fillRule="evenodd" />
          ))}
        </g>
      )}
      {pointer && (
        <g className="agent-mark__cursor" aria-hidden="true">
          <mask id={`${uid}-cut`} maskUnits="userSpaceOnUse" x="-6" y="-6" width="36" height="36">
            <rect x="-6" y="-6" width="36" height="36" fill="white" />
            <path className="agent-mark__pointer" d={CURSOR_POINTER} fill="black" />
          </mask>
          <path d={CURSOR_CUBE} mask={`url(#${uid}-cut)`} fill="currentColor" />
        </g>
      )}
      {charge && (
        <g className="agent-mark__charge" aria-hidden="true">
          <clipPath id={`${uid}-shape`}>
            <path d={d} clipRule="evenodd" />
          </clipPath>
          <g clipPath={`url(#${uid}-shape)`}>
            {ZONES.map(({ tempo, phase, ...box }, i) => (
              <rect key={i} className="agent-mark__zone" style={{ "--k": tempo, "--d": phase } as CSSProperties} {...box} />
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
