import { useEffect, useId, useRef, type KeyboardEvent, type ReactNode } from "react";
import { AgentMark } from "../../components/AgentMark/AgentMark";
import { AuxGlyph } from "../../components/StatusGlyph/StatusGlyph";
import { ADMIN_LABEL, ADMIN_MODE_REASON, BYPASS_ACK_TEXT, WAITING_FOR_ADMIN } from "../../lib/adminCopy";
import type { Os } from "../../lib/keymap";
import { tabbableIn } from "../../lib/focusGuard";
import { usePopupFocus } from "../../lib/usePopupFocus";
import type { ConfirmScripts } from "../scripts/confirmScripts";
import { ScriptsTrust } from "../scripts/ScriptsTrust";
import { CommandList } from "../tasks/CommandList";
import { branchHint } from "../tasks/taskName";
import { headerDanger, modeLabel } from "../window/SessionHeader";
import type { FolderInfo } from "../../generated/FolderInfo";
import { filterProjects, highlight, pickByLetter, projectLabel, type Project, type Range } from "./filter";
import { browseKey, folderEnter, folderRows, rowEnabled, type FolderRow, type PathCheck } from "./folderRows";
import {
  LAUNCH_AGENTS,
  LAUNCH_SHELLS,
  MODES,
  PACES,
  adminChip,
  launcherSentence,
  programById,
  rememberedNote,
  type LaunchProgram,
  type LaunchShell,
  type LauncherChoice,
  type Remembered,
} from "./launchChoice";
import { effectiveAgent, missingLine, type ShellOffer } from "./shellAgents";
import { effectiveMode, nextOffered, offeredMode } from "../modes/sessionModes";
import { adminKeyClaimed, focusFellOut, listNavKeys, stepActive } from "./launcherKeys";
import "./Launcher.css";
import "../sessions/CloseBar.css";

/** The confirm step for a task: waiting for the core's plan, the commands it will run, or why it cannot be made. */
export type TaskConfirm =
  | { state: "loading" }
  /** `commands` are the git commands exactly as the core will run them; `scripts` is what the repo's marshell.json adds, shown apart from them. */
  | { state: "ready"; commands: string[]; scripts?: ConfirmScripts }
  | { state: "refused"; reason: string };

export type LauncherView = {
  choice: LauncherChoice;
  /** Every folder the launcher knows, most recently used first; the chip and the sentence read from it. */
  projects: Project[];
  /** Non-null when the folder chip is open for typing; the listbox shows `filterProjects(query, projects)`. */
  query: string | null;
  /** What the core said about the typed path, shown as the first row; null or absent when the text only filters. */
  pathCheck?: PathCheck | null;
  /** The highlighted option of whichever list is open. */
  activeIndex: number;
  /** True while the shell chip's list is open. */
  shellsOpen?: boolean;
  /** True while the agent chip's list is open. */
  agentsOpen?: boolean;
  remembered?: Remembered;
  prompt?: string;
  /** The Administrator chip is on. Launcher state only: it is not part of `choice` and is never remembered. */
  elevated?: boolean;
  /** The administrator Bypass warning was explicitly acknowledged for this program. */
  bypassAcknowledged?: boolean;
  /** The number the new session takes: the lowest one nobody holds. */
  nextNumber?: number;
  /** The task name field's text. The field shows while the worktree chip is on in a folder in git; without this it never shows. */
  taskName?: string;
  /** When set, the confirm view replaces the sentence, the lists and the prompt line. */
  confirm?: TaskConfirm;
};

