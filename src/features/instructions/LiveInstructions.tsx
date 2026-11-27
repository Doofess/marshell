import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import type { ConfigPreview } from "../../generated/ConfigPreview";
import type { CopyRow } from "../../generated/CopyRow";
import type { Endpoint } from "../../generated/Endpoint";
import type { ProjectInfo } from "../../generated/ProjectInfo";
import { applyPreview, undoSnapshot } from "../../lib/api/agents";
import { ApiError } from "../../lib/api/client";
import { libraryView, previewCopyFix, previewDelete, reviewCopies, saveDoc } from "../../lib/api/instructions";
import { projects as projectsApi } from "../../lib/api/projects";
import { findCommand } from "../../lib/bindings";
import { osOfPlatform } from "../../lib/keymap";
import { useDialogFocus } from "../../lib/useDialogFocus";
import { useCoverPage } from "../browser/pageCover";
import { undoRefusedAnnouncement } from "../doctor/doctorUi";
import { SaveDialog, type SaveChoice } from "../editor/SaveDialog";
import { isSaveKey } from "../editor/TextTab";
import type { EditorPrefs } from "../editor/editorPrefs";
import { CopiesReview } from "./CopiesReview";
import { InstructionsPage } from "./InstructionsPage";
import { copyFixOf, toSave } from "./instructionsModel";
import { conflictLine, findDoc, initialState, instructionsReducer, isDirtyNow } from "./instructionsState";
import "../doctor/Doctor.css";
import "../doctor/LiveDoctor.css";
import "./LiveInstructions.css";

const message = (e: unknown) => (e instanceof ApiError ? e.message : e instanceof Error ? e.message : String(e));

export type LiveInstructionsProps = {
  endpoint: Endpoint | null;
  /** The focused session's folder, else the shell panel's: the first project is the one holding it. */
  folder: string | null;
  prefs: EditorPrefs;
  announce: (text: string) => void;
  onClose: () => void;
};

/**
 * The Instructions page over the live window. `instructionsReducer` keeps its state; this makes the calls: the view, saves,
 * and the previews and applies of Review, Delete… and the copy fixes (each step reads the view again).
 */
