import { useEffect, useEffectEvent, useRef, useState } from "react";
import { Switch } from "../../components/Switch/Switch";
import type { Endpoint } from "../../generated/Endpoint";
import type { ProjectInfo } from "../../generated/ProjectInfo";
import { projects as projectsApi } from "../../lib/api/projects";
import { getReferences, putReferences } from "../../lib/api/references";
import { addRow, firstProblem, forSave, patchRow, pickProject, removeRow, rowsOf, type Row } from "./projectsDoc";
import "./Settings.css";
import "./ProjectsSection.css";

export const NO_PROJECTS = "Start a session in a folder first; its project shows here.";
export const TOGGLE_HELP = "When a prompt names an alias, Marshell tells the agent which file or folder it means. Off until you turn it on.";
export const BROKEN_PATH = "Not found in the project, so it isn't sent.";
// Antigravity: "Antigravity can't take references yet." belongs here, under the toggle, only if the agy spike fails.

export type ProjectsState = {
  /** idle: nothing selected yet; loading and failed are about the selected project's references. */
  phase: "idle" | "loading" | "ready" | "failed";
  selected: string | null;
  /** The canonical project root the core answered with; sent back on Save. */
  root?: string;
  enabled: boolean;
  rows: Row[];
  /** The core's refusal, or why the references didn't load. */
  error: string | null;
  /** Bumped per refusal, so the sentence remounts and takes the focus each time. */
  errorSeq?: number;
  saving: boolean;
  saved?: boolean;
};

export type ProjectsHandlers = {
  onSelect: (path: string) => void;
  onToggle: (on: boolean) => void;
  onPatch: (index: number, patch: { name?: string; path?: string }) => void;
  onAdd: () => void;
  onRemove: (index: number) => void;
  onSave: () => void;
};

const takeFocus = (el: HTMLParagraphElement | null) => el?.focus();

