import { useId } from "react";

export type NotesBarsProps = {
  name: string;
  /** The file changed or was deleted on disk under unsaved notes: the bar stands in for the save line. */
  conflict: "changed" | "deleted" | null;
  /** The save line's text, or null for none. */
  line: string | null;
  /** The line offers Undo: a save just went through. */
  undo: boolean;
  /** The line offers Save as a copy: the save was refused. */
  copy: boolean;
  onUndo(): void;
  onCopy(): void;
  onReload(): void;
  onOverwrite(): void;
  onSave(): void;
};

/**
 * The rows under a PDF's toolbar that say how its notes stand with the file: the save line, or the bar when the file changed or
 * went under unsaved notes. Neither takes focus when it appears, and neither is a live region: the announcer says their lines.
 */
export function NotesBars(p: NotesBarsProps) {
  const id = useId();
  if (p.conflict !== null) {
    const deleted = p.conflict === "deleted";
    return (
      <div className="changed-bar pdf__notesbar" role="group" aria-labelledby={id}>
        <p id={id} className="changed-bar__text">
          {deleted ? `${p.name} was deleted. Your notes aren't saved.` : `${p.name} changed on disk. Your notes aren't saved.`}
        </p>
        <div className="changed-bar__actions">
          <button type="button" className="changed-bar__button" onClick={p.onCopy}>
            Save as a copy
          </button>
          {deleted ? (
            <button type="button" className="changed-bar__button" onClick={p.onSave}>
              Save
            </button>
          ) : (
            <>
              <button type="button" className="changed-bar__button" onClick={p.onReload}>
                Reload
              </button>
              <button type="button" className="changed-bar__button" onClick={p.onOverwrite}>
                Overwrite
              </button>
            </>
          )}
        </div>
      </div>
    );
  }
  if (p.line === null) return null;
  return (
    <div className="pdf__saveline">
      <span className="pdf__savetext">{p.line}</span>
      {p.undo && (
        <button type="button" className="text-tab__button" onClick={p.onUndo}>
          Undo
        </button>
      )}
      {p.copy && (
        <button type="button" className="text-tab__button" onClick={p.onCopy}>
          Save as a copy
        </button>
      )}
    </div>
  );
}
