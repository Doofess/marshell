import { useCallback, useEffect, useRef, useState } from "react";
import { AgentMark } from "../../components/AgentMark/AgentMark";
import { Switch } from "../../components/Switch/Switch";
import { DiffView } from "../../components/DiffView/DiffView";
import type { AgentSetup } from "../../generated/AgentSetup";
import type { Endpoint } from "../../generated/Endpoint";
import type { RemovePreview } from "../../generated/RemovePreview";
import { applyPreview, getSetup, planRemove, previewDiff } from "../../lib/api/agents";
import { useDialogFocus } from "../../lib/useDialogFocus";
import { SettingsReadError, type SettingsSync } from "../appearance/settingsSync";
import { ConsentCard } from "../onboarding/ConsentCard";
import { type Capability } from "../onboarding/consent";
import { actions, consentProps, hooksOffNotice, liveStatusOn, removalBlock, setupLine, type InstallFlow } from "./agentsState";
import { failureText, useInstallFlow } from "./useInstallFlow";
import "./AgentsDialog.css";

export const SETUP_UNREADABLE = "Couldn't read the setup from Marshell's core. Close this and try again.";

/** What the dialog shows below its title: the state, the card for Set up, or the diff for turning the hooks off. */
export type Panel =
  | { kind: "list" }
  | { kind: "setup"; flow: InstallFlow }
  | { kind: "off"; removal: RemovePreview | null; busy: boolean; error?: string; conflict?: boolean };

export type AgentsDialogHandlers = {
  onClose: () => void;
  onSetup: () => void;
  onHooksOff: () => void;
  onBack: () => void;
  /** Nothing was left to change: back to the list, with the state read again as after an apply. */
  onDone: () => void;
  onAllow: () => void;
  onToggleDiff: () => void;
  onReload: () => void;
  onTurnOff: () => void;
  onSwitch: (capability: Capability, checked: boolean) => void;
  /** The Live status switch for Claude Code. */
  onLiveStatus: (checked: boolean) => void;
};

/**
 * Settings → Agents (docs/PLAN.md slice 3c, decision 5), as a modal dialog over the window: Claude Code's install state, with Set up,
 * Hooks off (preview, then apply) and the orphaned-hooks warning. Presentational: `AgentsDialog` fetches and acts, this draws.
 */
export function AgentsDialogView({ setup, panel, liveStatus, liveStatusError, ...on }: { setup: AgentSetup | "loading" | "failed"; panel: Panel; liveStatus: boolean | null; liveStatusError?: string | null } & AgentsDialogHandlers) {
  const dialog = useRef<HTMLDivElement>(null);
  const title = useRef<HTMLHeadingElement>(null);
  useDialogFocus(dialog, title, on.onClose);
  return (
    <div className="agents-dialog">
      <div className="agents-dialog__backdrop" onClick={on.onClose} />
      <div ref={dialog} className="agents-dialog__sheet" role="dialog" aria-modal="true" aria-label="Agents">
        <h2 ref={title} tabIndex={-1} className="agents-dialog__title">
          Agents
        </h2>
        <div className="agents-dialog__body">
          {setup === "loading" && <p className="agents-dialog__line">Reading the setup…</p>}
          {setup === "failed" && (
            <p className="agents-dialog__error" role="alert">
              {SETUP_UNREADABLE}
            </p>
          )}
          {typeof setup === "object" && panel.kind === "list" && <AgentList setup={setup} liveStatus={liveStatus} error={liveStatusError} on={on} />}
          {typeof setup === "object" && panel.kind === "setup" && <SetupPanel setup={setup} flow={panel.flow} on={on} />}
          {typeof setup === "object" && panel.kind === "off" && <OffPanel panel={panel} on={on} />}
        </div>
        <footer className="agents-dialog__foot">
          <button type="button" className="agents-dialog__btn" onClick={on.onClose}>
            Close
          </button>
        </footer>
      </div>
    </div>
  );
}