/** The project's references: the picker, the switch, the alias rows and Save. Presentational; ProjectsSection owns the state. */
export function ProjectsView({ projects, state, handlers }: { projects: ProjectInfo[]; state: ProjectsState; handlers: ProjectsHandlers }) {
  if (state.selected === null) {
    return (
      <div className="projects">
        <p className="field__help">{NO_PROJECTS}</p>
      </div>
    );
  }
  const chosen = projects.find((p) => p.path === state.selected);
  return (
    <div className="projects">
      <div className="field">
        <label className="field__label" htmlFor="projects-picker">
          Project
        </label>
        <select id="projects-picker" className="field__control" value={state.selected} onChange={(e) => handlers.onSelect(e.target.value)}>
          {projects.map((p) => (
            <option key={p.path} value={p.path}>
              {p.name}
            </option>
          ))}
        </select>
        <p className="field__help projects__path" dir="ltr">
          {chosen?.path ?? state.selected}
        </p>
      </div>
      {state.phase === "loading" && <p className="field__help">Loading the references…</p>}
      {state.phase === "failed" && state.error && <p className="field__why">{state.error}</p>}
      {state.phase === "ready" && (
        <>
          <Switch label="Add references to prompts" checked={state.enabled} onChange={handlers.onToggle} reason={TOGGLE_HELP} />
          {state.rows.length === 0 ? (
            <p className="field__help">No aliases yet.</p>
          ) : (
            <table className="projects__table">
              <thead>
                <tr>
                  <th scope="col">Alias</th>
                  <th scope="col">Path in the project</th>
                  <td />
                </tr>
              </thead>
              <tbody>
                {state.rows.map((r, i) => (
                  <tr key={i} data-row={i}>
                    <td>
                      <input className="field__control projects__input" type="text" aria-label="Alias" value={r.name} spellCheck={false} onChange={(e) => handlers.onPatch(i, { name: e.target.value })} />
                    </td>
                    <td>
                      <input className="field__control projects__input projects__input--path" type="text" aria-label="Path in the project" value={r.path} spellCheck={false} dir="ltr" onChange={(e) => handlers.onPatch(i, { path: e.target.value })} />
                      {r.exists === false && <p className="field__why projects__broken">{BROKEN_PATH}</p>}
                    </td>
                    <td>
                      <button type="button" className="settings__btn" aria-label={`Remove ${r.name || i + 1}`} onClick={() => handlers.onRemove(i)}>
                        Remove
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          {state.error && (
            <p key={state.errorSeq ?? 0} className="field__why projects__error" tabIndex={-1} ref={takeFocus}>
              {state.error}
            </p>
          )}
          <div className="projects__actions">
            <button type="button" className="settings__btn" data-add onClick={handlers.onAdd}>
              Add alias
            </button>
            {/* aria-disabled, not disabled: a button that disables drops the focus it holds. */}
            <button type="button" className="settings__btn" data-primary aria-disabled={state.saving || undefined} onClick={() => !state.saving && handlers.onSave()}>
              {state.saving ? "Saving…" : "Save"}
            </button>
            {state.saved && <span className="projects__saved">Saved.</span>}
          </div>
        </>
      )}
    </div>
  );
}

const IDLE: ProjectsState = { phase: "idle", selected: null, enabled: false, rows: [], error: null, saving: false };
const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

/**
 * Settings → Projects, on the core. Mounted when first shown; `initialCwd` (the focused session's folder, from the palette) picks
 * the project, and a folder no recent project holds is offered as a project of its own.
 */
export function ProjectsSection({ endpoint, announce, initialCwd }: { endpoint: Endpoint; announce?: (text: string) => void; initialCwd?: string }) {
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
  const [state, setState] = useState<ProjectsState>(IDLE);
  const root = useRef<HTMLDivElement>(null);
  const afterEdit = useRef<{ row: number } | "add" | null>(null);
  const say = useEffectEvent((text: string) => announce?.(text));

  useEffect(() => {
    let live = true;
    const start = (list: ProjectInfo[]) => {
      if (!live) return;
      const { options, selected } = pickProject(list, initialCwd);
      setProjects(options);
      setState((s) => ({ ...s, selected }));
    };
    // Without the recent list the routed folder can still be edited.
    projectsApi.list(endpoint).then(start, () => start([]));
    return () => {
      live = false;
    };
  }, [endpoint, initialCwd]);

  const selected = state.selected;
  useEffect(() => {
    if (selected === null) return;
    let live = true;
    setState((s) => ({ ...s, phase: "loading", error: null, saved: false }));
    getReferences(endpoint, selected).then(
      (r) => live && setState((s) => ({ ...s, phase: "ready", root: r.root, enabled: r.enabled, rows: rowsOf(r.aliases) })),
      (e: unknown) => live && setState((s) => ({ ...s, phase: "failed", error: message(e) })),
    );
    return () => {
      live = false;
    };
  }, [endpoint, selected]);

  // After a row is added or removed the control that held the focus is gone or new: put it somewhere sensible, not on the page heading.
  useEffect(() => {
    const target = afterEdit.current;
    if (!target || !root.current) return;
    afterEdit.current = null;
    const row = target === "add" ? state.rows.length - 1 : Math.min(target.row, state.rows.length - 1);
    const el = row >= 0 ? root.current.querySelector<HTMLElement>(`[data-row="${row}"] input`) : root.current.querySelector<HTMLElement>("[data-add]");
    el?.focus();
  }, [state.rows]);

  const edit = (change: (s: ProjectsState) => Partial<ProjectsState>) => setState((s) => ({ ...s, ...change(s), error: null, saved: false }));

  const save = async () => {
    if (state.root === undefined) return;
    const fail = (text: string) => {
      setState((s) => ({ ...s, saving: false, saved: false, error: text, errorSeq: (s.errorSeq ?? 0) + 1 }));
      say(text);
    };
    const problem = firstProblem(state.rows);
    if (problem) return fail(problem);
    setState((s) => ({ ...s, saving: true, error: null, saved: false }));
    try {
      await putReferences(endpoint, { root: state.root, enabled: state.enabled, aliases: forSave(state.rows) });
    } catch (e) {
      return fail(message(e));
    }
    say("Saved.");
    // Read it back: the core trims, and says which paths are found.
    try {
      const r = await getReferences(endpoint, state.selected ?? state.root);
      setState((s) => ({ ...s, saving: false, saved: true, error: null, enabled: r.enabled, rows: rowsOf(r.aliases) }));
    } catch {
      setState((s) => ({ ...s, saving: false, saved: true }));
    }
  };

  const handlers: ProjectsHandlers = {
    onSelect: (path) => setState((s) => ({ ...s, selected: path })),
    onToggle: (on) => edit(() => ({ enabled: on })),
    onPatch: (i, patch) => edit((s) => ({ rows: patchRow(s.rows, i, patch) })),
    onAdd: () => {
      afterEdit.current = "add";
      edit((s) => ({ rows: addRow(s.rows) }));
    },
    onRemove: (i) => {
      afterEdit.current = { row: i };
      edit((s) => ({ rows: removeRow(s.rows, i) }));
    },
    onSave: () => void save(),
  };
  return (
    <div ref={root}>
      <ProjectsView projects={projects} state={state} handlers={handlers} />
    </div>
  );
}
