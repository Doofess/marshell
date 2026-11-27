import { type Ref, useId } from "react";
import type { CopyRow } from "../../generated/CopyRow";
import { Editor, type EditorHandle } from "../editor/Editor";
import type { EditorPrefs } from "../editor/editorPrefs";
import { CLI_NAME } from "../onboarding/consent";
import type { CliId } from "../onboarding/types";
import { type Draft, copilotLocked, toggleCli } from "./instructionsModel";
import { LoadsIn } from "./LoadsIn";

const CLIS: CliId[] = ["claude", "codex", "antigravity", "qwen", "copilot"];
export type FormErrors = Partial<Record<"name" | "description" | "globs" | "save", string>>;

const noop = () => {};

/**
 * The open doc: its fields, the text in the editor, Save and Delete, and where the doc loads. Each error sits under its
 * field and is tied to it; a save error sits by the buttons. Every change goes up through onDraft.
 */
export function DocForm({ draft, docError, rows, errors, busy, prefs, appChord, editorRef, onDraft, onSave, onDelete, onCopyFix }: {
  draft: Draft;
  docError?: string;
  rows: CopyRow[];
  errors: FormErrors;
  busy: boolean;
  prefs: EditorPrefs;
  appChord: (e: KeyboardEvent) => boolean;
  editorRef: Ref<EditorHandle> | null;
  onDraft: (d: Draft) => void;
  onSave: () => void;
  onDelete: () => void;
  onCopyFix: (row: CopyRow, takeCopy: boolean) => void;
}) {
  const uid = useId();
  const id = (s: string) => `${uid}-${s}`;
  const locked = copilotLocked(draft);
  const err = (key: keyof FormErrors) => (errors[key] === undefined ? undefined : id(`${key}-error`));
  const described = (key: "name" | "description" | "globs", hint?: string) => [err(key), hint].filter(Boolean).join(" ") || undefined;
  const loadsId = id("loads");
  return (
    <div className="docform">
      {docError !== undefined && <p className="instructions__problem">{docError}</p>}
      <form
        className="docform__form"
        onSubmit={(e) => {
          e.preventDefault();
          if (!busy) onSave();
        }}
      >
        <div className="docform__field">
          <label htmlFor={id("name")}>Name</label>
          <input id={id("name")} name="name" className="instructions__input" type="text" value={draft.name} aria-invalid={errors.name !== undefined ? true : undefined} aria-describedby={described("name")} onChange={(e) => onDraft({ ...draft, name: e.target.value })} />
          {errors.name !== undefined && (
            <p id={err("name")} className="instructions__error">
              {errors.name}
            </p>
          )}
        </div>
        <div className="docform__field">
          <label htmlFor={id("description")}>Description</label>
          <input id={id("description")} name="description" className="instructions__input" type="text" value={draft.description} aria-invalid={errors.description !== undefined ? true : undefined} aria-describedby={described("description")} onChange={(e) => onDraft({ ...draft, description: e.target.value })} />
          {errors.description !== undefined && (
            <p id={err("description")} className="instructions__error">
              {errors.description}
            </p>
          )}
        </div>
        <fieldset className="docform__clis">
          <legend>CLIs</legend>
          {CLIS.map((cli) => {
            const lock = cli === "copilot" && locked;
            return (
              <label key={cli} className="docform__check">
                <input type="checkbox" checked={draft.clis.includes(cli)} disabled={lock} aria-describedby={lock ? id("copilot") : undefined} onChange={() => onDraft(toggleCli(draft, cli))} />
                <span>{CLI_NAME[cli]}</span>
              </label>
            );
          })}
          {locked && (
            <p id={id("copilot")} className="instructions__hint">
              Copilot CLI reads .claude/rules, so it loads this too.
            </p>
          )}
        </fieldset>
        <div className="docform__field">
          <label htmlFor={id("globs")}>Globs</label>
          <textarea id={id("globs")} name="globs" className="instructions__input" rows={3} value={draft.globsText} aria-invalid={errors.globs !== undefined ? true : undefined} aria-describedby={described("globs", id("globs-hint"))} onChange={(e) => onDraft({ ...draft, globsText: e.target.value })} />
          <p id={id("globs-hint")} className="instructions__hint">
            One per line, relative to the project. Empty: the doc always loads.
          </p>
          {errors.globs !== undefined && (
            <p id={err("globs")} className="instructions__error">
              {errors.globs}
            </p>
          )}
        </div>
        {draft.scope === "project" && (
          <label className="docform__check">
            <input type="checkbox" checked={draft.enabled} onChange={(e) => onDraft({ ...draft, enabled: e.target.checked })} />
            <span>On in this project</span>
          </label>
        )}
        <div className="docform__field">
          <span id={id("text")} className="docform__label">
            Text
          </span>
          <div className="docform__editor" role="group" aria-labelledby={id("text")}>
            <Editor
              key={`${draft.scope}/${draft.oldName ?? ""}`}
              ref={editorRef ?? undefined}
              name={`${draft.name === "" ? "doc" : draft.name}.md`}
              path={`instructions/${draft.scope}/${draft.oldName ?? "new"}.md`}
              initial={draft.body}
              readOnly={false}
              wrap
              prefs={prefs}
              onEdit={(body) => onDraft({ ...draft, body })}
              onCursor={noop}
              appChord={appChord}
            />
          </div>
        </div>
        <div className="docform__actions">
          <button type="submit" className="instructions__btn" data-primary aria-disabled={busy ? "true" : undefined}>
            Save
          </button>
          <span className="instructions__hint">Ctrl+S</span>
          {draft.oldName !== null && (
            <button type="button" className="instructions__btn" aria-disabled={busy ? "true" : undefined} onClick={busy ? undefined : onDelete}>
              Delete…
            </button>
          )}
        </div>
        {errors.save !== undefined && <p className="instructions__error">{errors.save}</p>}
      </form>
      {draft.oldName !== null && (
        <section className="docform__loads" aria-labelledby={loadsId}>
          <h3 id={loadsId} className="docform__subhead">
            Loads in
          </h3>
          <LoadsIn rows={rows} onCopyFix={onCopyFix} />
        </section>
      )}
    </div>
  );
}
