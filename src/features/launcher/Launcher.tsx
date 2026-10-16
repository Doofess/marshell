import { useEffect, useId, useRef, type KeyboardEvent } from "react";
import { AgentMark } from "../../components/AgentMark/AgentMark";
import { CommandList } from "../tasks/CommandList";
import { branchHint } from "../tasks/taskName";
import { filterProjects, highlight, pickByLetter, projectLabel, type Project, type Range } from "./filter";
import {
  LAUNCH_GROUPS,
  LAUNCH_PROGRAMS,
  MODES,
  PACES,
  isAgentProgram,
  launcherSentence,
  programById,
  programsIn,
  rememberedNote,
  type LaunchProgram,
  type LauncherChoice,
  type Remembered,
} from "./launchChoice";
import { focusFellOut, listNavKeys, stepActive } from "./launcherKeys";
import "./Launcher.css";

/** The confirm step for a task: waiting for the core's plan, the commands it will run, or why it cannot be made. */
export type TaskConfirm = { state: "loading" } | { state: "ready"; commands: string[] } | { state: "refused"; reason: string };

export type LauncherView = {
  choice: LauncherChoice;
  /** Every folder the launcher knows, most recently used first; the chip and the sentence read from it. */
  projects: Project[];
  /** Non-null when the folder chip is open for typing; the listbox shows `filterProjects(query, projects)`. */
  query: string | null;
  /** The highlighted option of whichever list is open. */
  activeIndex: number;
  /** True while the first chip's list of agents, shells and tools is open. */
  programsOpen?: boolean;
  remembered?: Remembered;
  prompt?: string;
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
  onPrograms?: (open: boolean) => void;
  onActive?: (index: number) => void;
  onPrompt?: (text: string) => void;
  onStart?: () => void;
  onTaskName?: (text: string) => void;
  /** Create and start, in the confirm view. */
  onConfirm?: () => void;
  /** Back to the sentence, in the confirm view. */
  onBack?: () => void;
  onClose?: () => void;
  /** False hides the mode and pace chips (the live app, until phases 3 and 6 give them an effect); the worktree chip then shows only for a folder in git. Default true. */
  chips?: boolean;
  /** What the first chip lists. Defaults to every program in `LAUNCH_PROGRAMS`. */
  programs?: LaunchProgram[];
  /** Why the core refused the launch, shown under Start. */
  error?: string;
  /** A launch is in flight: Start is disabled and keeps its label. In the confirm view it is the worktree being made. */
  busy?: boolean;
};

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
 * chips, typing in the folder chip filters folders, letters pick values, Enter starts. The first chip lists AI agents,
 * shells and tools: a shell or tool is a plain terminal, so it has no mode, pace or worktree chip and its line asks for a
 * first command. The prompt line is set like the one in every pane (a flat command line), in the vendor's colour for an
 * agent. Presentational: the parent owns the state.
 */
