import { useId, useRef } from "react";
import { useDialogFocus } from "../../lib/useDialogFocus";
import { useCoverPage } from "../browser/pageCover";
import "../window/CloseDialog.css";

export type SaveChoice = "save" | "discard" | "cancel";

/**
 * The question closing a tab with unsaved edits asks. Presentational: the owner saves, drops the draft or does nothing, and then
 * closes it. It is the close dialog's sheet. Save is the default and takes the focus; Escape, a press outside and Cancel only close it.
 */
export function SaveDialog({ name, onChoose }: { name: string; onChoose: (choice: SaveChoice) => void }) {
  const dialog = useRef<HTMLDivElement>(null);
  const save = useRef<HTMLButtonElement>(null);
  const title = useId();
  const description = useId();
  useCoverPage();
  useDialogFocus(dialog, save, () => onChoose("cancel"));
  return (
    <div className="close-dialog">
      <div className="close-dialog__backdrop" onClick={() => onChoose("cancel")} />
      <div ref={dialog} className="close-dialog__sheet" role="dialog" aria-modal="true" aria-labelledby={title} aria-describedby={description}>
        <h2 id={title} className="close-dialog__title">
          Save the changes to {name}?
        </h2>
        <p id={description} className="close-dialog__line">
          Its unsaved edits are lost if you don't save them.
        </p>
        <footer className="close-dialog__foot">
          <button ref={save} type="button" className="close-dialog__btn" data-default onClick={() => onChoose("save")}>
            Save
          </button>
          <button type="button" className="close-dialog__btn" onClick={() => onChoose("discard")}>
            Don't save
          </button>
          <button type="button" className="close-dialog__btn" onClick={() => onChoose("cancel")}>
            Cancel
          </button>
        </footer>
      </div>
    </div>
  );
}
