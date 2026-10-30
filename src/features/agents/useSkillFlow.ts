import { useCallback, useRef, useState } from "react";
import type { AgentSetup } from "../../generated/AgentSetup";
import type { ConfigPreview } from "../../generated/ConfigPreview";
import type { Endpoint } from "../../generated/Endpoint";
import { applyPreview, planSkill, planSkillRemove, type HookAgent } from "../../lib/api/agents";
import { failureText } from "./useInstallFlow";

/** Where the marshell skill's install, update or removal is: the preview the core made, and what the row says about it. */
export type SkillFlow = { kind: "install" | "remove"; preview: ConfigPreview | null; open: boolean; busy: boolean; error?: string; conflict?: boolean };

/** Whether a new flow starts with its diff open: replacing an edited skill file shows what changes first. */
export const skillFlowStartsOpen = (kind: SkillFlow["kind"], skill: AgentSetup["skill"] | undefined): boolean => kind === "install" && skill === "edited";

/**
 * Install, update or remove one agent's marshell skill file: plan, apply through the same config apply as the hooks, and re-plan after a
 * conflict. Nothing is applied without a preview whose file hashes the core checks; a conflict offers a reload, never an overwrite.
 */
export function useSkillFlow(endpoint: Endpoint, agent: HookAgent, onApplied: (kind: SkillFlow["kind"]) => void, skill?: AgentSetup["skill"], onReload?: () => void) {
  const [flow, setFlow] = useState<SkillFlow | null>(null);
  // Only the latest plan counts: a slow answer to an older Reload must not replace a newer preview.
  const latest = useRef(0);

  const plan = useCallback(
    async (kind: SkillFlow["kind"]) => {
      const mine = ++latest.current;
      const open = skillFlowStartsOpen(kind, skill);
      setFlow({ kind, preview: null, open, busy: true });
      try {
        const preview = await (kind === "install" ? planSkill(endpoint, agent) : planSkillRemove(endpoint, agent));
        if (mine === latest.current) setFlow({ kind, preview, open, busy: false });
      } catch (e) {
        if (mine === latest.current) setFlow({ kind, preview: null, open, busy: false, error: failureText(e) });
      }
    },
    [endpoint, agent, skill],
  );

  const apply = useCallback(async () => {
    const current = flow;
    if (!current?.preview || current.busy || !current.preview.can_apply) return;
    const mine = ++latest.current;
    setFlow({ ...current, busy: true, error: undefined });
    try {
      const out = await applyPreview(endpoint, current.preview);
      if (mine !== latest.current) return;
      if (out.ok) {
        setFlow(null);
        onApplied(current.kind);
      } else if (out.conflict) setFlow({ ...current, busy: false, conflict: true });
      else setFlow({ ...current, busy: false, error: out.message });
    } catch (e) {
      if (mine === latest.current) setFlow({ ...current, busy: false, error: failureText(e) });
    }
  }, [endpoint, flow, onApplied]);

  const install = useCallback(() => plan("install"), [plan]);
  const remove = useCallback(() => plan("remove"), [plan]);
  // A conflict means the file changed: read the row's setup again as well, so its line and the announced verb follow the file.
  const reload = useCallback(() => {
    onReload?.();
    return flow ? plan(flow.kind) : Promise.resolve();
  }, [flow, plan, onReload]);
  const toggleDiff = useCallback(() => setFlow((f) => (f ? { ...f, open: !f.open } : f)), []);
  const cancel = useCallback(() => {
    latest.current++;
    setFlow(null);
  }, []);

  return { flow, install, remove, apply, toggleDiff, reload, cancel };
}
