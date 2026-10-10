import { AGENT_NAMES, MARK_PATHS, isAgentId, type AgentId } from "./agents";
import "./agent-colors.css";
import "./AgentMark.css";

/**
 * The vendor mark for a session's CLI, in the agent's brand colour. Unknown agents get the generic terminal mark.
 * `working` lets motion.css animate it with the vendor's own loop.
 */
export function AgentMark({ agent, size = 16, working = false }: { agent: AgentId; size?: 12 | 16 | 20; working?: boolean }) {
  const id: AgentId = isAgentId(agent) ? agent : "generic";
  const d = MARK_PATHS[id];
  return (
    <svg className="agent-mark" data-agent={id} data-working={working || undefined} role="img" aria-label={AGENT_NAMES[id]} width={size} height={size} viewBox="0 0 24 24">
      <path d={d} fill="currentColor" fillRule="evenodd" />
    </svg>
  );
}
