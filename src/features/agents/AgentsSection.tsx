import { useCallback, useEffect, useRef, useState, type ReactNode, type Ref } from "react";
import { AgentMark } from "../../components/AgentMark/AgentMark";
import { Switch } from "../../components/Switch/Switch";
import { DiffView } from "../../components/DiffView/DiffView";
import type { AgentSetup } from "../../generated/AgentSetup";
import type { Endpoint } from "../../generated/Endpoint";
import type { RemovePreview } from "../../generated/RemovePreview";
import { applyPreview, getSetup, planRemove, previewDiff, type HookAgent } from "../../lib/api/agents";
import { profiles } from "../../lib/api/profiles";
import { useAnnounceOnce } from "../../lib/useAnnounceOnce";
import { SettingsReadError, type SettingsSync } from "../appearance/settingsSync";
import { ConsentCard } from "../onboarding/ConsentCard";
import { CLI_NAME, type Capability } from "../onboarding/consent";
import { actions, consentProps, DEFAULT_WANT, hooksOffNotice, liveStatusOn, removalBlock, setupLine, setupWant, wslDistros, type InstallFlow, type SetupStart } from "./agentsState";
import { OwnAgentsSection } from "./OwnAgentsSection";
import { ToolsSection } from "./ToolsSection";
import { SkillRow, type SkillRowHandlers } from "./SkillRow";
import { failureText, useInstallFlow } from "./useInstallFlow";
import { useSkillFlow, type SkillFlow } from "./useSkillFlow";
import { OwnToolsSection } from "./OwnToolsSection";
import "./AgentsSection.css";

/** The name a section and the change titles use: "Antigravity" (as "Set up Antigravity hooks"), where the consent card's own is "Antigravity CLI". */
const agentTitle = (agent: HookAgent): string => (agent === "antigravity" ? "Antigravity" : CLI_NAME[agent]);

export const SKILL_CONFLICT = "The skill file changed. Reload to see it.";
export const SETUP_UNREADABLE = "Couldn't read the setup from Marshell's core. Close Settings and open it again.";

