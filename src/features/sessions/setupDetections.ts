import type { AgentSetup } from "../../generated/AgentSetup";
import { toDetection, type HookAgent } from "../../lib/api/agents";
import type { Detection } from "../onboarding/types";

export type SetupDetections = { detections: Detection[]; detectionFailed: ReadonlySet<string> };

/** Keep a failed read distinct from a known setting that is off, including after an earlier successful read. */
export function recordSetupDetection(state: SetupDetections, agent: HookAgent, setup?: AgentSetup): SetupDetections {
  const detectionFailed = new Set(state.detectionFailed);
  const detections = state.detections.filter((d) => d.cli !== agent);
  if (setup) {
    detectionFailed.delete(agent);
    detections.push(toDetection(setup));
  } else {
    detectionFailed.add(agent);
  }
  return { detections, detectionFailed };
}
