import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import type { Endpoint } from "../../generated/Endpoint";
import { FOCUSABLE } from "../../lib/dialogFocus";
import { usePopupFocus } from "../../lib/usePopupFocus";
import { AgentsSection } from "../agents/AgentsSection";
import type { SetupStart } from "../agents/agentsState";
import { AccountsSection } from "../accounts/AccountsSection";
import { AppearanceSection, TerminalSection } from "../appearance/AppearanceSections";
import type { SettingsSync } from "../appearance/settingsSync";
import { CompanySection } from "../companies/CompanySection";
import { openerUsable } from "../departures/liveBoard";
import { AdvancedSection } from "../diagnostics/AdvancedSection";
import { HistorySection } from "../history/HistorySection";
import { NotificationsSection } from "../notifications/NotificationsSection";
import { closeOrRefuse, focusSettingsHeading, mountedWith, settingsFocusFallback, settingsTabTarget, visibleControls } from "./settingsRouting";
import { ProfilesSection } from "./ProfilesSection";
import { ProjectsSection } from "./ProjectsSection";
import { SettingsShell, settingsKey } from "./SettingsShell";
import { COMPANY_NAMES, shownSections, type CompanyAgent, type SettingsSection } from "./settingsSections";
import "./LiveSettings.css";

/** Said when a close is refused over unsaved shell profile edits: the words the profiles dialog showed beside its disabled Close. */
const SAVE_FIRST = "Save or discard the changes to close.";

/**
 * The sections that have been shown, each in a panel of its own. A section mounts the first time it is shown and stays mounted,
 * hidden, so switching keeps an unsaved form; the `hidden` attribute is the only thing that hides it.
 */
export function SettingsPanels({ section, mounted, render }: { section: SettingsSection; mounted: SettingsSection[]; render: (id: SettingsSection) => ReactNode }) {
  return (
    <>
      {mounted.map((id) => (
        <div key={id} hidden={id !== section}>
          {render(id)}
        </div>
      ))}
    </>
  );
}

const NONE: ReadonlySet<CompanyAgent> = new Set();
const ZERO: Record<CompanyAgent, number> = { claude: 0, codex: 0, antigravity: 0 };
const noop = () => {};

type Props = {
  /** The core's endpoint. Shell profiles, Agents and Advanced talk to the core and show nothing without it. */
  endpoint?: Endpoint;
  sync?: SettingsSync;
  section: SettingsSection;
  onSection: (id: SettingsSection) => void;
  /** The row's "set up" link: Agents opens on that CLI's card. */
  agentsStart?: SetupStart;
  /** The folder the Projects section opens on: the focused session's, when the palette routed there. */
  initialCwd?: string;
  announce: (text: string) => void;
  onProfilesSaved?: () => void;
  onAgentsChanged?: () => void;
  /** "Notifications" inside Agents: the window moves to that section. */
  onNotifications: () => void;
  /** The company CLIs found on this machine: Accounts lists them and each has a section. */
  found?: ReadonlySet<CompanyAgent>;
  /** Bumped when a sign-in tab exits, so Accounts reads the status again. */
  accountsRefresh?: number;
  /** Bumped per CLI when its update tab exits, so its section reads the version again. */
  companyRefresh?: Record<CompanyAgent, number>;
  /** How many tabs are running each CLI. */
  running?: Record<CompanyAgent, number>;
  /** "Sign in to …" and "Update …" open a tab; the window closes Settings and selects it. */
  onSignIn?: (agent: "claude" | "codex") => void;
  onUpdate?: (agent: CompanyAgent) => void;
  /** `focusTerminal` is true when the element that had the focus before the page opened can't take it back. */
  onClose: (focusTerminal: boolean) => void;
};

/**
 * The Settings page in the live window: it covers the window as Departures does, and the session area stays mounted behind it.
 * Focus moves to the shown section's heading on open and on a section change. Esc and "Back to sessions" close it, unless
 * something inside already handled that Esc or the shell profiles hold unsaved edits; focus then returns to what had it before.
 */