export function Launcher({ view, onChange, onQuery, onPick, onPrograms, onActive, onPrompt, onStart, onTaskName, onConfirm, onBack, onClose, chips = true, programs = LAUNCH_PROGRAMS, error, busy = false }: Props) {
  const { choice, projects, query, activeIndex, remembered, nextNumber, taskName, confirm } = view;
  const project = projects.find((p) => p.id === choice.projectId);
  const agentRun = isAgentProgram(choice.program, programs);
  // A worktree needs git: without chips the chip is offered only for a folder that has a branch.
  const inGit = project?.branch !== undefined;
  const showWorktree = agentRun && (chips || inGit);
  const showTask = showWorktree && choice.worktree && inGit && taskName !== undefined;
  const program = programById(choice.program, programs) ?? programs[0] ?? LAUNCH_PROGRAMS[0]!;
  const note = agentRun ? rememberedNote(project?.name ?? "this folder", remembered) : null;
  const folderOpen = query !== null;
  const programsOpen = !folderOpen && view.programsOpen === true;
  const options = folderOpen ? filterProjects(query, projects) : [];
  const listSize = programsOpen ? programs.length : options.length;
  // Ids belong to one launcher (the review page shows many at once).
  const uid = useId();
  const id = (name: string) => `${uid}-${name}`;

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

  // The confirm view: Enter on the default creates, and focus goes to whichever button can be used (a disabled one cannot hold it).
  const createButton = useRef<HTMLButtonElement>(null);
  const backButton = useRef<HTMLButtonElement>(null);
  const confirmState = confirm?.state;
  useEffect(() => {
    if (confirmState === undefined) return;
    (confirmState === "ready" && !busy ? createButton.current : backButton.current)?.focus();
  }, [confirmState, busy]);

  const onSheetKey = (e: KeyboardEvent<HTMLDivElement>) => {
    if (confirm) {
      if (e.key === "Escape") {
        e.preventDefault();
        if (!busy) onBack?.();
      } else if (e.key === "Enter" && !e.shiftKey && (e.target as HTMLElement).closest("button") === null) {
        e.preventDefault();
        if (confirm.state === "ready" && !busy) onConfirm?.();
      }
      return;
    }
    const listOpen = folderOpen || programsOpen;
    if (e.key === "Escape") {
      e.preventDefault();
      if (folderOpen) onQuery?.(null);
      else if (programsOpen) onPrograms?.(false);
      else onClose?.();
      return;
    }
    if (listOpen && listSize > 0 && listNavKeys(programsOpen ? "programs" : "folder").includes(e.key)) {
      e.preventDefault();
      onActive?.(stepActive(activeIndex, e.key, listSize));
      return;
    }
    if (e.key !== "Enter" || e.shiftKey) return;
    const onButton = (e.target as HTMLElement).closest("button") !== null;
    if (listOpen) {
      e.preventDefault();
      if (programsOpen) {
        const p = programs[activeIndex];
        if (p) onChange?.({ program: p.id });
        onPrograms?.(false);
      } else {
        const hit = options[activeIndex];
        if (hit) onPick?.(hit.item.id);
      }
    } else if (!onButton) {
      e.preventDefault();
      onStart?.();
    }
  };

  const chipKeys = (labels: string[], current: number, set: (i: number) => void) => (e: KeyboardEvent<HTMLButtonElement>) => letter(labels, current, e.key, set);

  return (
    <div className="launcher" data-open={folderOpen || programsOpen || undefined}>
      <div className="launcher__backdrop" />
      <div
        className="launcher__sheet"
        role="dialog"
        aria-modal="true"
        aria-label={confirm ? CONFIRM_TITLE : "Start a session"}
        aria-describedby={id("desc")}
        data-agent={agentRun ? program.kind : undefined}
        data-manual={!agentRun || undefined}
        onKeyDown={onSheetKey}
      >
        {confirm ? (
          <>
            <h2 className="launcher__title">{CONFIRM_TITLE}</h2>
            {confirm.state === "loading" && (
              <p className="launcher__pending" role="status">
                Checking the folder…
              </p>
            )}
            {confirm.state === "refused" && (
              <p className="launcher__error" role="alert">
                {confirm.reason}
              </p>
            )}
            {confirm.state === "ready" && (
              <>
                <p className="launcher__lede">Marshell will run these commands, then start {program.label} in the new folder.</p>
                <CommandList commands={confirm.commands} />
              </>
            )}
            {busy && (
              <p className="launcher__pending" role="status">
                Creating the worktree…
              </p>
            )}
            <div className="launcher__actions">
              <button ref={createButton} type="button" className="launcher__start" disabled={confirm.state !== "ready" || busy} onClick={onConfirm}>
                Create and start
              </button>
              <button ref={backButton} type="button" className="launcher__back" disabled={busy} onClick={onBack}>
                Back
              </button>
              <span className="launcher__hint">
                <kbd>Enter</kbd> creates {"·"} <kbd>Esc</kbd> goes back
              </span>
            </div>
          </>
        ) : (
          <>
            <p className="launcher__sentence">
              <button
                type="button"
                className="launcher__chip"
                role="combobox"
                aria-label={`Program: ${program.label}`}
                aria-haspopup="listbox"
                aria-expanded={programsOpen}
                aria-controls={programsOpen ? id("programs") : undefined}
                aria-activedescendant={programsOpen ? id(`prog-${activeIndex}`) : undefined}
                onClick={() => onPrograms?.(!programsOpen)}
                onKeyDown={chipKeys(
                  programs.map((p) => p.label),
                  programs.indexOf(program),
                  (i) => onChange?.({ program: programs[i]!.id }),
                )}
              >
                <span aria-hidden="true" className="launcher__mark">
                  <AgentMark agent={program.kind} size={12} />
                </span>
                {program.label}
              </button>
              <span> in </span>
              {folderOpen ? (
                <input
                  className="launcher__input"
                  role="combobox"
                  aria-expanded={options.length > 0}
                  aria-controls={options.length > 0 ? id("list") : undefined}
                  aria-activedescendant={options.length > 0 ? id(`opt-${activeIndex}`) : undefined}
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
              {agentRun && chips && (
                <>
                  <span> {"·"} </span>
                  <button
                    type="button"
                    className="launcher__chip"
                    aria-label={`Mode: ${choice.mode}`}
                    onClick={() => onChange?.({ mode: cycle(MODES, choice.mode) })}
                    onKeyDown={chipKeys(MODES, MODES.indexOf(choice.mode), (i) => onChange?.({ mode: MODES[i]! }))}
                  >
                    {choice.mode}
                  </button>
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
            </p>

            {programsOpen && (
              <div className="launcher__programs" id={id("programs")} role="listbox" aria-label="Agents, shells and tools">
                {LAUNCH_GROUPS.filter((g) => programsIn(g, programs).length > 0).map((g) => (
                  <div key={g} className="launcher__group" role="group" aria-labelledby={id(`group-${g}`)}>
                    <span className="launcher__group-name" id={id(`group-${g}`)}>
                      {g}
                    </span>
                    <div className="launcher__choices">
                      {programsIn(g, programs).map((p) => {
                        const i = programs.indexOf(p);
                        return (
                          <div
                            key={p.id}
                            id={id(`prog-${i}`)}
                            role="option"
                            aria-selected={i === activeIndex}
                            className="launcher__program"
                            data-active={i === activeIndex || undefined}
                            data-current={p.id === choice.program || undefined}
                            onClick={() => {
                              onChange?.({ program: p.id });
                              onPrograms?.(false);
                            }}
                          >
                            <span aria-hidden="true" className="launcher__mark">
                              <AgentMark agent={p.kind} size={16} />
                            </span>
                            {p.label}
                          </div>
                        );
                      })}
                    </div>
                  </div>
                ))}
              </div>
            )}

            {folderOpen && options.length === 0 && (
              // Outside the listbox: only options may live inside one.
              <p className="launcher__empty" role="status">
                No folder matches {"“"}
                {query}
                {"”"}
              </p>
            )}
            {folderOpen && options.length > 0 && (
              <ul className="launcher__list" id={id("list")} role="listbox" aria-label="Folders">
                {options.map((h, i) => (
                  <li
                    key={h.item.id}
                    id={id(`opt-${i}`)}
                    role="option"
                    aria-selected={i === activeIndex}
                    className="launcher__opt"
                    data-active={i === activeIndex || undefined}
                    onClick={() => onPick?.(h.item.id)}
                  >
                    <span className="launcher__opt-name" dir="auto">
                      <Marked text={projectLabel(h.item)} ranges={h.ranges} />
                    </span>
                    <span className="launcher__opt-path" dir="auto">
                      {h.item.path}
                    </span>
                  </li>
                ))}
              </ul>
            )}

            {note && !folderOpen && !programsOpen && <p className="launcher__note">{note}</p>}

            {!program.noFirst && (
              <div className="launcher__line">
                {/* A drawn chevron, as in the prompt line: a drawing needs 3:1, a text glyph would need 4.5:1 in the vendor colour. */}
                <svg className="launcher__glyph" width="12" height="16" viewBox="0 0 12 16" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M3.5 3.5L8.5 8l-5 4.5" />
                </svg>
                <textarea
                  className="launcher__prompt"
                  aria-label={agentRun ? "First prompt (optional)" : "First command (optional)"}
                  placeholder={program.first}
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

            <footer className="launcher__footer">
              <span className="launcher__hint">
                <kbd>Tab</kbd> moves between choices {"·"} type a letter to pick {"·"} <kbd>Esc</kbd> closes
              </span>
              <span className="launcher__go">
                {nextNumber !== undefined && <span className="launcher__next">Session {nextNumber}</span>}
                <button type="button" className="launcher__start" disabled={busy} onClick={onStart}>
                  Start <kbd>Enter</kbd>
                </button>
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
          {launcherSentence(choice, projects, programs)}
          {nextNumber !== undefined ? `. It becomes session ${nextNumber}` : ""}
        </p>
      </div>
    </div>
  );
}