type Props = {
  view: LauncherView;
  onChange?: (patch: Partial<LauncherChoice>) => void;
  onQuery?: (q: string | null) => void;
  onPick?: (projectId: string) => void;
  /** The typed path exists: use that folder. */
  onUsePath?: (folder: FolderInfo) => void;
  /** Open the native folder picker (the Browse… row, or Ctrl+O in the folder field). */
  onBrowse?: () => void;
  onShells?: (open: boolean) => void;
  onAgents?: (open: boolean) => void;
  onActive?: (index: number) => void;
  onPrompt?: (text: string) => void;
  onStart?: () => void;
  /** The Administrator chip was toggled (the chip, or Alt+A anywhere in the sheet). */
  onElevated?: (on: boolean) => void;
  onBypassAck?: (on: boolean) => void;
  onTaskName?: (text: string) => void;
  /** Create and start, in the confirm view. */
  onConfirm?: () => void;
  /** Back to the sentence, in the confirm view. */
  onBack?: () => void;
  /** The trust card's first button: allow this repo's scripts, then go on. */
  onAllow?: () => void;
  /** The trust card's second button: go on without the scripts. */
  onSkipScripts?: () => void;
  onClose?: () => void;
  /** False hides the pace chip (the live app, until pace has an effect); the mode chip always shows for an agent, and the worktree chip then shows only for a folder in git. Default true. */
  paceChip?: boolean;
  /** A create is in flight (not just the lists loading). With the Administrator chip on, the launcher then says it waits for approval. */
  pending?: boolean;
  /** The platform; the Administrator chip is available on Windows only. Default "windows". */
  os?: Os;
  /** What the shell chip lists. Defaults to `LAUNCH_SHELLS`. */
  shells?: LaunchShell[];
  /** The agents the agent chip can list. Defaults to `LAUNCH_AGENTS`. */
  agents?: LaunchProgram[];
  /** What the chosen shell offers (`offerFor`). Absent means every agent in `agents`. */
  offer?: ShellOffer;
  /** Why the core refused the launch, shown under Start. */
  error?: string;
  /** A launch is in flight: Start is disabled and keeps its label. In the confirm view it is the worktree being made. */
  busy?: boolean;
  /** The confirm view's one status line while busy: the worktree being made, then the program starting. Default "Creating the worktree…". */
  busyText?: string;
};

const PATH_VERB: Record<PathCheck["state"], string> = { found: "Use", missing: "No folder at", checking: "Checking", failed: "Couldn't check" };

const CONFIRM_TITLE = "Start a task in a new worktree";

const Marked = ({ text, ranges }: { text: string; ranges: Range[] }) => (
  <>
    {highlight(text, ranges).map((s, i) =>
      s.hit ? (
        <mark key={i} dir="auto">
          {s.text}
        </mark>
      ) : (
        <span key={i}>{s.text}</span>
      ),
    )}
  </>
);

const cycle = <T,>(all: T[], now: T) => all[(all.indexOf(now) + 1) % all.length]!;

/**
 * The launcher sheet (docs/PLAN.md "Launch a session"): one sentence of chips and a prompt line. Tab moves between the
 * chips, typing in the folder chip filters folders, letters pick values, Enter starts. The first chip picks the shell and the
 * second the agent that shell offers, or Just the shell, which is a plain terminal with no mode or pace chip and no prompt line
 * (it can still run in a worktree). The prompt line is set like the one in every pane (a flat command line), in the vendor's colour for an
 * agent. Presentational: the parent owns the state.
 */
