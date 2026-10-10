import { AGENT_NAMES, MARK_PATHS, isAgentId, type AgentId } from "./agents";
import "./agent-colors.css";
import "./AgentMark.css";

/** The vendor mark for a session's CLI, in the agent's brand colour. Unknown agents get the generic terminal mark. */
export function AgentMark({ agent, size = 16 }: { agent: AgentId; size?: 12 | 16 | 20 }) {
  const id: AgentId = isAgentId(agent) ? agent : "generic";
  return (
    <svg className="agent-mark" data-agent={id} role="img" aria-label={AGENT_NAMES[id]} width={size} height={size} viewBox="0 0 24 24">
      <path d={MARK_PATHS[id]} fill="currentColor" fillRule="evenodd" />
    </svg>
  );
}
