import { encodingLabel, eolLabel, type Encoding, type Eol } from "./textFile";

/** The id of the hint when the caller gives none. One editor on screen at a time can use it; several pass their own. */
export const EDITOR_HINT = "editor-hint";

/** The line under the editor: where the cursor is, how the file is stored, and how to leave the editor with the keyboard. Not a live region. */
export function EditorFooter({ line, column, encoding, eol, hintId = EDITOR_HINT }: { line: number; column: number; encoding: Encoding; eol: Eol; hintId?: string }) {
  return (
    <div className="editor-footer">
      <span className="editor-footer__item">{`Ln ${line}, Col ${column}`}</span>
      <span className="editor-footer__item">{encodingLabel(encoding)}</span>
      <span className="editor-footer__item">{eolLabel(eol)}</span>
      <span className="editor-footer__hint" id={hintId}>
        Esc, then Tab, leaves the editor
      </span>
    </div>
  );
}