/** What the dialog shows below its title: the state, the card for Set up, or the diff for turning the hooks off. */
export type Panel =
  | { kind: "list" }
  | { kind: "setup"; flow: InstallFlow; agent?: HookAgent }
  | { kind: "off"; removal: RemovePreview | null; busy: boolean; error?: string; conflict?: boolean; agent?: HookAgent; distro?: string };

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
  /** Set up this agent's hooks inside a WSL distro. */
  onWslSetup?: (agent: HookAgent, distro: string) => void;
  /** Turn this agent's hooks off inside a WSL distro. */
  onWslRemove?: (agent: HookAgent, distro: string) => void;
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
  skills,
  wslDistros: distros,
  ...on
}: {
  /** The WSL distros the session profiles name; the WSL group shows only once they are known. */
  wslDistros?: string[];
  /** Each agent's marshell skill flow and handlers; without it the rows are left out. */
  skills?: Record<HookAgent, { flow: SkillFlow | null; on: SkillRowHandlers }>;
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
        <p className="agents-dialog__error">
          {SETUP_UNREADABLE}
        </p>
      )}
      {typeof setup === "object" && panel.kind === "list" && (
        <>
          <AgentList setup={setup} agent="claude" liveStatus={liveStatus} error={liveStatusError} on={on} skill={skills?.claude} distros={distros} />
          {/* A CLI missing on Windows still gets its section when there are distros: it may be installed inside one. */}
          {codex && (codex.found || (distros?.length ?? 0) > 0) && <AgentList setup={codex} agent="codex" liveStatus={codexLiveStatus} error={codexLiveStatusError} on={on} skill={skills?.codex} distros={distros} />}
          {antigravity && (antigravity.found || (distros?.length ?? 0) > 0) && <AgentList setup={antigravity} agent="antigravity" liveStatus={antigravityLiveStatus} error={antigravityLiveStatusError} on={on} skill={skills?.antigravity} distros={distros} />}
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
function AgentList({ setup, agent, liveStatus, error, on, skill, distros }: { setup: AgentSetup; agent: HookAgent; liveStatus: boolean | null; error?: string | null; on: AgentsHandlers; skill?: { flow: SkillFlow | null; on: SkillRowHandlers }; distros?: string[] }) {
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
      {skill && setup.found && <SkillRow agent={agent} setup={setup} flow={skill.flow} on={skill.on} />}
      {/* Antigravity's hooks and its status line live in two files; both are named. */}
      {setup.found && setup.hooks_path !== undefined && (
        <p className="agents-dialog__line" dir="auto">
          Hooks file: <code>{setup.hooks_path}</code>. Status line file: <code>{setup.settings_path}</code>.
        </p>
      )}
      {setup.found && setup.hooks === "installed" && !setup.approve_installed && <p className="agents-dialog__line">Approve from anywhere: off</p>}
      {/* Left out until the settings have been read: an unread value is never shown as off. Marshell's own setting, so it stays for a CLI found only in a distro. */}
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
        <p className="agents-dialog__error">
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
      {distros && <WslGroup agent={agent} distros={distros} on={on} />}
    </section>
  );
}

/**
 * Hook setup inside each WSL distro (slice 6j1). The distros come from the session profiles, so nothing here starts one:
 * a distro is asked about only when one of its buttons is pressed. The skill file is not offered; the core installs none in a distro.
 */
function WslGroup({ agent, distros, on }: { agent: HookAgent; distros: string[]; on: AgentsHandlers }) {
  return (
    <div className="agents-dialog__wsl" role="group" aria-label="WSL" data-wsl-group={agent}>
      <h4 className="agents-dialog__wsl-head">WSL</h4>
      <p className="agents-dialog__line">Agents you run inside a WSL distro read their own settings there. Set up writes Marshell&apos;s hooks into that distro.</p>
      {distros.length === 0 ? (
        <p className="agents-dialog__line">No WSL distros found.</p>
      ) : (
        distros.map((distro) => (
          <div key={distro} className="agents-dialog__actions" data-wsl-distro={distro}>
            <button type="button" className="agents-dialog__btn" onClick={() => on.onWslSetup?.(agent, distro)}>
              {`Set up in ${distro}`}
            </button>
            <button type="button" className="agents-dialog__btn" onClick={() => on.onWslRemove?.(agent, distro)}>
              {`Remove from ${distro}`}
            </button>
          </div>
        ))
      )}
    </div>
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
    <section className="agents-dialog__off" aria-label={`Turn ${agentTitle(agent)} hooks off${panel.distro ? ` in ${panel.distro}` : ""}`}>
      <h3 className="agents-dialog__name">{`Turn ${agentTitle(agent)} hooks off${panel.distro ? ` in ${panel.distro}` : ""}`}</h3>
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
        <p className="agents-dialog__error">
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
  // The skill rows: after an apply the section reads the setup again, and the announcer says what was done.
  const setups = { claude: setup, codex, antigravity };
  const skillApplied = (agent: HookAgent) => (kind: SkillFlow["kind"]) => {
    const current = setups[agent];
    const updating = typeof current === "object" && current !== null && (current.skill === "outdated" || current.skill === "edited");
    const verb = kind === "remove" ? "Removed" : updating ? "Updated" : "Installed";
    announce?.(`${verb} the marshell skill for ${CLI_NAME[agent]}.`);
    void load();
    onChanged?.();
  };
  const skillOf = (s: AgentSetup | "loading" | "failed" | null | undefined) => (typeof s === "object" && s !== null ? s.skill : undefined);
  const reloadSetups = useCallback(() => void load(), [load]);
  const claudeSkill = useSkillFlow(endpoint, "claude", skillApplied("claude"), skillOf(setup), reloadSetups);
  const codexSkill = useSkillFlow(endpoint, "codex", skillApplied("codex"), skillOf(codex), reloadSetups);
  const antigravitySkill = useSkillFlow(endpoint, "antigravity", skillApplied("antigravity"), skillOf(antigravity), reloadSetups);
  const skillFlows = { claude: claudeSkill, codex: codexSkill, antigravity: antigravitySkill };
  const skills = Object.fromEntries(
    (Object.keys(skillFlows) as HookAgent[]).map((a) => {
      const f = skillFlows[a];
      return [a, { flow: f.flow, on: { install: () => void f.install(), remove: () => void f.remove(), apply: () => void f.apply(), toggleDiff: f.toggleDiff, reload: () => void f.reload(), cancel: f.cancel } }];
    }),
  ) as Record<HookAgent, { flow: SkillFlow | null; on: SkillRowHandlers }>;
  // A conflict is said once, when it appears.
  useAnnounceOnce(claudeSkill.flow?.conflict ? SKILL_CONFLICT : null, announce);
  useAnnounceOnce(codexSkill.flow?.conflict ? SKILL_CONFLICT : null, announce);
  useAnnounceOnce(antigravitySkill.flow?.conflict ? SKILL_CONFLICT : null, announce);
  const installs = { claude: claudeInstall, codex: codexInstall, antigravity: antigravityInstall };
  const install = installs[active];

  const startSetup = useCallback(
    (agent: HookAgent = "claude", current?: AgentSetup) => {
      const found = current ?? (agent === "codex" ? codex : agent === "antigravity" ? antigravity : setup);
      if (typeof found !== "object" || found === null) return;
      setActive(agent);
      setMode("setup");
      void (agent === "codex" ? codexInstall : agent === "antigravity" ? antigravityInstall : claudeInstall).plan(setupWant(found), found.skill);
    },
    [claudeInstall.plan, codexInstall.plan, antigravityInstall.plan, setup, codex, antigravity],
  );
  const startOff = useCallback(
    async (agent: HookAgent = "claude", distro?: string) => {
      setActive(agent);
      setMode("off");
      setOff({ kind: "off", removal: null, busy: true, agent, distro });
      try {
        setOff({ kind: "off", removal: await planRemove(endpoint, agent, distro), busy: false, agent, distro });
      } catch (e) {
        setOff({ kind: "off", removal: null, busy: false, error: failureText(e), agent, distro });
      }
    },
    [endpoint],
  );
  // The WSL distros come from the session profiles (one GET of the core's stored profiles; no distro is started and nothing polls).
  const [distros, setDistros] = useState<string[] | undefined>(undefined);
  useEffect(() => {
    let live = true;
    profiles.get(endpoint).then(
      (doc) => {
        if (live) setDistros(wslDistros(doc.profiles));
      },
      () => {},
    );
    return () => {
      live = false;
    };
  }, [endpoint]);
  const startWslSetup = useCallback(
    (agent: HookAgent, distro: string) => {
      // The hooks the Windows panel would offer by default; the core installs no skill file in a distro.
      setActive(agent);
      setMode("setup");
      void (agent === "codex" ? codexInstall : agent === "antigravity" ? antigravityInstall : claudeInstall).plan(DEFAULT_WANT, undefined, distro);
    },
    [claudeInstall.plan, codexInstall.plan, antigravityInstall.plan],
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
  // The view draws these errors as plain text; the page's announcer says each once, when it appears.
  useAnnounceOnce(setup === "failed" ? SETUP_UNREADABLE : null, announce);
  useAnnounceOnce(saveError?.text, announce);
  useAnnounceOnce(mode === "off" ? off.error : null, announce);
  const panel: Panel =
    mode === "setup"
      ? { kind: "setup", agent: active, flow: install.flow ?? { preview: null, open: false, busy: true, want: install.want } }
      : mode === "off"
        ? off
        : { kind: "list" };
  return (
    <AgentsSectionView
      rootRef={root}
      skills={skills}
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
      wslDistros={distros}
      onWslSetup={startWslSetup}
      onWslRemove={(agent, distro) => void startOff(agent, distro)}
      onSetup={(agent) => startSetup(agent)}
      onHooksOff={(agent) => void startOff(agent)}
      onBack={back}
      onDone={done}
      onAllow={() => void install.apply()}
      onToggleDiff={install.toggleDiff}
      onSwitch={install.choose}
      onReload={() => void (mode === "off" ? startOff(active, off.distro) : install.plan())}
      onTurnOff={() => void turnOff()}
    />
  );
}
