import { useCallback, useRef, useState } from "react";
import type { Endpoint } from "../../generated/Endpoint";
import { ApiError } from "../../lib/api/client";
import { applyPreview, planInstall } from "../../lib/api/agents";
import type { Capability } from "../onboarding/consent";
import { DEFAULT_WANT, installRequest, type InstallFlow, type Want } from "./agentsState";

export const UNREACHABLE = "Couldn't reach Marshell's core. Try again in a moment.";

/** The text to show for a request that failed: the core's own words for a refusal, a plain line for anything else. */
export const failureText = (e: unknown): string => (e instanceof ApiError ? e.message : UNREACHABLE);

/**
 * Set up Claude Code's hooks: plan, apply, and re-plan after a conflict. A refused file (a 422) is the card's error, never a throw.
 * Nothing is ever applied without a preview whose file hashes the core checks, and a conflict offers a reload, never an overwrite.
 */
export function useInstallFlow(endpoint: Endpoint, onApplied: () => void) {
  const [flow, setFlow] = useState<InstallFlow | null>(null);
  // Only the latest plan counts: a slow answer to an older Reload must not replace a newer preview.
  const latest = useRef(0);
  // What the person has switched on. The plan and the preview on screen always come from it, so Allow applies what the card shows.
  const [want, setWant] = useState<Want>(DEFAULT_WANT);
  const wantRef = useRef(want);

  const plan = useCallback(async () => {
    const mine = ++latest.current;
    const asked = wantRef.current;
    // Status off installs nothing: there is no plan to ask for, and nothing to apply.
    if (!asked.status) return setFlow({ preview: null, open: false, busy: false });
    setFlow({ preview: null, open: false, busy: true });
    try {
      const preview = await planInstall(endpoint, "claude", installRequest(asked));
      if (mine === latest.current) setFlow({ preview, open: false, busy: false });
    } catch (e) {
      if (mine === latest.current) setFlow({ preview: null, open: false, busy: false, error: failureText(e) });
    }
  }, [endpoint]);

  const apply = useCallback(async () => {
    const current = flow;
    if (!current?.preview || current.busy) return;
    const mine = ++latest.current;
    setFlow({ ...current, busy: true, error: undefined });
    try {
      const out = await applyPreview(endpoint, current.preview);
      if (mine !== latest.current) return;
      if (out.ok) {
        setFlow(null);
        onApplied();
      } else if (out.conflict) setFlow({ ...current, busy: false, conflict: true });
      else setFlow({ ...current, busy: false, error: out.message });
    } catch (e) {
      if (mine === latest.current) setFlow({ ...current, busy: false, error: failureText(e) });
    }
  }, [endpoint, flow, onApplied]);

  const toggleDiff = useCallback(() => setFlow((f) => (f ? { ...f, open: !f.open } : f)), []);
  const reset = useCallback(() => {
    latest.current++;
    wantRef.current = DEFAULT_WANT;
    setWant(DEFAULT_WANT);
    setFlow(null);
  }, []);
  /** A switch changed: keep it, and plan again so the preview matches. */
  const choose = useCallback(
    (c: Capability, checked: boolean) => {
      if (c !== "status" && c !== "statusline") return;
      wantRef.current = { ...wantRef.current, [c]: checked };
      setWant(wantRef.current);
      void plan();
    },
    [plan],
  );

  return { flow: flow && { ...flow, want }, want, plan, apply, toggleDiff, reset, choose };
}
