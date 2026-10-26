import { useCallback, useEffect, useRef, useState, type ReactNode, type Ref } from "react";
import { AgentMark } from "../../components/AgentMark/AgentMark";
import { Switch } from "../../components/Switch/Switch";
import { DiffView } from "../../components/DiffView/DiffView";
import type { AgentSetup } from "../../generated/AgentSetup";
import type { Endpoint } from "../../generated/Endpoint";
import type { RemovePreview } from "../../generated/RemovePreview";
import { applyPreview, getSetup, planRemove, previewDiff, type HookAgent } from "../../lib/api/agents";
import { useDialogFocus } from "../../lib/useDialogFocus";
import { SettingsReadError, type SettingsSync } from "../appearance/settingsSync";
import { ConsentCard } from "../onboarding/ConsentCard";
import { CLI_NAME, type Capability } from "../onboarding/consent";
import { actions, consentProps, hooksOffNotice, liveStatusOn, removalBlock, setupLine, setupWant, type InstallFlow, type SetupStart } from "./agentsState";
import { OwnAgentsSection } from "./OwnAgentsSection";
import { ToolsSection } from "./ToolsSection";
import { failureText, useInstallFlow } from "./useInstallFlow";
import { OwnToolsSection } from "./OwnToolsSection";
import "./AgentsSection.css";

/** The name a section and the change titles use: "Antigravity" (as "Set up Antigravity hooks"), where the consent card's own is "Antigravity CLI". */
const agentTitle = (agent: HookAgent): string => (agent === "antigravity" ? "Antigravity" : CLI_NAME[agent]);

export const SETUP_UNREADABLE = "Couldn't read the setup from Marshell's core. Close Settings and open it again.";

/** What the dialog shows below its title: the state, the card for Set up, or the diff for turning the hooks off. */
export type Panel =
  | { kind: "list" }
  | { kind: "setup"; flow: InstallFlow; agent?: HookAgent }
  | { kind: "off"; removal: RemovePreview | null; busy: boolean; error?: string; conflict?: boolean; agent?: HookAgent };

export type AgentsHandlers = {
  onSetup: (agent: HookAgent) => void;
  onHooksOff: (agent: HookAgent) => void;
  onBack: () => void;
  /** Nothing was left to change: back to the list, with the state read again as after an apply. */
  onDone: () => void;
  onAllow: () => void;
  onToggleDiff: () => void;
  onReload: () => void;
  onTurnOff: () => void;
  onSwitch: (capability: Capability, checked: boolean) => void;
  /** The Live status switch of one agent. */
  onLiveStatus: (checked: boolean, agent: HookAgent) => void;
  /** Opens the Notifications section, where the sounds and banners are set. */
  onNotifications: () => void;
};

/** Esc on the consent card (and its diff) or on the Hooks off preview leaves that panel and keeps the page from also closing. */
export function escapeLeavesPanel(e: { key: string; preventDefault: () => void }, panel: Panel, leave: () => void): void {
  if (e.key !== "Escape" || panel.kind === "list") return;
  e.preventDefault();
  leave();
}

/**
 * Settings → Agents (docs/PLAN.md slice 3c, decision 5), one section of the Settings page: each CLI's install state, with Set up,
 * Hooks off (preview, then apply) and the orphaned-hooks warning, then the own agents and the tools. Presentational: `AgentsSection` fetches and acts, this draws.
 */
export function AgentsSectionView({
  setup,
  codex,
  panel,
  liveStatus,
  liveStatusError,
  codexLiveStatus = null,
  codexLiveStatusError,
  antigravity,
  antigravityLiveStatus = null,
  antigravityLiveStatusError,
  own,
  tools,
  rootRef,
  ...on
}: {
  /** The tools, after the own agents. */
  tools?: ReactNode;
  rootRef?: Ref<HTMLDivElement>;
  /** Antigravity's setup; its section shows only when it was read and agy is found. */
  antigravity?: AgentSetup | null;
  antigravityLiveStatus?: boolean | null;
  antigravityLiveStatusError?: string | null;
  /** The section for the person's own agents, after the hook sections. */
  own?: ReactNode;
  setup: AgentSetup | "loading" | "failed";
  /** Codex's setup; its section shows only when it was read and Codex is found. */
  codex?: AgentSetup | null;
  panel: Panel;
  liveStatus: boolean | null;
  liveStatusError?: string | null;
  codexLiveStatus?: boolean | null;
  codexLiveStatusError?: string | null;
} & AgentsHandlers) {
  // The card and the turn-off panel belong to the agent the person chose.
  const chosen = panel.kind === "list" ? undefined : panel.agent === "codex" ? codex : panel.agent === "antigravity" ? antigravity : undefined;
  const target = chosen ?? (typeof setup === "object" ? setup : null);
  return (
    <div ref={rootRef} className="agents" onKeyDown={(e) => escapeLeavesPanel(e, panel, on.onBack)}>
      {setup === "loading" && <p className="agents-dialog__line">Reading the setup…</p>}
      {setup === "failed" && (
        <p className="agents-dialog__error" role="alert">
          {SETUP_UNREADABLE}
        </p>
      )}
      {typeof setup === "object" && panel.kind === "list" && (
        <>
          <AgentList setup={setup} agent="claude" liveStatus={liveStatus} error={liveStatusError} on={on} />
          {codex?.found && <AgentList setup={codex} agent="codex" liveStatus={codexLiveStatus} error={codexLiveStatusError} on={on} />}
          {antigravity?.found && <AgentList setup={antigravity} agent="antigravity" liveStatus={antigravityLiveStatus} error={antigravityLiveStatusError} on={on} />}
          {own}
          {tools}
        </>
      )}
      {target && panel.kind === "setup" && <SetupPanel setup={target} flow={panel.flow} on={on} />}
      {target && panel.kind === "off" && <OffPanel agent={target.agent as HookAgent} panel={panel} on={on} />}
    </div>
  );
}

