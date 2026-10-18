import { useEffect, useState } from "react";
import type { AgentSetup } from "../../generated/AgentSetup";
import type { Endpoint } from "../../generated/Endpoint";
import { getSetup, toDetection } from "../../lib/api/agents";
import { actions, consentProps, type InstallFlow } from "../agents/agentsState";
import { useInstallFlow } from "../agents/useInstallFlow";
import { ConsentCard } from "./ConsentCard";
import { Detected } from "./Detected";
import { OnboardingFrame } from "./OnboardingFrame";

export const SETUP_UNREADABLE = "Couldn't read the setup from Marshell's core. You can set up Claude Code later from the Agents dialog.";

/** Where the live onboarding is: reading the setup, the Detected screen, the consent card on the core's preview, or a setup it could not read. */
export type OnboardingState =
  | { kind: "loading" }
  | { kind: "failed" }
  | { kind: "detected"; setup: AgentSetup }
  | { kind: "consent"; setup: AgentSetup; flow: InstallFlow };

export type OnboardingHandlers = {
  onContinue: () => void;
  onBack: () => void;
  onAllow: () => void;
  onSkip: () => void;
  onReload: () => void;
  onToggleDiff: () => void;
};

/** The first two onboarding screens on the core's data. Presentational: `LiveOnboarding` fetches and acts, this draws. */
export function OnboardingView({ state, ...on }: { state: OnboardingState } & OnboardingHandlers) {
  if (state.kind === "loading")
    return (
      <OnboardingFrame step="detected" title="Coding agents found" primary="Continue" primaryDisabled>
        <p>Looking for coding agents…</p>
      </OnboardingFrame>
    );
  if (state.kind === "failed")
    return (
      <OnboardingFrame step="detected" title="Coding agents found" primary="Skip for now" onPrimary={on.onSkip}>
        <p role="alert">{SETUP_UNREADABLE}</p>
      </OnboardingFrame>
    );
  if (state.kind === "detected")
    return (
      <OnboardingFrame step="detected" title="Coding agents found" primary="Continue" onPrimary={on.onContinue}>
        <Detected detections={[toDetection(state.setup)]} />
      </OnboardingFrame>
    );
  const { flow, setup } = state;
  // The frame's button is the Allow; the card keeps its Skip, its diff toggle and, after a conflict, its Reload.
  const ready = Boolean(flow.preview) && !flow.conflict && !flow.busy;
  return (
    <OnboardingFrame step="consent" title="What Marshell may change" primary="Allow these changes" primaryDisabled={!ready} onPrimary={on.onAllow} onBack={on.onBack}>
      <ConsentCard {...consentProps(setup, flow)} onSkip={on.onSkip} onToggleDiff={on.onToggleDiff} conflict={flow.conflict ? { onReload: on.onReload } : undefined} />
    </OnboardingFrame>
  );
}

/**
 * First run on the core's data: what was found, then the consent card on the core's own preview. The state is read once when it
 * opens. `onDone` is called when the person allows, skips or leaves; the window records that the onboarding was seen.
 * Notifications and the first session are not part of this yet.
 */
export function LiveOnboarding({ endpoint, onDone }: { endpoint: Endpoint; onDone: () => void }) {
  const [setup, setSetup] = useState<AgentSetup | "loading" | "failed">("loading");
  const [consent, setConsent] = useState(false);
  const install = useInstallFlow(endpoint, onDone);

  useEffect(() => {
    let live = true;
    getSetup(endpoint, "claude").then(
      (s) => live && setSetup(s),
      () => live && setSetup("failed"),
    );
    return () => {
      live = false;
    };
  }, [endpoint]);

  const state: OnboardingState =
    setup === "loading" || setup === "failed"
      ? { kind: setup }
      : consent
        ? { kind: "consent", setup, flow: install.flow ?? { preview: null, open: false, busy: true } }
        : { kind: "detected", setup };

  return (
    <OnboardingView
      state={state}
      onContinue={() => {
        // Nothing to set up (not found, already on, a file that does not parse): the card has nothing to offer, so the onboarding ends.
        if (typeof setup !== "object" || !actions(setup).includes("setup")) return onDone();
        setConsent(true);
        void install.plan();
      }}
      onBack={() => {
        install.reset();
        setConsent(false);
      }}
      onAllow={() => void install.apply()}
      onSkip={onDone}
      onReload={() => void install.plan()}
      onToggleDiff={install.toggleDiff}
    />
  );
}
