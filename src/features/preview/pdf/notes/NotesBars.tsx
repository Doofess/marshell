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
  /** Puts the keyboard on the pages. */
  toPages(): void;
  onUndo(): void;
  onCopy(): void;
  onReload(): void;
  onOverwrite(): void;
  onSave(): void;
};

/** A button the rows offer, by its name. */
export type NotesButton = { label: string; press(): void };

/**
 * The buttons the rows offer now, in the order they are drawn. A press puts the keyboard on the pages before it acts: the button
 * may go with its row (Reload, Overwrite, Save, Undo), and the keyboard would fall out of the tab, so it goes where a text tab's
 * bar sends it.
 */
export function notesButtons(p: NotesBarsProps): NotesButton[] {
  const on = (label: string, act: () => void): NotesButton => ({
    label,
    press: () => {
      p.toPages();
      act();
    },
  });
  if (p.conflict === "deleted") return [on("Save as a copy", p.onCopy), on("Save", p.onSave)];
  if (p.conflict === "changed") return [on("Save as a copy", p.onCopy), on("Reload", p.onReload), on("Overwrite", p.onOverwrite)];
  if (p.line === null) return [];
  return [...(p.undo ? [on("Undo", p.onUndo)] : []), ...(p.copy ? [on("Save as a copy", p.onCopy)] : [])];
}

/**
 * The rows under a PDF's toolbar that say how its notes stand with the file: the save line, or the bar when the file changed or
 * went under unsaved notes. Neither takes focus when it appears, and neither is a live region: the announcer says their lines.
 */
export function NotesBars(p: NotesBarsProps) {
  const id = useId();
  const buttons = (className: string) =>
    notesButtons(p).map((b) => (
      <button key={b.label} type="button" className={className} onClick={b.press}>
        {b.label}
      </button>
    ));
  if (p.conflict !== null) {
    return (
      <div className="changed-bar pdf__notesbar" role="group" aria-labelledby={id}>
        <p id={id} className="changed-bar__text">
          {p.conflict === "deleted" ? `${p.name} was deleted. Your notes aren't saved.` : `${p.name} changed on disk. Your notes aren't saved.`}
        </p>
        <div className="changed-bar__actions">{buttons("changed-bar__button")}</div>
      </div>
    );
  }
  if (p.line === null) return null;
  return (
    <div className="pdf__saveline">
      <span className="pdf__savetext">{p.line}</span>
      {buttons("text-tab__button")}
    </div>
  );
}