/** What Codex says about Marshell's hook entries (docs/PLAN.md slice 6a decision 10). Absent trust says nothing. */
function TrustLine({ trust }: { trust: NonNullable<AgentSetup["trust"]> }) {
  switch (trust) {
    case "trusted":
      return <p className="agents-dialog__line">Trusted: Codex runs these hooks.</p>;
    case "untrusted":
      return (
        <p className="agents-dialog__warning">
          Not trusted yet: Codex won&apos;t run these hooks until you trust them. The next time Codex starts, choose <strong>Trust all and continue</strong>, or review them first in <code>/hooks</code>.
        </p>
      );
    case "changed":
      return (
        <p className="agents-dialog__warning">
          Changed since you trusted them: Codex won&apos;t run the changed hooks until you trust them again in <code>/hooks</code>.
        </p>
      );
  }
}

/** One agent's section: its state, its Live status switch, and Set up or Hooks off. Claude Code's has the Notifications link; Codex has no status line. */
function AgentList({ setup, agent, liveStatus, error, on }: { setup: AgentSetup; agent: HookAgent; liveStatus: boolean | null; error?: string | null; on: AgentsHandlers }) {
  const todo = actions(setup);
  const orphaned = setup.found && setup.hooks === "orphaned" && setup.settings_state !== "unparsable";
  const name = agentTitle(agent);
  return (
    <section className="agents-dialog__agent" aria-label={name} data-agent-card={agent} tabIndex={-1}>
      <header className="agents-dialog__agent-head">
        <AgentMark agent={agent} size={20} />
        <h3 className="agents-dialog__name">{name}</h3>
        {setup.found && setup.version && <span className="agents-dialog__version">version {setup.version}</span>}
      </header>
      <p className={orphaned ? "agents-dialog__warning" : "agents-dialog__line"} dir="auto">
        {setupLine(setup)}
      </p>
      {agent === "codex" && setup.trust && <TrustLine trust={setup.trust} />}
      {/* Antigravity's hooks and its status line live in two files; both are named. */}
      {setup.hooks_path !== undefined && (
        <p className="agents-dialog__line" dir="auto">
          Hooks file: <code>{setup.hooks_path}</code>. Status line file: <code>{setup.settings_path}</code>.
        </p>
      )}
      {setup.hooks === "installed" && !setup.approve_installed && <p className="agents-dialog__line">Approve from anywhere: off</p>}
      {/* Left out until the settings have been read: an unread value is never shown as off. */}
      {liveStatus !== null && (
        <Switch label="Live status" checked={liveStatus} onChange={(checked) => on.onLiveStatus(checked, agent)} reason={liveStatus ? undefined : "Rows use output activity only: Working or Idle."} />
      )}
      {agent === "claude" && (
        <p className="agents-dialog__line">
          Sounds and banners:{" "}
          <button type="button" className="agents-dialog__btn" onClick={on.onNotifications}>
            Notifications…
          </button>
        </p>
      )}
      {error && (
        <p className="agents-dialog__error" role="alert">
          {error}
        </p>
      )}
      {todo.length > 0 && (
        <div className="agents-dialog__actions">
          {todo.includes("setup") && (
            <button type="button" className="agents-dialog__btn" onClick={() => on.onSetup(agent)}>
              {setup.hooks === "none" || setup.hooks === "installed" ? "Set up" : "Set up again"}
            </button>
          )}
          {todo.includes("hooksOff") && (
            <button type="button" className="agents-dialog__btn" onClick={() => on.onHooksOff(agent)}>
              Hooks off
            </button>
          )}
        </div>
      )}
    </section>
  );
}

