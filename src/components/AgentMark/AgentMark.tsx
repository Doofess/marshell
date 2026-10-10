import { useId } from "react";
import { AGENT_NAMES, MARK_PATHS, isAgentId, type AgentId } from "./agents";
import "./agent-colors.css";
import "./AgentMark.css";

/** Radius, in the 24-unit mark, of the centre of the Claude burst that stays still while its rays move. */
export const CLAUDE_CORE_RADIUS = 4;

/**
 * The vendor mark for a session's CLI, in the agent's brand colour. Unknown agents get the generic terminal mark.
 * `working` lets motion.css animate it with the vendor's own loop. A working Claude also carries a copy of its centre
 * that stays put while the rays draw in and out around it, so the burst breathes instead of the whole logo shrinking.
 */
export function AgentMark({ agent, size = 16, working = false }: { agent: AgentId; size?: 12 | 16 | 20; working?: boolean }) {
  const id: AgentId = isAgentId(agent) ? agent : "generic";
  const d = MARK_PATHS[id];
  const core = `${useId().replace(/[^a-zA-Z0-9]/g, "")}-core`;
  return (
    <svg className="agent-mark" data-agent={id} data-working={working || undefined} role="img" aria-label={AGENT_NAMES[id]} width={size} height={size} viewBox="0 0 24 24">
      <path className="agent-mark__body" d={d} fill="currentColor" fillRule="evenodd" />
      {id === "claude" && working && (
        <>
          <clipPath id={core}>
            <circle cx="12" cy="12" r={CLAUDE_CORE_RADIUS} />
          </clipPath>
          <path className="agent-mark__core" d={d} fill="currentColor" fillRule="evenodd" clipPath={`url(#${core})`} aria-hidden="true" />
        </>
      )}
    </svg>
  );
}