export function LiveSettings({ endpoint, sync, section, onSection, agentsStart, initialCwd, announce, onProfilesSaved, onAgentsChanged, onNotifications, found = NONE, accountsRefresh = 0, companyRefresh = ZERO, running = ZERO, onSignIn = noop, onUpdate = noop, onClose }: Props) {
  const root = useRef<HTMLDivElement>(null);
  const opener = useRef<HTMLElement | null | undefined>(undefined);
  const profilesCanClose = useRef(true);
  const [profilesClosable, setProfilesClosable] = useState(true);
  const [mounted, setMounted] = useState<SettingsSection[]>([section]);
  const shown = mountedWith(mounted, section);
  if (shown !== mounted) setMounted(shown);

  // First, so the heading doesn't become the opener: it is read once, however often the effects run.
  useEffect(() => {
    if (opener.current === undefined) opener.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  }, []);
  useEffect(() => focusSettingsHeading(root.current), [section]);
  // Stray focus (a button that disables, a panel that swaps, a field that unmounts) comes back to the heading. Tab is the page's own.
  usePopupFocus(root, () => settingsFocusFallback(root.current) as HTMLElement | null, { tab: false });

  const close = () => {
    if (closeOrRefuse(profilesCanClose.current) === "refuse") {
      announce(SAVE_FIRST);
      onSection("profiles");
      return;
    }
    const usable = openerUsable(opener.current, document.body);
    onClose(!usable);
    if (usable) opener.current?.focus();
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === "Tab") {
      // A popup inside that keeps Tab for itself has already prevented it.
      if (e.defaultPrevented) return;
      const controls = visibleControls([...e.currentTarget.querySelectorAll<HTMLElement>(FOCUSABLE)]);
      const active = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      const target = settingsTabTarget(controls, active, e.shiftKey);
      e.preventDefault();
      if (target !== null) controls[target]?.focus();
      return;
    }
    if (settingsKey(e) !== "close") return;
    e.preventDefault();
    e.stopPropagation();
    if (!e.repeat) close();
  };

  const render = (id: SettingsSection): ReactNode => {
    switch (id) {
      case "appearance":
        return <AppearanceSection announce={announce} />;
      case "terminal":
        return <TerminalSection announce={announce} />;
      case "notifications":
        return <NotificationsSection sync={sync} announce={announce} />;
      case "profiles":
        return (
          endpoint && (
            <>
              <ProfilesSection
                endpoint={endpoint}
                announce={announce}
                onSaved={onProfilesSaved}
                onCanClose={(can) => {
                  profilesCanClose.current = can;
                  setProfilesClosable(can);
                }}
              />
              {!profilesClosable && <p className="field__help profiles__saved">{SAVE_FIRST}</p>}
            </>
          )
        );
      case "history":
        return <HistorySection sync={sync} announce={announce} />;
      case "projects":
        return endpoint && <ProjectsSection key={initialCwd ?? ""} endpoint={endpoint} initialCwd={initialCwd} announce={announce} />;
      case "agents":
        return endpoint && <AgentsSection endpoint={endpoint} sync={sync} startWith={agentsStart} announce={announce} onChanged={onAgentsChanged} onNotifications={onNotifications} />;
      case "accounts":
        return endpoint && <AccountsSection endpoint={endpoint} found={found} onSignIn={onSignIn} refresh={accountsRefresh} announce={announce} />;
      case "claude":
      case "codex":
      case "antigravity":
        return endpoint && <CompanySection endpoint={endpoint} agent={id} name={COMPANY_NAMES[id]} running={running[id]} onUpdate={onUpdate} refresh={companyRefresh[id]} announce={announce} />;
      case "advanced":
        return endpoint && <AdvancedSection endpoint={endpoint} announce={announce} />;
    }
  };

  return (
    <div ref={root} className="live-settings" role="dialog" aria-modal="true" aria-label="Settings" tabIndex={-1} onKeyDown={onKeyDown}>
      <SettingsShell section={section} onSection={onSection} onClose={close} sections={shownSections(found)}>
        <SettingsPanels section={section} mounted={shown} render={render} />
      </SettingsShell>
    </div>
  );
}