function AgentList({ setup, liveStatus, error, on }: { setup: AgentSetup; liveStatus: boolean | null; error?: string | null; on: AgentsDialogHandlers }) {
  const todo = actions(setup);
  const orphaned = setup.found && setup.hooks === "orphaned" && setup.settings_state !== "unparsable";
  return (
    <section className="agents-dialog__agent" aria-label="Claude Code">
      <header className="agents-dialog__agent-head">
        <AgentMark agent="claude" size={20} />
        <h3 className="agents-dialog__name">Claude Code</h3>
        {setup.found && setup.version && <span className="agents-dialog__version">version {setup.version}</span>}
      </header>
      <p className={orphaned ? "agents-dialog__warning" : "agents-dialog__line"} dir="auto">
        {setupLine(setup)}
      </p>
      {/* Left out until the settings have been read: an unread value is never shown as off. */}
      {liveStatus !== null && (
        <Switch label="Live status" checked={liveStatus} onChange={on.onLiveStatus} reason={liveStatus ? undefined : "Rows use output activity only: Working or Idle."} />
      )}
      {error && (
        <p className="agents-dialog__error" role="alert">
          {error}
        </p>
      )}
      {todo.length > 0 && (
        <div className="agents-dialog__actions">
          {todo.includes("setup") && (
            <button type="button" className="agents-dialog__btn" onClick={on.onSetup}>
              {setup.hooks === "none" ? "Set up" : "Set up again"}
            </button>
          )}
          {todo.includes("hooksOff") && (
            <button type="button" className="agents-dialog__btn" onClick={on.onHooksOff}>
              Hooks off
            </button>
          )}
        </div>
      )}
    </section>
  );
}

function SetupPanel({ setup, flow, on }: { setup: AgentSetup; flow: InstallFlow; on: AgentsDialogHandlers }) {
  const { allow, done, ...card } = consentProps(setup, flow);
  return (
    <ConsentCard
      {...card}
      onApply={allow ? on.onAllow : undefined}
      onSkip={done ? on.onDone : on.onBack}
      skipLabel={done ? "Done" : undefined}
      onToggleDiff={on.onToggleDiff}
      onSwitch={on.onSwitch}
      conflict={flow.conflict ? { onReload: on.onReload } : undefined}
    />
  );
}

function OffPanel({ panel, on }: { panel: Extract<Panel, { kind: "off" }>; on: AgentsDialogHandlers }) {
  const { removal } = panel;
  const block = removal ? removalBlock(removal.preview) : undefined;
  return (
    <section className="agents-dialog__off" aria-label="Turn Claude Code hooks off">
      <h3 className="agents-dialog__name">Turn Claude Code hooks off</h3>
      {removal ? (
        <>
          <p className="agents-dialog__line" dir="auto">
            {block ?? removal.preview.reason}
          </p>
          {!block && <DiffView diff={previewDiff(removal.preview)} label="Exact changes to turn the hooks off" />}
        </>
      ) : (
        !panel.error && <p className="agents-dialog__line">Checking what would change…</p>
      )}
      {panel.conflict && (
        <p className="agents-dialog__conflict">
          <strong>Review the difference.</strong> The file changed after this preview was made, so nothing was written. Reload to see what it says now.
        </p>
      )}
      {panel.error && (
        <p className="agents-dialog__error" role="alert">
          {panel.error}
        </p>
      )}
      <div className="agents-dialog__actions">
        {panel.conflict ? (
          <button type="button" className="agents-dialog__btn" onClick={on.onReload}>
            Reload
          </button>
        ) : (
          removal && (
            <button type="button" className="agents-dialog__btn" disabled={panel.busy || !removal.preview.can_apply} onClick={on.onTurnOff}>
              Turn hooks off
            </button>
          )
        )}
        <button type="button" className="agents-dialog__btn" onClick={on.onBack}>
          Back
        </button>
      </div>
    </section>
  );
}

/**
 * The Agents dialog on the core's data. The state is read when it opens and again after each apply; nothing polls.
 * `startWith: "setup"` opens straight on the Set up card (the row link). It only talks to the core's HTTP API.
 */
