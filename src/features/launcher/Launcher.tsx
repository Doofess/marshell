import type { KeyboardEvent } from "react";
import { AgentMark } from "../../components/AgentMark/AgentMark";
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
  type LauncherChoice,
  type Remembered,
} from "./launchChoice";
import "./Launcher.css";

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
};

type Props = {
  view: LauncherView;
  onChange?: (patch: Partial<LauncherChoice>) => void;
  onQuery?: (q: string | null) => void;
  onPick?: (projectId: string) => void;
  onPrograms?: (open: boolean) => void;
  onActive?: (index: number) => void;
  onStart?: () => void;
  onClose?: () => void;
};

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
export function Launcher({ view, onChange, onQuery, onPick, onPrograms, onActive, onStart, onClose }: Props) {
  const { choice, projects, query, activeIndex, remembered, nextNumber } = view;
  const project = projects.find((p) => p.id === choice.projectId);
  const agentRun = isAgentProgram(choice.program);
  const program = programById(choice.program) ?? LAUNCH_PROGRAMS[0]!;
  const note = agentRun ? rememberedNote(project?.name ?? "this folder", remembered) : null;
  const folderOpen = query !== null;
  const programsOpen = !folderOpen && view.programsOpen === true;
  const options = folderOpen ? filterProjects(query, projects) : [];
  const listSize = programsOpen ? LAUNCH_PROGRAMS.length : options.length;

  const letter = (labels: string[], current: number, key: string, set: (i: number) => void) => {
    const i = pickByLetter(labels, key, current);
    if (i >= 0) set(i);
  };

  const onSheetKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const listOpen = folderOpen || programsOpen;
    if (e.key === "Escape") {
      e.preventDefault();
      if (folderOpen) onQuery?.(null);
      else if (programsOpen) onPrograms?.(false);
      else onClose?.();
      return;
    }
    if (listOpen && listSize > 0 && ["ArrowDown", "ArrowUp", "ArrowRight", "ArrowLeft"].includes(e.key)) {
      e.preventDefault();
      const step = e.key === "ArrowDown" || e.key === "ArrowRight" ? 1 : -1;
      onActive?.((activeIndex + step + listSize) % listSize);
      return;
    }
    if (e.key !== "Enter" || e.shiftKey) return;
    const onButton = (e.target as HTMLElement).closest("button") !== null;
    if (listOpen) {
      e.preventDefault();
      if (programsOpen) {
        const p = LAUNCH_PROGRAMS[activeIndex];
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
        aria-label="Start a session"
        aria-describedby="launcher-desc"
        data-agent={agentRun ? choice.program : undefined}
        data-manual={!agentRun || undefined}
        onKeyDown={onSheetKey}
      >
        <p className="launcher__sentence">
          <button
            type="button"
            className="launcher__chip"
            role="combobox"
            aria-label={`Program: ${program.label}`}
            aria-haspopup="listbox"
            aria-expanded={programsOpen}
            aria-controls={programsOpen ? "launcher-programs" : undefined}
            aria-activedescendant={programsOpen ? `launcher-prog-${activeIndex}` : undefined}
            onClick={() => onPrograms?.(!programsOpen)}
            onKeyDown={chipKeys(
              LAUNCH_PROGRAMS.map((p) => p.label),
              LAUNCH_PROGRAMS.indexOf(program),
              (i) => onChange?.({ program: LAUNCH_PROGRAMS[i]!.id }),
            )}
          >
            <span aria-hidden="true" className="launcher__mark">
              <AgentMark agent={program.id} size={12} />
            </span>
            {program.label}
          </button>
          <span> in </span>
          {folderOpen ? (
            <input
              className="launcher__input"
              role="combobox"
              aria-expanded={options.length > 0}
              aria-controls={options.length > 0 ? "launcher-list" : undefined}
              aria-activedescendant={options.length > 0 ? `launcher-opt-${activeIndex}` : undefined}
              aria-label="Folder"
              value={query}
              onChange={(e) => onQuery?.(e.target.value)}
              autoFocus
            />
          ) : (
            <button type="button" className="launcher__chip" aria-haspopup="listbox" onClick={() => onQuery?.("")}>
              {project ? projectLabel(project) : "Choose a folder"}
            </button>
          )}
          {agentRun && (
            <>
              <span> {"·"} </span>
              <button
                type="button"
                className="launcher__chip"
                onClick={() => onChange?.({ mode: cycle(MODES, choice.mode) })}
                onKeyDown={chipKeys(MODES, MODES.indexOf(choice.mode), (i) => onChange?.({ mode: MODES[i]! }))}
              >
                {choice.mode}
              </button>
              <span> {"·"} </span>
              <button
                type="button"
                className="launcher__chip"
                onClick={() => onChange?.({ pace: cycle(PACES, choice.pace) })}
                onKeyDown={chipKeys(PACES, PACES.indexOf(choice.pace), (i) => onChange?.({ pace: PACES[i]! }))}
              >
                {choice.pace}
              </button>
              <span> {"·"} </span>
              <button type="button" className="launcher__chip" role="checkbox" aria-checked={choice.worktree} onClick={() => onChange?.({ worktree: !choice.worktree })}>
                <span className="launcher__box" aria-hidden="true" data-checked={choice.worktree || undefined} />
                worktree
              </button>
            </>
          )}
        </p>

        {programsOpen && (
          <div className="launcher__programs" id="launcher-programs" role="listbox" aria-label="Agents, shells and tools">
            {LAUNCH_GROUPS.map((g) => (
              <div key={g} className="launcher__group" role="group" aria-labelledby={`launcher-group-${g}`}>
                <span className="launcher__group-name" id={`launcher-group-${g}`}>
                  {g}
                </span>
                <div className="launcher__choices">
                  {programsIn(g).map((p) => {
                    const i = LAUNCH_PROGRAMS.indexOf(p);
                    return (
                      <div
                        key={p.id}
                        id={`launcher-prog-${i}`}
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
                          <AgentMark agent={p.id} size={16} />
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
          <ul className="launcher__list" id="launcher-list" role="listbox" aria-label="Folders">
            {options.map((h, i) => (
              <li
                key={h.item.id}
                id={`launcher-opt-${i}`}
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
          />
        </div>

        <footer className="launcher__footer">
          <span className="launcher__hint">
            <kbd>Tab</kbd> moves between choices {"·"} type a letter to pick {"·"} <kbd>Esc</kbd> closes
          </span>
          <span className="launcher__go">
            {nextNumber !== undefined && <span className="launcher__next">Session {nextNumber}</span>}
            <button type="button" className="launcher__start" onClick={onStart}>
              Start <kbd>Enter</kbd>
            </button>
          </span>
        </footer>
        <p className="launcher__sr" id="launcher-desc">
          {launcherSentence(choice, projects)}
          {nextNumber !== undefined ? `. It becomes session ${nextNumber}` : ""}
        </p>
      </div>
    </div>
  );
}
