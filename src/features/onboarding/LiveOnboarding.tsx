import { useEffect, useRef, useState } from "react";
import type { AgentSetup } from "../../generated/AgentSetup";
import type { AgentFound } from "../../generated/AgentFound";
import type { Endpoint } from "../../generated/Endpoint";
import { getSetup, listAgents, toDetections } from "../../lib/api/agents";
import { consentProps, consentQueue, setupWant, type InstallFlow } from "../agents/agentsState";
import { useInstallFlow } from "../agents/useInstallFlow";
import { ConsentCard } from "./ConsentCard";
import type { Capability } from "./consent";
import { Detected } from "./Detected";
import { OnboardingFrame } from "./OnboardingFrame";

export const SETUP_UNREADABLE = "Couldn't read the setup from Marshell's core. You can set up Claude Code later from the Agents dialog.";

/** Where the live onboarding is: reading the setup, the Detected screen, the consent card on the core's preview, or a setup it could not read. */
export type OnboardingState =
  | { kind: "loading" }
  | { kind: "failed" }
  | { kind: "detected"; setup: AgentSetup; found: AgentFound[] | null; codex?: AgentSetup | null }
  /** The card of one agent (`setup` is that agent's): each found agent with hooks gets its own, one after the other. */
  | { kind: "consent"; setup: AgentSetup; flow: InstallFlow };

export type OnboardingHandlers = {
  onContinue: () => void;
  onBack: () => void;
  onAllow: () => void;
  onSkip: () => void;
  onReload: () => void;
  onToggleDiff: () => void;
  onSwitch: (capability: Capability, checked: boolean) => void;
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
        <Detected detections={toDetections(state.setup, state.found, state.codex)} />
      </OnboardingFrame>
    );
  const { flow, setup } = state;
  // The frame's button is the Allow; the card keeps its Skip, its diff toggle and, after a conflict, its Reload.
  const { allow, done, ...card } = consentProps(setup, flow);
  const ready = allow && !flow.busy;
  return (
    <OnboardingFrame step="consent" title="What Marshell may change" primary="Allow these changes" primaryDisabled={!ready} onPrimary={on.onAllow} onBack={on.onBack}>
      <ConsentCard {...card} onSkip={on.onSkip} skipLabel={done ? "Done" : undefined} onToggleDiff={on.onToggleDiff} onSwitch={on.onSwitch} conflict={flow.conflict ? { onReload: on.onReload } : undefined} />
    </OnboardingFrame>
  );
}

/**
 * First run on the core's data: what was found, then one consent card per found agent with hooks (Claude Code's, then Codex's), each
 * on the core's own preview. The state is read once when it opens. `onDone` is called when the last card is allowed or skipped, or the
 * person leaves; the window records that the onboarding was seen. Notifications and the first session are not part of this yet.
 */
export function LiveOnboarding({ endpoint, onDone }: { endpoint: Endpoint; onDone: () => void }) {
  const [setup, setSetup] = useState<AgentSetup | "loading" | "failed">("loading");
  const [found, setFound] = useState<AgentFound[] | null>(null);
  const [codex, setCodex] = useState<AgentSetup | null>(null);
  // Which card of the queue is open; null is the Detected screen.
  const [at, setAt] = useState<number | null>(null);
  // One flow per agent: an agent's card plans on its own flow, and the next card never waits on a render.
  const flows = { claude: useInstallFlow(endpoint, "claude", () => next.current()), codex: useInstallFlow(endpoint, "codex", () => next.current()) };

  const queue = typeof setup === "object" ? consentQueue([setup, ...(codex ? [codex] : [])]) : [];
  const current = at === null ? undefined : queue[at];
  // Allowing or skipping a card opens the next one, and after the last the onboarding ends.
  const go = (i: number) => {
    const target = queue[i];
    if (!target) return onDone();
    setAt(i);
    void flows[target.agent as "claude" | "codex"].plan(setupWant(target));
  };
  const next = useRef(() => {});
  next.current = () => go((at ?? -1) + 1);

  useEffect(() => {
    let live = true;
    Promise.all([getSetup(endpoint, "claude"), listAgents(endpoint).catch(() => null), getSetup(endpoint, "codex").catch(() => null)]).then(
      ([s, list, c]) => {
        if (!live) return;
        setFound(list);
        setCodex(c);
        setSetup(s);
      },
      () => live && setSetup("failed"),
    );
    return () => {
      live = false;
    };
  }, [endpoint]);

  const state: OnboardingState =
    setup === "loading" || setup === "failed"
      ? { kind: setup }
      : current
        ? { kind: "consent", setup: current, flow: flows[current.agent as "claude" | "codex"].flow ?? { preview: null, open: false, busy: true, want: flows[current.agent as "claude" | "codex"].want } }
        : { kind: "detected", setup, found, codex };
  const flow = current ? flows[current.agent as "claude" | "codex"] : undefined;

  return (
    <OnboardingView
      state={state}
      // No agent with something to set up means no card to offer, so onboarding ends.
      onContinue={() => go(0)}
      onBack={() => {
        flows.claude.reset();
        flows.codex.reset();
        setAt(null);
      }}
      onAllow={() => void flow?.apply()}
      onSkip={() => {
        flow?.reset();
        go((at ?? -1) + 1);
      }}
      onReload={() => void flow?.plan()}
      onToggleDiff={() => flow?.toggleDiff()}
      onSwitch={(c, checked) => flow?.choose(c, checked)}
    />
  );
}