export function AgentsDialog({
  endpoint,
  onClose,
  onNotice,
  onChanged,
  startWith,
  sync,
}: {
  endpoint: Endpoint;
  onClose: () => void;
  onNotice?: (text: string) => void;
  /** The hooks were set up or turned off: the window reads the state again. */
  onChanged?: () => void;
  startWith?: "setup";
  /** The settings the Live status switch reads and saves. Left out, there is no switch. */
  sync?: SettingsSync;
}) {
  const [setup, setSetup] = useState<AgentSetup | "loading" | "failed">("loading");
  const [mode, setMode] = useState<"list" | "setup" | "off">("list");
  const [off, setOff] = useState<Extract<Panel, { kind: "off" }>>({ kind: "off", removal: null, busy: false });

  // null until the settings have been read.
  const [liveStatus, setLiveStatus] = useState<boolean | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    sync?.load().then(
      () => live && setLiveStatus(liveStatusOn(sync.statusOff, "claude")),
      () => {},
    );
    return () => {
      live = false;
    };
  }, [sync]);
  const switchLiveStatus = async (on: boolean) => {
    if (!sync) return;
    setSaveError(null);
    try {
      await sync.setStatusOff("claude", !on);
      setLiveStatus(liveStatusOn(sync.statusOff, "claude"));
    } catch (e) {
      setSaveError(e instanceof SettingsReadError ? e.message : failureText(e));
    }
  };

  const load = useCallback(async () => {
    try {
      setSetup(await getSetup(endpoint, "claude"));
    } catch {
      setSetup((s) => (typeof s === "object" ? s : "failed"));
    }
  }, [endpoint]);

  const installed = useCallback(() => {
    setMode("list");
    void load();
    onChanged?.();
  }, [load, onChanged]);
  const install = useInstallFlow(endpoint, installed);

  const startSetup = useCallback(() => {
    setMode("setup");
    void install.plan();
  }, [install.plan]);
  const startOff = useCallback(async () => {
    setMode("off");
    setOff({ kind: "off", removal: null, busy: true });
    try {
      setOff({ kind: "off", removal: await planRemove(endpoint, "claude"), busy: false });
    } catch (e) {
      setOff({ kind: "off", removal: null, busy: false, error: failureText(e) });
    }
  }, [endpoint]);

  const first = useRef(true);
  useEffect(() => {
    if (!first.current) return;
    first.current = false;
    void load().then(() => {
      if (startWith === "setup") startSetup();
    });
  }, [load, startWith, startSetup]);

  const turnOff = async () => {
    if (!off.removal) return;
    const removal = off.removal;
    setOff({ ...off, busy: true, error: undefined });
    try {
      const out = await applyPreview(endpoint, removal.preview);
      if (out.ok) {
        onNotice?.(hooksOffNotice(removal.statusline_left));
        setMode("list");
        void load();
        onChanged?.();
      } else if (out.conflict) setOff({ ...off, busy: false, conflict: true });
      else setOff({ ...off, busy: false, error: out.message });
    } catch (e) {
      setOff({ ...off, busy: false, error: failureText(e) });
    }
  };

  const back = () => {
    install.reset();
    setMode("list");
  };
  const done = () => {
    install.reset();
    installed();
  };
  const panel: Panel =
    mode === "setup" ? { kind: "setup", flow: install.flow ?? { preview: null, open: false, busy: true, want: install.want } } : mode === "off" ? off : { kind: "list" };
  return (
    <AgentsDialogView
      setup={setup}
      panel={panel}
      liveStatus={liveStatus}
      liveStatusError={saveError}
      onLiveStatus={(on) => void switchLiveStatus(on)}
      onClose={onClose}
      onSetup={startSetup}
      onHooksOff={() => void startOff()}
      onBack={back}
      onDone={done}
      onAllow={() => void install.apply()}
      onToggleDiff={install.toggleDiff}
      onSwitch={install.choose}
      onReload={() => void (mode === "off" ? startOff() : install.plan())}
      onTurnOff={() => void turnOff()}
    />
  );
}
