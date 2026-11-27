import { type Ref, useId } from "react";
import type { CopyRow } from "../../generated/CopyRow";
import type { DocScope } from "../../generated/DocScope";
import type { LibraryView } from "../../generated/LibraryView";
import type { ProjectInfo } from "../../generated/ProjectInfo";
import type { EditorHandle } from "../editor/Editor";
import type { EditorPrefs } from "../editor/editorPrefs";
import { DocForm, type FormErrors } from "./DocForm";
import { DocList, type OpenDoc } from "./DocList";
import { type Draft, barText } from "./instructionsModel";
import "../../components/Page/Page.css";
import "./instructions.css";

export type InstructionsPageProps = {
  view: LibraryView | null;
  projects: ProjectInfo[];
  project: string | null;
  open: OpenDoc | null;
  draft: Draft | null;
  errors: FormErrors;
  busy: boolean;
  status: { text: string; undo?: () => void } | null;
  prefs: EditorPrefs;
  appChord: (e: KeyboardEvent) => boolean;
  textRev: number;
  onPickProject: (path: string) => void;
  onOpen: (scope: DocScope, name: string) => void;
  onNew: (scope: DocScope) => void;
  onDraft: (d: Draft) => void;
  onSave: () => void;
  onDelete: () => void;
  onReview: () => void;
  onCopyFix: (row: CopyRow, takeCopy: boolean) => void;
  onCheckAgain: () => void;
  onClose: () => void;
  editorRef: Ref<EditorHandle> | null;
};

/**
 * The Instructions page: the docs on the left, the open doc on the right, and a bar over both when copies are out of date or
 * were changed outside Marshell. It only draws; the parent owns the data and every handler.
 */
export function InstructionsPage(p: InstructionsPageProps) {
  const title = useId();
  const bar = p.view === null ? null : barText(p.view.out_of_date, p.view.changed_outside);
  const doc = p.open === null ? undefined : [...(p.view?.user_docs ?? []), ...(p.view?.project_docs ?? [])].find((d) => d.doc.scope === p.open!.scope && d.doc.name.toLowerCase() === p.open!.name.toLowerCase());
  return (
    <section className="instructions" aria-labelledby={title}>
      <header className="instructions__head">
        <h2 id={title} className="instructions__title">
          Instructions
        </h2>
        <div className="instructions__actions">
          <button type="button" className="instructions__btn" onClick={p.onCheckAgain}>
            Check again
          </button>
          <button type="button" className="instructions__btn" aria-label="Close Instructions" onClick={p.onClose}>
            Close
          </button>
        </div>
      </header>
      {(p.status !== null || bar !== null) && (
        <div className="instructions__bar">
          {p.status !== null && (
            <p className="instructions__bar-text">
              {p.status.text}
              {p.status.undo !== undefined && (
                <button type="button" className="instructions__btn" onClick={p.status.undo}>
                  Undo
                </button>
              )}
            </p>
          )}
          {bar !== null && (
            <p className="instructions__bar-text">
              {bar}
              <button type="button" className="instructions__btn" onClick={p.onReview}>
                Review
              </button>
            </p>
          )}
        </div>
      )}
      <div className="instructions__body page__body" role="region" aria-label="Instructions content" tabIndex={0}>
        <DocList view={p.view} projects={p.projects} project={p.project} open={p.open} onPickProject={p.onPickProject} onOpen={p.onOpen} onNew={p.onNew} />
        <div className="instructions__main">
          {p.draft === null ? (
            <p className="instructions__empty">Pick a doc, or start a new one.</p>
          ) : (
            <DocForm textRev={p.textRev} draft={p.draft} docError={doc?.doc.error} rows={doc?.rows ?? []} errors={p.errors} busy={p.busy} prefs={p.prefs} appChord={p.appChord} editorRef={p.editorRef} onDraft={p.onDraft} onSave={p.onSave} onDelete={p.onDelete} onCopyFix={p.onCopyFix} />
          )}
        </div>
      </div>
    </section>
  );
}