export function Launcher({ view, onChange, onQuery, onPick, onUsePath, onBrowse, onShells, onAgents, onActive, onPrompt, onStart, onElevated, onBypassAck, onTaskName, onConfirm, onBack, onAllow, onSkipScripts, onClose, paceChip = true, os = "windows", pending = false, shells = LAUNCH_SHELLS, agents = LAUNCH_AGENTS, offer: offerProp, error, busy = false, busyText = "Creating the worktree…" }: Props) {
  const { choice, projects, query, activeIndex, remembered, nextNumber, taskName, confirm } = view;
  const project = projects.find((p) => p.id === choice.projectId);
  const offer: ShellOffer = offerProp ?? { state: "ready", offered: agents, missing: [] };
  const shell: LaunchShell = shells.find((x) => x.id === choice.shell) ?? shells[0] ?? { id: choice.shell, label: "…", kind: "program", family: "posix" };
  // The agent list: every agent while the shell is being asked, none once it could not be reached, else what it offers. Just the shell is always row 0.
  const listed = offer.state === "ready" ? offer.offered : offer.state === "checking" ? agents : [];
  const agentId = effectiveAgent(choice, offer);
  const program = agentId === null ? undefined : programById(agentId, agents);
  const agentRun = program !== undefined;
  const checking = offer.state === "checking";
  const cmdShell = shell.family === "cmd";
  // A worktree needs git: without the pace chip the chip is offered only for a folder that has a branch. Any program can use one.
  const inGit = project?.branch !== undefined;
  const showWorktree = paceChip || inGit;
  const showTask = showWorktree && choice.worktree && inGit && taskName !== undefined;
  // The modes this agent offers; none means no mode chip. The chosen mode shown is one of them.
  const modes = program?.modes ?? MODES;
  const admin = adminChip(view.elevated, os, program?.adminUnavailable);
  // Administrator starts request Manual and lock the chip; an agent's own setting may still make that Bypass.
  const modeLocked = admin.on && agentRun;
  const pickedMode = modeLocked ? "manual" : (offeredMode(program?.modes, choice.mode) ?? choice.mode);
  const mode = effectiveMode(pickedMode, program?.skipsAll !== undefined);
  const settingBypass = program?.skipsAll;
  const needsAck = admin.on && agentRun && (mode === "bypass" || program?.adminSetupFailed === true);
  // While the shell is asked, only an agent waits: it may be one the shell lacks. Just the shell always starts.
  const canStart = !busy && !(checking && agentRun) && !(admin.on && program?.adminSetupPending) && (!needsAck || view.bypassAcknowledged === true);
  const start = () => {
    if (canStart) onStart?.();
  };
  const note = agentRun && !modeLocked ? rememberedNote(project?.name ?? "this folder", remembered) : null;
  const folderOpen = query !== null;
  const shellsOpen = !folderOpen && view.shellsOpen === true;
  const agentsOpen = !folderOpen && !shellsOpen && view.agentsOpen === true;
  const options = folderOpen ? filterProjects(query, projects) : [];
  const rows = folderOpen ? folderRows(view.pathCheck ?? null, options) : [];
  const listSize = shellsOpen ? shells.length : agentsOpen ? listed.length + 1 : rows.length;
  const agentLabels = ["Just the shell", ...listed.map((p) => p.label)];
  const agentIndex = agentId === null ? 0 : listed.findIndex((p) => p.id === agentId) + 1;
  const pickAgent = (i: number) => onChange?.({ agent: i <= 0 ? null : (listed[i - 1]?.id ?? null) });
  const missing = offer.state === "ready" ? missingLine(shell.label, offer.missing) : null;
  // A row's click and Enter do the same; a row that can't be taken does nothing.
  const takeRow = (row: FolderRow | undefined) => {
    const act = folderEnter(row);
    if (act?.kind === "pick") onPick?.(act.id);
    else if (act?.kind === "use") onUsePath?.(act.folder);
    else if (act?.kind === "browse") onBrowse?.();
  };
  // Ids belong to one launcher (the review page shows many at once).
  const uid = useId();
  const id = (name: string) => `${uid}-${name}`;
  const toggleAdmin = () => {
    if (admin.enabled) onElevated?.(!admin.on);
  };

  // Esc or a pick closes the folder field, which swaps an input for a button; focus must not fall to the page, or Esc and Enter go dead.
  const folderChip = useRef<HTMLButtonElement>(null);
  const wasOpen = useRef(folderOpen);
  useEffect(() => {
    const body = typeof document !== "undefined" && (document.activeElement === null || document.activeElement === document.body);
    if (focusFellOut({ wasOpen: wasOpen.current, isOpen: folderOpen, activeIsBody: body })) folderChip.current?.focus();
    wasOpen.current = folderOpen;
  }, [folderOpen]);

  const letter = (labels: string[], current: number, key: string, set: (i: number) => void) => {
    const i = pickByLetter(labels, key, current);
    if (i >= 0) set(i);
  };

  // The confirm view: Enter on the default creates. Focus stays on Create (it is aria-disabled, never `disabled`, while the plan
  // or the launch is pending, so it keeps focus and an Enter or key repeat does nothing); only a refusal moves it to Back.
  const createButton = useRef<HTMLButtonElement>(null);
  const backButton = useRef<HTMLButtonElement>(null);
  const confirmState = confirm?.state;
  // A repo's scripts waiting for a yes: its card replaces Create and start, and focus goes to its heading, so Enter cannot allow by reflex.
  const scripts = confirm?.state === "ready" ? confirm.scripts : undefined;
  const asking = scripts?.kind === "ask" ? scripts : undefined;
  const askKey = asking === undefined ? "" : asking.only ? "only" : "ask";
  const trustHeading = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    if (confirmState === undefined) return;
    if (askKey !== "") trustHeading.current?.focus();
    else (confirmState === "refused" ? backButton.current : createButton.current)?.focus();
  }, [confirmState, askKey]);
  const canCreate = confirm?.state === "ready" && !busy && asking === undefined;
  const create = () => {
    if (canCreate) onConfirm?.();
  };
  const back = () => {
    // Once the task exists, going back would leave a worktree with no session: the card's two buttons are the only ways on.
    if (!busy && asking?.only !== true) onBack?.();
  };

  // The prompt line is missing in the confirm view and for a program with no first command; then the first control is the way back.
  const sheet = useRef<HTMLDivElement>(null);
  usePopupFocus(sheet, () => sheet.current?.querySelector<HTMLElement>(".launcher__prompt") ?? (sheet.current ? tabbableIn(sheet.current)[0] : null));

  const onSheetKey = (e: KeyboardEvent<HTMLDivElement>) => {
    if (confirm) {
      if (e.key === "Escape") {
        e.preventDefault();
        back();
      } else if (e.key === "Enter" && !e.shiftKey && (e.target as HTMLElement).closest("button") === null) {
        e.preventDefault();
        create();
      }
      return;
    }
    if (adminKeyClaimed(e, admin.enabled)) {
      e.preventDefault();
      toggleAdmin();
      return;
    }
    const listOpen = folderOpen || shellsOpen || agentsOpen;
    if (e.key === "Escape") {
      e.preventDefault();
      if (folderOpen) onQuery?.(null);
      else if (shellsOpen) onShells?.(false);
      else if (agentsOpen) onAgents?.(false);
      else onClose?.();
      return;
    }
    if (folderOpen && browseKey(e)) {
      e.preventDefault();
      onBrowse?.();
      return;
    }
    if (listOpen && listSize > 0 && listNavKeys(shellsOpen ? "shells" : agentsOpen ? "agents" : "folder").includes(e.key)) {
      e.preventDefault();
      onActive?.(stepActive(activeIndex, e.key, listSize));
      return;
    }
    if (e.key !== "Enter" || e.shiftKey) return;
    const onButton = (e.target as HTMLElement).closest("button") !== null;
    if (listOpen) {
      e.preventDefault();
      if (shellsOpen) {
        const s = shells[activeIndex];
        if (s) onChange?.({ shell: s.id });
        onShells?.(false);
      } else if (agentsOpen) {
        pickAgent(activeIndex);
        onAgents?.(false);
      } else {
        takeRow(rows[activeIndex]);
      }
    } else if (!onButton) {
      e.preventDefault();
      start();
    }
  };

  const chipKeys = (labels: string[], current: number, set: (i: number) => void) => (e: KeyboardEvent<HTMLButtonElement>) => letter(labels, current, e.key, set);

  return (
    <div className="launcher" data-open={folderOpen || shellsOpen || agentsOpen || undefined}>
      <div className="launcher__backdrop" />
      <div
        ref={sheet}
        className="launcher__sheet"
        role="dialog"
        aria-modal="true"
        aria-label={confirm ? CONFIRM_TITLE : "Start a session"}
        aria-describedby={id("desc")}
        data-agent={program?.kind}
        data-manual={!agentRun || undefined}
        onKeyDown={onSheetKey}
      >
        {confirm ? (
          <>
            <h2 className="launcher__title">{CONFIRM_TITLE}</h2>
            {confirm.state === "loading" && (
              <p className="launcher__pending">Checking the folder…</p>
            )}
            {confirm.state === "refused" && (
              <p className="launcher__error" role="alert">
                {confirm.reason}
              </p>
            )}
            {asking && <ScriptsTrust scripts={asking.scripts} allowLabel={asking.allowLabel} cancelLabel={asking.cancelLabel} busy={busy} headingRef={trustHeading} onAllow={() => onAllow?.()} onCancel={() => onSkipScripts?.()} />}
            {confirm.state === "ready" && asking?.only !== true && (
              <>
                <p className="launcher__lede">Marshell will run these commands, then start {program?.label ?? shell.label} in the new folder.</p>
                <CommandList commands={scripts?.kind === "setup" ? [...confirm.commands, `setup: ${scripts.script}`] : confirm.commands} />
                {scripts?.kind === "problem" && (
                  <p className="launcher__error" dir="auto">
                    {scripts.text}
                  </p>
                )}
              </>
            )}
            {busy && (
              <p className="launcher__pending" role="status">
                {busyText}
              </p>
            )}
            {asking?.only !== true && (
              <div className="launcher__actions">
                {asking === undefined && (
                  <button ref={createButton} type="button" className="launcher__start" aria-disabled={!canCreate || undefined} onClick={create}>
                    Create and start
                  </button>
                )}
                <button ref={backButton} type="button" className="launcher__back" aria-disabled={busy || undefined} onClick={back}>
                  Back
                </button>
                <span className="launcher__hint">
                  {asking === undefined && (
                    <>
                      <kbd>Enter</kbd> creates {"·"}{" "}
                    </>
                  )}
                  <kbd>Esc</kbd> goes back
                </span>
              </div>
            )}
          </>
        ) : (
          <>
            <p className="launcher__sentence">
              <button
                type="button"
                className="launcher__chip"
                role="combobox"
                aria-label={`Shell: ${shell.label}`}
                aria-haspopup="listbox"
                aria-expanded={shellsOpen}
                aria-controls={shellsOpen ? id("shells") : undefined}
                aria-activedescendant={shellsOpen ? id(`shell-${activeIndex}`) : undefined}
                onClick={() => onShells?.(!shellsOpen)}
                onKeyDown={chipKeys(
                  shells.map((x) => x.label),
                  shells.findIndex((x) => x.id === shell.id),
                  (i) => onChange?.({ shell: shells[i]!.id }),
                )}
              >
                <span aria-hidden="true" className="launcher__mark">
                  <AgentMark agent={shell.kind} size={12} />
                </span>
                {shell.label}
              </button>
              <span>{agentRun ? " running " : " with "}</span>
              <button
                type="button"
                className="launcher__chip"
                role="combobox"
                aria-label={`Agent: ${program?.label ?? "none"}`}
                aria-haspopup="listbox"
                aria-expanded={agentsOpen}
                aria-controls={agentsOpen ? id("agents") : undefined}
                aria-activedescendant={agentsOpen ? id(`agent-${activeIndex}`) : undefined}
                aria-describedby={checking ? id("checking") : undefined}
                onClick={() => onAgents?.(!agentsOpen)}
                onKeyDown={chipKeys(agentLabels, agentIndex, pickAgent)}
              >
                <span aria-hidden="true" className="launcher__mark">
                  <AgentMark agent={program?.kind ?? shell.kind} size={12} />
                </span>
                {program?.label ?? "no agent"}
              </button>
              <span> in </span>
              {folderOpen ? (
                <input
                  className="launcher__input"
                  role="combobox"
                  aria-expanded={rows.length > 0}
                  aria-controls={rows.length > 0 ? id("list") : undefined}
                  aria-activedescendant={rows.length > 0 ? id(`opt-${activeIndex}`) : undefined}
                  aria-label="Folder"
                  value={query}
                  onChange={(e) => onQuery?.(e.target.value)}
                  autoFocus
                />
              ) : (
                <button ref={folderChip} type="button" className="launcher__chip" aria-haspopup="listbox" onClick={() => onQuery?.("")}>
                  {project ? projectLabel(project) : "Choose a folder"}
                </button>
              )}
              {agentRun && modes.length > 0 && (
                <>
                  <span> {"·"} </span>
                  <button
                    type="button"
                    className="launcher__chip"
                    aria-label={`Mode: ${modeLabel(mode)}`}
                    aria-disabled={modeLocked || undefined}
                    aria-describedby={modeLocked ? id("admin-mode") : undefined}
                    onClick={() => !modeLocked && onChange?.({ mode: nextOffered(program?.modes, pickedMode) })}
                    onKeyDown={modeLocked ? undefined : chipKeys(modes, modes.indexOf(pickedMode), (i) => onChange?.({ mode: modes[i]! }))}
                  >
                    {headerDanger(mode) && (
                      <span className="launcher__risk" aria-hidden="true">
                        {"⚠ "}
                      </span>
                    )}
                    {modeLabel(mode)}
                  </button>
                </>
              )}
              {agentRun && paceChip && (
                <>
                  <span> {"·"} </span>
                  <button
                    type="button"
                    className="launcher__chip"
                    aria-label={`Pace: ${choice.pace}`}
                    onClick={() => onChange?.({ pace: cycle(PACES, choice.pace) })}
                    onKeyDown={chipKeys(PACES, PACES.indexOf(choice.pace), (i) => onChange?.({ pace: PACES[i]! }))}
                  >
                    {choice.pace}
                  </button>
                </>
              )}
              {showWorktree && (
                <>
                  <span> {"·"} </span>
                  <button type="button" className="launcher__chip" role="checkbox" aria-checked={choice.worktree} onClick={() => onChange?.({ worktree: !choice.worktree })}>
                    <span className="launcher__box" aria-hidden="true" data-checked={choice.worktree || undefined} />
                    worktree
                  </button>
                </>
              )}
              <span> {"·"} </span>
              <button
                type="button"
                className="launcher__chip"
                role="checkbox"
                aria-checked={admin.on}
                aria-disabled={admin.enabled ? undefined : true}
                aria-describedby={admin.enabled ? undefined : id("admin-off")}
                aria-keyshortcuts={admin.enabled ? "Alt+A" : undefined}
                onClick={toggleAdmin}
              >
                <span className="launcher__box" aria-hidden="true" data-checked={admin.on || undefined} />
                {admin.on && (
                  <span aria-hidden="true" className="launcher__shield">
                    <AuxGlyph kind="elevated" />
                  </span>
                )}
                {ADMIN_LABEL}
              </button>
            </p>

            {!admin.enabled && (
              <p className="launcher__reason" id={id("admin-off")}>
                {admin.reason}
              </p>
            )}
            {modeLocked && (
              <p className="launcher__reason" id={id("admin-mode")}>
                {program?.adminSetupFailed ? "Marshell couldn't read Antigravity's settings, so it may skip every prompt." : program?.skipsAll ? `${program.label} runs as Bypass because its own ${program.skipsAll.setting} setting skips every prompt.` : ADMIN_MODE_REASON}
              </p>
            )}

            {settingBypass && !modeLocked && <p className="launcher__reason">{program?.label} runs as Bypass because its own {settingBypass.setting} setting skips every prompt.</p>}
            {admin.on && program?.adminSetupPending && <p className="launcher__reason">Checking Antigravity's settings…</p>}

            {shellsOpen && (
              <div className="launcher__programs" id={id("shells")} role="listbox" aria-label="Shells">
                <div className="launcher__choices">
                  {shells.map((x, i) => (
                    <div
                      key={x.id}
                      id={id(`shell-${i}`)}
                      role="option"
                      aria-selected={i === activeIndex}
                      className="launcher__program"
                      data-active={i === activeIndex || undefined}
                      data-current={x.id === shell.id || undefined}
                      onClick={() => {
                        onChange?.({ shell: x.id });
                        onShells?.(false);
                      }}
                    >
                      <span aria-hidden="true" className="launcher__mark">
                        <AgentMark agent={x.kind} size={16} />
                      </span>
                      {x.label}
                    </div>
                  ))}
                </div>
              </div>
            )}

            {agentsOpen && (
              <>
                <div className="launcher__programs" id={id("agents")} role="listbox" aria-label={`Agents in ${shell.label}`}>
                  <div className="launcher__choices">
                    {[null, ...listed].map((p, i) => (
                      <div
                        key={p === null ? "none" : p.id}
                        id={id(`agent-${i}`)}
                        role="option"
                        aria-selected={i === activeIndex}
                        className="launcher__program"
                        data-active={i === activeIndex || undefined}
                        data-current={i === agentIndex || undefined}
                        onClick={() => {
                          pickAgent(i);
                          onAgents?.(false);
                        }}
                      >
                        <span aria-hidden="true" className="launcher__mark">
                          <AgentMark agent={p === null ? shell.kind : p.kind} size={16} />
                        </span>
                        {p === null ? "Just the shell" : p.label}
                      </div>
                    ))}
                  </div>
                </div>
                {/* Outside the listbox: only options may live inside one. */}
                {missing !== null && <p className="launcher__reason">{missing}</p>}
              </>
            )}

            {checking && (
              <p className="launcher__reason" id={id("checking")}>
                Checking which agents {shell.label} has…
              </p>
            )}
            {offer.state === "unreached" && <p className="launcher__reason">Marshell couldn't reach {shell.label}, so only the shell is offered.</p>}

            {folderOpen && options.length === 0 && !view.pathCheck && (
              // Outside the listbox: only options may live inside one.
              <p className="launcher__empty" role="status">
                No folder matches {"“"}
                {query}
                {"”"}
              </p>
            )}
            {folderOpen && rows.length > 0 && (
              <ul className="launcher__list" id={id("list")} role="listbox" aria-label="Folders">
                {rows.map((row, i) => (
                  <li
                    key={row.kind === "project" ? row.hit.item.id : row.kind}
                    id={id(`opt-${i}`)}
                    role="option"
                    aria-selected={i === activeIndex}
                    aria-disabled={rowEnabled(row) ? undefined : true}
                    className="launcher__opt"
                    data-active={i === activeIndex || undefined}
                    onClick={() => takeRow(row)}
                  >
                    {row.kind === "project" && (
                      <>
                        <span className="launcher__opt-name" dir="auto">
                          <Marked text={projectLabel(row.hit.item)} ranges={row.hit.ranges} />
                        </span>
                        <span className="launcher__opt-path" dir="auto">
                          {row.hit.item.path}
                        </span>
                      </>
                    )}
                    {row.kind === "path" && (
                      <>
                        <span className="launcher__opt-name" dir="auto">
                          {PATH_VERB[row.check.state]}
                        </span>{" "}
                        <span className="launcher__opt-path" dir="auto">
                          {row.check.path}
                          {row.check.state === "checking" ? "…" : ""}
                        </span>
                      </>
                    )}
                    {row.kind === "browse" && (
                      <>
                        <span className="launcher__opt-name">Browse…</span>
                        <kbd>Ctrl+O</kbd>
                      </>
                    )}
                  </li>
                ))}
              </ul>
            )}

            {note && !folderOpen && !shellsOpen && !agentsOpen && <p className="launcher__note">{note}</p>}

            {agentRun && cmdShell && <p className="launcher__reason">Command Prompt can't pass a first prompt. Type it once {program?.label} starts.</p>}
            {agentRun && !cmdShell && !program?.noFirst && (
              <div className="launcher__line">
                {/* A drawn chevron, as in the prompt line: a drawing needs 3:1, a text glyph would need 4.5:1 in the vendor colour. */}
                <svg className="launcher__glyph" width="12" height="16" viewBox="0 0 12 16" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M3.5 3.5L8.5 8l-5 4.5" />
                </svg>
                <textarea
                  className="launcher__prompt"
                  aria-label="First prompt (optional)"
                  placeholder={program?.first}
                  rows={2}
                  defaultValue={view.prompt}
                  onChange={(e) => onPrompt?.(e.target.value)}
                />
              </div>
            )}

            {showTask && (
              <div className="launcher__task">
                <label className="launcher__task-label" htmlFor={id("task")}>
                  Task name
                </label>
                <input
                  id={id("task")}
                  className="launcher__task-input"
                  type="text"
                  value={taskName}
                  spellCheck={false}
                  autoComplete="off"
                  aria-describedby={id("task-hint")}
                  onChange={(e) => onTaskName?.(e.target.value)}
                />
                <p className="launcher__task-hint" id={id("task-hint")}>
                  {branchHint(taskName)}
                </p>
              </div>
            )}

            {needsAck && (
              <label className="close-bar__ack" htmlFor={id("bypass-ack")}>
                <input id={id("bypass-ack")} type="checkbox" checked={view.bypassAcknowledged === true} onChange={(e) => onBypassAck?.(e.target.checked)} />
                <span>{BYPASS_ACK_TEXT}</span>
              </label>
            )}

            <footer className="launcher__footer">
              <span className="launcher__hint">
                <kbd>Tab</kbd> moves between choices {"·"} type a letter to pick {"·"} <kbd>Esc</kbd> closes
              </span>
              <span className="launcher__go">
                {nextNumber !== undefined && <span className="launcher__next">Session {nextNumber}</span>}
                <button type="button" className="launcher__start" disabled={!canStart} onClick={start}>
                  Start <kbd>Enter</kbd>
                </button>
                {pending && admin.on && !confirm && (
                  <span className="launcher__pending" role="status">
                    {WAITING_FOR_ADMIN}
                  </span>
                )}
              </span>
            </footer>
          </>
        )}
        {error && (
          <p className="launcher__error" role="alert">
            {error}
          </p>
        )}
        <p className="launcher__sr" id={id("desc")}>
          {launcherSentence({ ...choice, agent: agentId, mode }, projects, agents, admin.on, shells)}
          {nextNumber !== undefined ? `. It becomes session ${nextNumber}` : ""}
        </p>
      </div>
    </div>
  );
}