function SetupPanel({ setup, flow, on }: { setup: AgentSetup; flow: InstallFlow; on: AgentsHandlers }) {
  const { allow, done, ...card } = consentProps(setup, flow);
  return (
    <ConsentCard
      {...card}
      onApply={allow ? on.onAllow : undefined}
      onSkip={done ? on.onDone : on.onBack}
      skipLabel={done ? "Done" : setup.hooks === "installed" ? "Back" : undefined}
      onToggleDiff={on.onToggleDiff}
      onSwitch={on.onSwitch}
      conflict={flow.conflict ? { onReload: on.onReload } : undefined}
    />
  );
}

function OffPanel({ agent, panel, on }: { agent: HookAgent; panel: Extract<Panel, { kind: "off" }>; on: AgentsHandlers }) {
  const { removal } = panel;
  const block = removal ? removalBlock(removal.preview) : undefined;
  return (
    <section className="agents-dialog__off" aria-label={`Turn ${agentTitle(agent)} hooks off`}>
      <h3 className="agents-dialog__name">Turn {agentTitle(agent)} hooks off</h3>
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
 * The Agents section on the core's data. The state is read when it mounts and again after each apply; nothing polls.
 * `startWith: { setup: agent }` opens straight on that agent's Set up card (the row link). It only talks to the core's HTTP API.
 */
export function AgentsSection({
  endpoint,
  announce,
  onChanged,
  onNotifications,
  startWith,
  sync,
}: {
  endpoint: Endpoint;
  /** Says a result through the window's announcer. */
  announce?: (text: string) => void;
  /** The hooks were set up or turned off: the window reads the state again. */
  onChanged?: () => void;
  startWith?: SetupStart;
  /** Moves to the Notifications section. */
  onNotifications: () => void;
  /** The settings the Live status switch reads and saves. */
  sync?: SettingsSync;
}) {
  const [setup, setSetup] = useState<AgentSetup | "loading" | "failed">("loading");
  const [codex, setCodex] = useState<AgentSetup | null>(null);
  const [antigravity, setAntigravity] = useState<AgentSetup | null>(null);
  const [mode, setMode] = useState<"list" | "setup" | "off">("list");
  // The agent whose card or turn-off panel is open.
  const [active, setActive] = useState<HookAgent>("claude");
  const [off, setOff] = useState<Extract<Panel, { kind: "off" }>>({ kind: "off", removal: null, busy: false });

  // null until the settings have been read.
  const [liveStatus, setLiveStatus] = useState<Record<HookAgent, boolean | null>>({ claude: null, codex: null, antigravity: null });
  const [saveError, setSaveError] = useState<{ agent: HookAgent; text: string } | null>(null);
  useEffect(() => {
    let live = true;
    sync?.load().then(
      () => {
        if (!live) return;
        setLiveStatus({ claude: liveStatusOn(sync.statusOff, "claude"), codex: liveStatusOn(sync.statusOff, "codex"), antigravity: liveStatusOn(sync.statusOff, "antigravity") });
      },
      () => {},
    );
    return () => {
      live = false;
    };
  }, [sync]);
  const switchLiveStatus = async (on: boolean, agent: HookAgent) => {
    if (!sync) return;
    setSaveError(null);
    try {
      await sync.setStatusOff(agent, !on);
      setLiveStatus((s) => ({ ...s, [agent]: liveStatusOn(sync.statusOff, agent) }));
    } catch (e) {
      setSaveError({ agent, text: e instanceof SettingsReadError ? e.message : failureText(e) });
    }
  };

  const load = useCallback(async () => {
    // Codex's section is a bonus: a setup that cannot be read leaves it out and never fails Claude's.
    void getSetup(endpoint, "codex").then(setCodex, () => {});
    void getSetup(endpoint, "antigravity").then(setAntigravity, () => {});
    try {
      const found = await getSetup(endpoint, "claude");
      setSetup(found);
      return found;
    } catch {
      setSetup((s) => (typeof s === "object" ? s : "failed"));
    }
  }, [endpoint]);

  const installed = useCallback(() => {
    setMode("list");
    void load();
    onChanged?.();
  }, [load, onChanged]);
  // One flow per agent, so choosing an agent and planning never wait on a render.
  const claudeInstall = useInstallFlow(endpoint, "claude", installed);
  const codexInstall = useInstallFlow(endpoint, "codex", installed);
  const antigravityInstall = useInstallFlow(endpoint, "antigravity", installed);
  const installs = { claude: claudeInstall, codex: codexInstall, antigravity: antigravityInstall };
  const install = installs[active];

  const startSetup = useCallback(
    (agent: HookAgent = "claude", current?: AgentSetup) => {
      const found = current ?? (agent === "codex" ? codex : agent === "antigravity" ? antigravity : setup);
      if (typeof found !== "object" || found === null) return;
      setActive(agent);
      setMode("setup");
      void (agent === "codex" ? codexInstall : agent === "antigravity" ? antigravityInstall : claudeInstall).plan(setupWant(found));
    },
    [claudeInstall.plan, codexInstall.plan, antigravityInstall.plan, setup, codex, antigravity],
  );
  const startOff = useCallback(
    async (agent: HookAgent = "claude") => {
      setActive(agent);
      setMode("off");
      setOff({ kind: "off", removal: null, busy: true, agent });
      try {
        setOff({ kind: "off", removal: await planRemove(endpoint, agent), busy: false, agent });
      } catch (e) {
        setOff({ kind: "off", removal: null, busy: false, error: failureText(e), agent });
      }
    },
    [endpoint],
  );

  const first = useRef(true);
  useEffect(() => {
    if (!first.current) return;
    first.current = false;
    // The row's link opens straight on that CLI's Set up card (PLAN §9, slice 3 decision 5).
    void load().then(async (found) => {
      if (!startWith) return;
      if (startWith.setup === "claude") {
        if (found) startSetup("claude", found);
        return;
      }
      // Codex's or Antigravity's own setup is read here, so its card does not wait on the section's load.
      const agent = startWith.setup;
      const own = await getSetup(endpoint, agent).catch(() => null);
      if (own) startSetup(agent, own);
    });
  }, [load, startWith, startSetup, endpoint]);
  const root = useRef<HTMLDivElement>(null);

  const turnOff = async () => {
    if (!off.removal) return;
    const removal = off.removal;
    setOff({ ...off, busy: true, error: undefined });
    try {
      const out = await applyPreview(endpoint, removal.preview);
      if (out.ok) {
        announce?.(hooksOffNotice(removal.statusline_left));
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
    mode === "setup"
      ? { kind: "setup", agent: active, flow: install.flow ?? { preview: null, open: false, busy: true, want: install.want } }
      : mode === "off"
        ? off
        : { kind: "list" };
  return (
    <AgentsSectionView
      rootRef={root}
      setup={setup}
      codex={codex}
      own={<OwnAgentsSection endpoint={endpoint} onNotice={announce} />}
      tools={<ToolsSection endpoint={endpoint} ownEditor={<OwnToolsSection endpoint={endpoint} announce={announce} />} />}
      panel={panel}
      liveStatus={liveStatus.claude}
      liveStatusError={saveError?.agent === "claude" ? saveError.text : null}
      codexLiveStatus={liveStatus.codex}
      codexLiveStatusError={saveError?.agent === "codex" ? saveError.text : null}
      antigravity={antigravity}
      antigravityLiveStatus={liveStatus.antigravity}
      antigravityLiveStatusError={saveError?.agent === "antigravity" ? saveError.text : null}
      onLiveStatus={(on, agent) => void switchLiveStatus(on, agent)}
      onNotifications={onNotifications}
      onSetup={(agent) => startSetup(agent)}
      onHooksOff={(agent) => void startOff(agent)}
      onBack={back}
      onDone={done}
      onAllow={() => void install.apply()}
      onToggleDiff={install.toggleDiff}
      onSwitch={install.choose}
      onReload={() => void (mode === "off" ? startOff(active) : install.plan())}
      onTurnOff={() => void turnOff()}
    />
  );
}

/** Temporary until the Settings page is wired in (slice 6e1 Task 9). */
export function AgentsDialog({
  onClose,
  onNotice,
  ...section
}: {
  endpoint: Endpoint;
  onClose: () => void;
  onNotice?: (text: string) => void;
  onChanged?: () => void;
  startWith?: SetupStart;
  onNotifications: () => void;
  sync?: SettingsSync;
}) {
  const dialog = useRef<HTMLDivElement>(null);
  const title = useRef<HTMLHeadingElement>(null);
  useDialogFocus(dialog, title, onClose);
  return (
    <div className="agents-dialog">
      <div className="agents-dialog__backdrop" onClick={onClose} />
      <div ref={dialog} className="agents-dialog__sheet" role="dialog" aria-modal="true" aria-label="Agents">
        <h2 ref={title} tabIndex={-1} className="agents-dialog__title">
          Agents
        </h2>
        <div className="agents-dialog__body">
          <AgentsSection {...section} announce={onNotice} />
        </div>
        <footer className="agents-dialog__foot">
          <button type="button" className="agents-dialog__btn" onClick={onClose}>
            Close
          </button>
        </footer>
      </div>
    </div>
  );
}
