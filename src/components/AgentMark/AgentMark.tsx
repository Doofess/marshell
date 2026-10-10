import { useId, type CSSProperties } from "react";
import { AGENT_NAMES, MARK_PATHS, isAgentId, type AgentId } from "./agents";
import { CLAUDE_WEDGES } from "./claudeRays";
import "./agent-colors.css";
import "./AgentMark.css";

/**
 * The vendor mark for a session's CLI, in the agent's brand colour. Unknown agents get the generic terminal mark.
 * `working` lets motion.css animate it with the vendor's own loop. Claude's loop moves its twelve rays one after
 * another, so a working Claude also carries the rays as twelve wedge-clipped copies of the logo; they stay hidden
 * unless motion is allowed (motion.css), and the whole logo is what shows otherwise.
 */
export function AgentMark({ agent, size = 16, working = false }: { agent: AgentId; size?: 12 | 16 | 20; working?: boolean }) {
  const id: AgentId = isAgentId(agent) ? agent : "generic";
  const uid = useId().replace(/[^a-zA-Z0-9]/g, "");
  const d = MARK_PATHS[id];
  return (
    <svg className="agent-mark" data-agent={id} data-working={working || undefined} role="img" aria-label={AGENT_NAMES[id]} width={size} height={size} viewBox="0 0 24 24">
      <path className="agent-mark__still" d={d} fill="currentColor" fillRule="evenodd" />
      {id === "claude" && working && (
        <g className="agent-mark__rays" aria-hidden="true">
          <defs>
            {CLAUDE_WEDGES.map((points, i) => (
              <clipPath key={i} id={`${uid}-w${i}`}>
                <polygon points={points} />
              </clipPath>
            ))}
          </defs>
          {CLAUDE_WEDGES.map((_, i) => (
            <g key={i} clipPath={`url(#${uid}-w${i})`}>
              <g className="agent-mark__ray" style={{ "--i": i } as CSSProperties}>
                <path d={d} fill="currentColor" fillRule="evenodd" />
              </g>
            </g>
          ))}
        </g>
      )}
    </svg>
  );
}