export function LiveInstructions({ endpoint, folder, prefs, announce, onClose }: LiveInstructionsProps) {
  const [s, dispatch] = useReducer(instructionsReducer, undefined, () => initialState([], folder));
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
  const root = useRef<HTMLDivElement>(null);
  // How to read the open sheet's preview again (Reload).
  const replan = useRef<(() => Promise<ConfigPreview>) | null>(null);
  const latest = useRef(s);
  latest.current = s;
  const sheetOpen = useRef(false);
  sheetOpen.current = s.review !== null || s.asking !== null;
  const os = endpoint ? osOfPlatform(endpoint.platform) : "windows";
  useCoverPage();

  const requestClose = () => {
    if (isDirtyNow(latest.current)) dispatch({ type: "closeAsked" });
    else onClose();
  };
  useDialogFocus(root, root, () => {
    if (!sheetOpen.current) requestClose();
  });
  useEffect(() => {
    if (s.closed) onClose();
  }, [s.closed, onClose]);

  // The known projects; the first project is the one holding the folder, once they are read.
  useEffect(() => {
    if (!endpoint) return;
    let live = true;
    projectsApi.list(endpoint).then(
      (list) => {
        if (!live) return;
        setProjects(list);
        const first = initialState(list, folder).project;
        if (first !== null && latest.current.project === null) dispatch({ type: "pickProject", path: first });
      },
      () => {},
    );
    return () => {
      live = false;
    };
    // The folder is the one the page opened with.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [endpoint]);

  const readView = useCallback(async () => {
    if (!endpoint) return null;
    try {
      return await libraryView(endpoint, latest.current.project);
    } catch (e) {
      announce(`Couldn't read the instruction library: ${message(e)}`);
      return null;
    }
  }, [endpoint, announce]);
  const refresh = useCallback(async () => {
    const view = await readView();
    if (view !== null) dispatch({ type: "viewLoaded", view });
  }, [readView]);
  useEffect(() => {
    void refresh();
    // Read again when the project changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [endpoint, s.project]);

  /** Saves the draft; `hash` is a fresh disk hash ("Save over it"). True when it is saved. */
  const save = async (hash?: string | null): Promise<boolean> => {
    const cur = latest.current;
    const d = cur.draft;
    if (!endpoint || d === null || cur.busy) return false;
    const name = d.name.trim();
    dispatch({ type: "saveStarted" });
    try {
      const r = await saveDoc(endpoint, toSave(hash === undefined ? d : { ...d, hash }, cur.project));
      if (r.ok) {
        dispatch({ type: "saved", view: r.saved.view, name });
        announce(`Saved ${name}.`);
        return true;
      }
      if (r.conflict) {
        dispatch({ type: "saveConflict" });
        announce(conflictLine(name));
      } else {
        dispatch({ type: "saveRefused", message: r.message });
        announce(r.message);
      }
    } catch (e) {
      dispatch({ type: "saveRefused", message: message(e) });
      announce(message(e));
    }
    return false;
  };

  const reload = async () => {
    const view = await readView();
    if (view !== null) dispatch({ type: "reloaded", view });
  };
  const saveOver = async () => {
    const view = await readView();
    if (view === null) return;
    await save(findDoc(view, latest.current.open)?.hash ?? null);
  };

  const answer = async (choice: SaveChoice) => {
    if (choice === "cancel") return dispatch({ type: "cancel" });
    if (choice === "save" && !(await save())) return dispatch({ type: "cancel" });
    dispatch({ type: "discard" });
  };

  // A preview of copies: shown in the sheet, which Reload re-reads with the same request.
  const show = async (title: string, plan: () => Promise<ConfigPreview>) => {
    try {
      const preview = await plan();
      replan.current = plan;
      dispatch({ type: "reviewShown", title, preview });
    } catch (e) {
      announce(`Couldn't prepare the change: ${message(e)}`);
    }
  };
  const review = () => {
    if (endpoint) void show("Update the copies", () => reviewCopies(endpoint, latest.current.project));
  };
  const remove = () => {
    const { open, project } = latest.current;
    if (endpoint && open !== null) void show(`Delete ${open.name}`, () => previewDelete(endpoint, project === null ? { scope: open.scope, name: open.name } : { scope: open.scope, project, name: open.name }));
  };
  const copyFix = (row: CopyRow, takeCopy: boolean) => {
    const { view, open, project } = latest.current;
    const doc = findDoc(view, open);
    if (endpoint && doc !== null) void show(takeCopy ? `Use this copy's text for ${doc.name}` : `Replace this copy with ${doc.name}`, () => previewCopyFix(endpoint, copyFixOf(doc, row, project, takeCopy)));
  };
  const reloadSheet = async () => {
    const plan = replan.current;
    const title = latest.current.review?.title;
    if (plan === null || title === undefined) return;
    await show(title, plan);
  };
  const apply = async () => {
    const r = latest.current.review;
    if (!endpoint || r === null || r.busy) return;
    dispatch({ type: "reviewBusy" });
    try {
      const out = await applyPreview(endpoint, r.preview);
      if (out.ok) {
        dispatch({ type: "applied", snapshot: out.result.snapshot_id });
        announce("Copies updated.");
        await refresh();
      } else if (out.conflict) dispatch({ type: "reviewConflict" });
      else {
        dispatch({ type: "reviewCancelled" });
        announce(out.message);
      }
    } catch (e) {
      dispatch({ type: "reviewCancelled" });
      announce(`Couldn't update the copies: ${message(e)}`);
    }
  };
  const undo = async (snapshot: string) => {
    if (!endpoint) return;
    try {
      const out = await undoSnapshot(endpoint, snapshot);
      if (out.ok) {
        dispatch({ type: "undone" });
        announce("Put back.");
        await refresh();
      } else if ("changed" in out) announce(undoRefusedAnnouncement(out.changed).text);
      else announce(out.message);
    } catch (e) {
      announce(`Couldn't put the copies back: ${message(e)}`);
    }
  };

  // Ctrl+S saves. The editor leaves the key to the page (`appChord`), and it comes up here.
  const appChord = useCallback((e: KeyboardEvent) => isSaveKey(e, os) || findCommand(e, os) !== undefined, [os]);
  const saveKey = (e: React.KeyboardEvent) => {
    if (!isSaveKey(e, os)) return;
    e.preventDefault();
    void save();
  };
  const snapshot = s.status?.snapshot;

  return (
    <div ref={root} className="live-instructions" role="dialog" aria-modal="true" aria-label="Instructions" tabIndex={-1} onKeyDown={saveKey}>
      <InstructionsPage
        view={s.view}
        projects={projects}
        project={s.project}
        open={s.open}
        draft={s.draft}
        errors={s.errors}
        busy={s.busy}
        status={s.status === null ? null : snapshot === undefined ? { text: s.status.text } : { text: s.status.text, undo: () => void undo(snapshot) }}
        prefs={prefs}
        appChord={appChord}
        onPickProject={(path) => dispatch({ type: "pickProject", path })}
        onOpen={(scope, name) => dispatch({ type: "open", scope, name })}
        onNew={(scope) => dispatch({ type: "newDoc", scope })}
        onDraft={(draft) => dispatch({ type: "edit", draft })}
        onSave={() => void save()}
        onDelete={remove}
        onReview={review}
        onCopyFix={copyFix}
        onCheckAgain={() => void refresh()}
        onClose={requestClose}
        editorRef={null}
      />
      {s.conflict !== null && (
        <div className="live-instructions__conflict" role="group" aria-label="Save conflict">
          <p>{s.conflict}</p>
          <button type="button" onClick={() => void reload()}>
            Reload
          </button>
          <button type="button" onClick={() => void saveOver()}>
            Save over it
          </button>
        </div>
      )}
      {s.review !== null && (
        <div className="live-doctor__sheet">
          <CopiesReview title={s.review.title} preview={s.review.preview} busy={s.review.busy} conflict={s.review.conflict} onApply={() => void apply()} onCancel={() => dispatch({ type: "reviewCancelled" })} onReload={() => void reloadSheet()} />
        </div>
      )}
      {s.asking !== null && <SaveDialog name={s.draft?.name.trim() || "this doc"} onChoose={(c) => void answer(c)} />}
    </div>
  );
}
